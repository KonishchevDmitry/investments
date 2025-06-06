mod analyser;
pub mod config;
mod instrument_view;
pub mod types;
pub mod statistics;

use std::collections::HashMap;

use easy_logging::GlobalContext;
use itertools::Itertools;
use strum::IntoEnumIterator;

use crate::broker_statement::{BrokerStatement, StockSell, StockSellType};
use crate::commissions::CommissionCalc;
use crate::config::PortfolioConfig;
use crate::core::EmptyResult;
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverterRc;
use crate::instruments::Instrument;
use crate::localities::Country;
use crate::quotes::QuotesRc;
use crate::taxes::{LtoDeductionCalculator, TaxCalculator};

use self::analyser::PortfolioPerformanceAnalyser;
use self::config::{AssetGroupConfig, PerformanceMergingConfig};
use self::statistics::{AssetGroup, PortfolioStatistics, LtoStatistics};
use self::types::PerformanceAnalysisMethod;

pub struct PortfolioAnalyser<'a> {
    pub country: Country,
    pub interactive: bool,
    pub include_closed_positions: bool,

    pub asset_groups: &'a HashMap<String, AssetGroupConfig>,
    pub merge_performance: Option<&'a PerformanceMergingConfig>,

    pub quotes: QuotesRc,
    pub converter: CurrencyConverterRc,

    pub lto_calc: LtoDeductionCalculator,
    pub taxes: TaxCalculator,
}

impl<'a> PortfolioAnalyser<'a> {
    pub fn process(
        mut self, mut portfolios: Vec<(&'a PortfolioConfig, BrokerStatement)>,
        statistics: &mut PortfolioStatistics,
    ) -> EmptyResult {
        let multiple = portfolios.len() > 1;

        for (_, statement) in &portfolios {
            statement.batch_quotes(&self.quotes)?;
        }

        if multiple {
            self.quotes.execute()?; // To write shared logs without context
        }

        for (name, config) in self.asset_groups {
            let group = AssetGroup {
                taxes: TaxCalculator::new(self.country.clone()),
                net_value: config.currencies.iter().map(|currency| Cash::zero(currency)).collect(),
            };
            assert!(statistics.asset_groups.insert(name.clone(), group).is_none());
        }

        for (portfolio, statement) in &mut portfolios {
            let _logging_context = multiple.then(|| GlobalContext::new(&portfolio.name));

            let broker = statement.broker.type_;
            if self.interactive {
                statement.check_date();
            }

            statistics.process(|statistics| {
                let cash_assets = statement.assets.cash.total_assets_real_time(
                    &statistics.currency, &self.converter)?;

                Ok(statistics.add_assets(&portfolio.name, broker, "Cash", cash_assets, cash_assets))
            })?;

            let net_value = statement.net_value(&self.converter, &self.quotes, portfolio.currency(), true)?;
            let mut commission_calc = CommissionCalc::new(
                self.converter.clone(), statement.broker.commission_spec.clone(), net_value)?;

            for (symbol, quantity) in statement.open_positions.clone().into_iter().sorted_unstable() {
                let price = self.quotes.get(statement.get_quote_query(&symbol))?;
                statement.emulate_sell(&symbol, quantity, price, &mut commission_calc)?;
            }

            let additional_commissions = statement.emulate_commissions(commission_calc)?;
            statistics.process(|statistics| {
                let additional_commissions = additional_commissions.total_assets_real_time(
                    &statistics.currency, &self.converter)?;

                statistics.projected_commissions += additional_commissions;
                Ok(())
            })?;

            statement.process_trades(None)?;

            for trade in statement.stock_sells.iter().rev() {
                if !trade.emulation {
                    break;
                }

                let instrument = statement.instrument_info.get_or_empty(&trade.symbol);
                self.process_asset(portfolio, &instrument, trade, statistics)?;
            }
        }

        self.process_totals(portfolios, statistics)
    }

    fn process_asset(
        &mut self, portfolio: &PortfolioConfig, instrument: &Instrument, trade: &StockSell,
        statistics: &mut PortfolioStatistics,
    ) -> EmptyResult {
        let (volume, commission) = match trade.type_ {
            StockSellType::Trade {volume, commission, ..} => (volume, commission),
            _ => unreachable!(),
        };

        let mut details = trade.calculate(&self.country, instrument, &portfolio.tax_exemptions, &self.converter)?;

        for source in &mut details.fifo {
            if let Some(deductible) = source.long_term_ownership_deductible.take() {
                self.lto_calc.add(deductible.profit, deductible.years, false);
                details.taxable_local_profit.amount -= deductible.profit;
            }
        }

        let (tax_year, _) = portfolio.tax_payment_day().get(trade.execution_date, true);
        let totals_tax = details.tax(&mut self.taxes, tax_year);

        for (name, config) in self.asset_groups {
            if let Some(portfolios) = config.portfolios.as_ref() {
                if !portfolios.contains(&portfolio.name) {
                    continue;
                }
            }

            if !config.instruments.contains(&trade.symbol) {
                continue;
            }

            let group = statistics.asset_groups.get_mut(name).unwrap();
            let group_tax = details.tax(&mut group.taxes, tax_year);

            for net_value in group.net_value.iter_mut() {
                net_value.amount += self.converter.real_time_convert_to(volume, net_value.currency)?;
                net_value.amount -= self.converter.real_time_convert_to(commission, net_value.currency)?;
                net_value.amount -= self.converter.real_time_convert_to(group_tax.to_pay, net_value.currency)?;
            }
        }

        statistics.process(|statistics| {
            let currency = &statistics.currency;

            let volume = self.converter.real_time_convert_to(volume, currency)?;
            let commission = self.converter.real_time_convert_to(commission, currency)?;

            let tax_to_pay = self.converter.real_time_convert_to(totals_tax.to_pay, currency)?;
            let tax_deduction = self.converter.real_time_convert_to(totals_tax.deduction, currency)?;

            statistics.add_assets(&portfolio.name, portfolio.broker, &trade.symbol, volume, volume - commission - tax_to_pay);
            statistics.projected_commissions += commission;
            statistics.projected_taxes += tax_to_pay;
            statistics.projected_tax_deductions += tax_deduction;

            Ok(())
        })
    }

    fn process_totals(
        self, portfolios: Vec<(&'a PortfolioConfig, BrokerStatement)>, statistics: &mut PortfolioStatistics,
    ) -> EmptyResult {
        let mut applied_lto = None;

        for method in PerformanceAnalysisMethod::iter() {
            let _logging_context = GlobalContext::new(&method.to_string());

            statistics.process(|statistics| {
                let mut analyser = PortfolioPerformanceAnalyser::new(
                    &self.country, &statistics.currency, &self.converter,
                    method, self.include_closed_positions);

                for (portfolio, statement) in &portfolios {
                    let mut performance_merging_config = portfolio.merge_performance.clone();
                    if let Some(merge_performance) = self.merge_performance {
                        performance_merging_config.add(merge_performance)?;
                    }
                    analyser.add(portfolio, statement, performance_merging_config)?;
                }

                let (performance, lto) = analyser.analyse()?;
                statistics.set_performance(method, performance);

                if method == PerformanceAnalysisMethod::Real {
                    if let Some(prev) = applied_lto.take() {
                        assert_eq!(prev, lto);
                    }
                    applied_lto.replace(lto);
                }

                Ok(())
            })?;
        }

        statistics.lto = Some(LtoStatistics {
            applied: applied_lto.unwrap(),
            projected: self.lto_calc.calculate(),
        });

        Ok(())
    }
}