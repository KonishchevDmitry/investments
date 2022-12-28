pub mod config;
pub mod deposit_emulator;
mod deposit_performance;
mod instrument_view;
mod portfolio_analysis;
mod portfolio_performance;
mod sell_simulation;

use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use easy_logging::GlobalContext;
use log::warn;

use crate::brokers::Broker;
use crate::broker_statement::{BrokerStatement, ReadingStrictness, StockSellType};
use crate::commissions::CommissionCalc;
use crate::config::{Config, PortfolioConfig};
use crate::core::{GenericResult, EmptyResult};
use crate::currency::converter::{CurrencyConverter, CurrencyConverterRc};
use crate::db;
use crate::localities::Country;
use crate::quotes::Quotes;
use crate::taxes::{IncomeType, LtoDeductionCalculator, LtoDeduction, NetLtoDeduction};
use crate::telemetry::TelemetryRecordBuilder;
use crate::types::Decimal;

use self::config::{AssetGroupConfig, PerformanceMergingConfig};
use self::portfolio_analysis::PortfolioPerformanceAnalysis;
use self::portfolio_performance::PortfolioPerformanceAnalyser;

pub struct PortfolioStatistics {
    country: Country,
    pub currencies: Vec<PortfolioCurrencyStatistics>,
    pub lto: Option<LtoStatistics>,
}

pub struct LtoStatistics {
    pub applied: BTreeMap<i32, NetLtoDeduction>,
    pub projected: LtoDeduction,
}

impl PortfolioStatistics {
    fn new(country: Country) -> PortfolioStatistics {
        PortfolioStatistics {
            country,
            currencies: ["USD", "RUB"].iter().map(|&currency| (
                PortfolioCurrencyStatistics {
                    currency: currency.to_owned(),

                    assets: BTreeMap::new(),
                    brokers: BTreeMap::new(),
                    performance: None,

                    projected_taxes: dec!(0),
                    projected_tax_deductions: dec!(0),
                    projected_commissions: dec!(0),
                }
            )).collect(),
            lto: None,
        }
    }

    pub fn print(&self) {
        let lto = self.lto.as_ref().unwrap();

        for (year, result) in &lto.applied {
            if !result.loss.is_zero() {
                warn!("Long-term ownership tax deduction loss in {}: {}.",
                      year, self.country.cash(result.loss));
            }

            if !result.applied_above_limit.is_zero() {
                warn!("Long-term ownership tax deductions applied in {} have exceeded the total limit by {}.",
                      year, self.country.cash(result.applied_above_limit));
            }
        }

        for statistics in &self.currencies {
            statistics.performance.as_ref().unwrap().print(&format!(
                "Average rate of return from cash investments in {}", &statistics.currency));
        }

        if !lto.projected.deduction.is_zero() {
            lto.projected.print("Projected LTO deduction")
        }
    }

    fn process<F>(&mut self, mut handler: F) -> EmptyResult
        where F: FnMut(&mut PortfolioCurrencyStatistics) -> EmptyResult
    {
        for statistics in &mut self.currencies {
            handler(statistics)?;
        }

        Ok(())
    }
}

pub struct PortfolioCurrencyStatistics {
    pub currency: String,

    // Use BTreeMap to get consistent metrics order
    pub assets: BTreeMap<String, Decimal>,
    pub brokers: BTreeMap<Broker, Decimal>,
    pub performance: Option<PortfolioPerformanceAnalysis>,

    pub projected_taxes: Decimal,
    pub projected_tax_deductions: Decimal,
    pub projected_commissions: Decimal,
}

impl PortfolioCurrencyStatistics {
    fn add_assets(&mut self, broker: Broker, instrument: &str, amount: Decimal) {
        *self.assets.entry(instrument.to_owned()).or_default() += amount;
        *self.brokers.entry(broker).or_default() += amount;
    }
}

pub fn analyse(
    config: &Config, portfolio_name: Option<&str>, include_closed_positions: bool,
    _asset_groups: &HashMap<String, AssetGroupConfig>,
    merge_performance: Option<&PerformanceMergingConfig>, interactive: bool,
) -> GenericResult<(PortfolioStatistics, CurrencyConverterRc, TelemetryRecordBuilder)> {
    let mut telemetry = TelemetryRecordBuilder::new();

    let mut portfolios = load_portfolios(config, portfolio_name)?;
    let multiple = portfolios.len() > 1;

    let country = config.get_tax_country();
    let (converter, quotes) = load_tools(config)?;
    let mut lto_calc = LtoDeductionCalculator::new();
    let mut statistics = PortfolioStatistics::new(country.clone());

    for (_, statement) in &mut portfolios {
        statement.batch_quotes(&quotes)?;
    }

    for (portfolio, statement) in &mut portfolios {
        let _logging_context = multiple.then(|| GlobalContext::new(&portfolio.name));

        let broker = statement.broker.type_;
        telemetry.add_broker(broker);

        if interactive {
            statement.check_date();
        }

        statistics.process(|statistics| {
            let cash_assets = statement.assets.cash.total_assets_real_time(
                &statistics.currency, &converter)?;

            Ok(statistics.add_assets(broker, "Cash", cash_assets))
        })?;

        let net_value = statement.net_value(&converter, &quotes, portfolio.currency()?, true)?;
        let mut commission_calc = CommissionCalc::new(
            converter.clone(), statement.broker.commission_spec.clone(), net_value)?;

        for (symbol, quantity) in statement.open_positions.clone() {
            let price = quotes.get(statement.get_quote_query(&symbol))?;
            statement.emulate_sell(&symbol, quantity, price, &mut commission_calc)?;
        }

        let additional_commissions = statement.emulate_commissions(commission_calc)?;
        statistics.process(|statistics| {
            let additional_commissions = additional_commissions.total_assets_real_time(
                &statistics.currency, &converter)?;

            statistics.projected_commissions += additional_commissions;
            Ok(())
        })?;

        statement.process_trades(None)?;

        for trade in statement.stock_sells.iter().rev() {
            if !trade.emulation {
                break;
            }

            let (volume, commission) = match trade.type_ {
                StockSellType::Trade {volume, commission, ..} => (volume, commission),
                _ => unreachable!(),
            };

            let (tax_year, _) = portfolio.tax_payment_day().get(trade.execution_date, true);
            let details = trade.calculate(&country, tax_year, &portfolio.tax_exemptions, &converter)?;

            let mut taxable_local_profit = details.taxable_local_profit;
            for source in &details.fifo {
                if let Some(deductible) = source.long_term_ownership_deductible {
                    lto_calc.add(deductible.profit, deductible.years, false);
                    taxable_local_profit.amount -= deductible.profit;
                }
            }

            let tax_without_deduction = country.tax_to_pay(
                IncomeType::Trading, tax_year, details.local_profit, None);

            let tax_to_pay = country.tax_to_pay(
                IncomeType::Trading, tax_year, taxable_local_profit, None);

            let tax_deduction = tax_without_deduction - tax_to_pay;
            assert!(!tax_deduction.is_negative());

            statistics.process(|statistics| {
                let currency = &statistics.currency;

                let volume = converter.real_time_convert_to(volume, currency)?;
                let commission = converter.real_time_convert_to(commission, currency)?;

                let tax_to_pay = converter.real_time_convert_to(tax_to_pay, currency)?;
                let tax_deduction = converter.real_time_convert_to(tax_deduction, currency)?;

                statistics.add_assets(broker, &trade.symbol, volume);
                statistics.projected_commissions += commission;
                statistics.projected_taxes += tax_to_pay;
                statistics.projected_tax_deductions += tax_deduction;

                Ok(())
            })?;
        }
    }

    let mut applied_lto = None;

    statistics.process(|statistics| {
        let mut analyser = PortfolioPerformanceAnalyser::new(
            &country, &statistics.currency, &converter, include_closed_positions);

        for (portfolio, statement) in &mut portfolios {
            let mut performance_merging_config = portfolio.merge_performance.clone();
            if let Some(merge_performance) = merge_performance {
                performance_merging_config.add(merge_performance)?;
            }
            analyser.add(portfolio, statement, performance_merging_config)?;
        }

        let (performance, lto) = analyser.analyse()?;
        statistics.performance.replace(performance);

        if let Some(prev) = applied_lto.take() {
            assert_eq!(prev, lto);
        }
        applied_lto.replace(lto);

        Ok(())
    })?;

    statistics.lto = Some(LtoStatistics {
        applied: applied_lto.unwrap(),
        projected: lto_calc.calculate()
    });

    Ok((statistics, converter, telemetry))
}

pub fn simulate_sell(
    config: &Config, portfolio_name: &str, positions: Option<Vec<(String, Option<Decimal>)>>,
    base_currency: Option<&str>,
) -> GenericResult<TelemetryRecordBuilder> {
    let portfolio = config.get_portfolio(portfolio_name)?;
    let statement = load_portfolio(config, portfolio, ReadingStrictness::TRADE_SETTLE_DATE)?;
    let (converter, quotes) = load_tools(config)?;

    sell_simulation::simulate_sell(
        &config.get_tax_country(), portfolio, statement,
        converter, &quotes, positions, base_currency)?;

    Ok(TelemetryRecordBuilder::new_with_broker(portfolio.broker))
}

fn load_portfolios<'a>(config: &'a Config, name: Option<&str>) -> GenericResult<Vec<(&'a PortfolioConfig, BrokerStatement)>> {
    let mut portfolios = Vec::new();
    let reading_strictness = ReadingStrictness::REPO_TRADES;

    if let Some(name) = name {
        let portfolio = config.get_portfolio(name)?;
        let statement = load_portfolio(config, portfolio, reading_strictness)?;
        portfolios.push((portfolio, statement));
    } else {
        if config.portfolios.is_empty() {
            return Err!("There is no any portfolio defined in the configuration file")
        }

        let multiple = config.portfolios.len() > 1;

        for portfolio in &config.portfolios {
            let _logging_context = multiple.then(|| GlobalContext::new(&portfolio.name));
            let statement = load_portfolio(config, portfolio, reading_strictness)?;
            portfolios.push((portfolio, statement));
        }
    }

    Ok(portfolios)
}

fn load_portfolio(config: &Config, portfolio: &PortfolioConfig, strictness: ReadingStrictness) -> GenericResult<BrokerStatement> {
    let broker = portfolio.broker.get_info(config, portfolio.plan.as_ref())?;
    BrokerStatement::read(
        broker, portfolio.statements_path()?, &portfolio.symbol_remapping, &portfolio.instrument_internal_ids,
        &portfolio.instrument_names, portfolio.get_tax_remapping()?, &portfolio.corporate_actions,
        strictness)
}

fn load_tools(config: &Config) -> GenericResult<(CurrencyConverterRc, Rc<Quotes>)> {
    let database = db::connect(&config.db_path)?;
    let quotes = Rc::new(Quotes::new(config, database.clone())?);
    let converter = CurrencyConverter::new(database, Some(quotes.clone()), false);
    Ok((converter, quotes))
}