use std::collections::BTreeMap;
use std::rc::Rc;

use crate::brokers::Broker;
use crate::broker_statement::BrokerStatement;
use crate::commissions::CommissionCalc;
use crate::config::{Config, PortfolioConfig, PerformanceMergingConfig};
use crate::core::{GenericResult, EmptyResult};
use crate::currency::converter::{CurrencyConverter, CurrencyConverterRc};
use crate::db;
use crate::quotes::Quotes;
use crate::types::Decimal;

use self::portfolio_analysis::PortfolioPerformanceAnalysis;
use self::portfolio_performance::PortfolioPerformanceAnalyser;

pub mod deposit_emulator;
mod deposit_performance;
mod portfolio_analysis;
mod portfolio_performance;
mod sell_simulation;

pub struct PortfolioStatistics {
    pub currencies: Vec<PortfolioCurrencyStatistics>,
}

impl PortfolioStatistics {
    fn new() -> PortfolioStatistics {
        PortfolioStatistics {
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
        }
    }

    pub fn print(&self) {
        for statistics in &self.currencies {
            statistics.performance.as_ref().unwrap().print(&format!(
                "Average rate of return from cash investments in {}", &statistics.currency));
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
    merge_performance: Option<&PerformanceMergingConfig>, interactive: bool,
) -> GenericResult<(PortfolioStatistics, CurrencyConverterRc)> {
    let mut portfolios = load_portfolios(config, portfolio_name)?;

    let country = config.get_tax_country();
    let (converter, quotes) = load_tools(config)?;
    let mut statistics = PortfolioStatistics::new();

    for (_, statement) in &mut portfolios {
        statement.batch_quotes(&quotes)?;
    }

    for (portfolio, statement) in &mut portfolios {
        let broker = statement.broker.type_;

        if interactive {
            statement.check_date();
        }

        statistics.process(|statistics| {
            let cash_assets = statement.cash_assets.total_assets_real_time(
                &statistics.currency, &converter)?;

            Ok(statistics.add_assets(broker, "Cash", cash_assets))
        })?;

        let net_value = statement.net_value(&converter, &quotes, portfolio.currency()?)?;
        let mut commission_calc = CommissionCalc::new(
            converter.clone(), statement.broker.commission_spec.clone(), net_value)?;

        for (symbol, quantity) in statement.open_positions.clone() {
            let price = quotes.get(&symbol)?;
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

        // FIXME(konishchev): HERE
        for trade in statement.stock_sells.iter().rev() {
            if !trade.emulation {
                break;
            }

            let (tax_year, _) = portfolio.tax_payment_day().get(trade.execution_date, true);
            let details = trade.calculate(&country, tax_year, &portfolio.tax_exemptions, &converter)?;

            statistics.process(|statistics| {
                let currency = &statistics.currency;
                let volume = converter.real_time_convert_to(trade.volume, currency)?;
                let commission = converter.real_time_convert_to(trade.commission, currency)?;
                let tax_to_pay = converter.real_time_convert_to(details.tax_to_pay, currency)?;
                let tax_deduction = converter.real_time_convert_to(details.tax_deduction, currency)?;

                statistics.add_assets(broker, &trade.symbol, volume);
                statistics.projected_commissions += commission;
                statistics.projected_taxes += tax_to_pay;
                statistics.projected_tax_deductions += tax_deduction;

                Ok(())
            })?;
        }

        if !portfolio.merge_performance.is_empty() {
            statement.merge_symbols(&portfolio.merge_performance, true).map_err(|e| format!(
                "Invalid performance merging configuration: {}", e))?;
        }

        if let Some(merge_performance) = merge_performance {
            if !merge_performance.is_empty() {
                statement.merge_symbols(merge_performance, false)?;
            }
        }
    }

    statistics.process(|statistics| {
        let mut analyser = PortfolioPerformanceAnalyser::new(
            &country, &statistics.currency, &converter, include_closed_positions);

        for (portfolio, statement) in &mut portfolios {
            analyser.add(&portfolio, &statement)?;
        }

        statistics.performance.replace(analyser.analyse()?);
        Ok(())
    })?;

    Ok((statistics, converter))
}

pub fn simulate_sell(
    config: &Config, portfolio_name: &str, positions: Vec<(String, Option<Decimal>)>,
    base_currency: Option<&str>,
) -> EmptyResult {
    let portfolio = config.get_portfolio(portfolio_name)?;
    let statement = load_portfolio(config, portfolio, true)?;
    let (converter, quotes) = load_tools(config)?;

    sell_simulation::simulate_sell(
        &config.get_tax_country(), portfolio, statement,
        converter, &quotes, positions, base_currency)
}

fn load_portfolios<'a>(config: &'a Config, name: Option<&str>) -> GenericResult<Vec<(&'a PortfolioConfig, BrokerStatement)>> {
    let mut portfolios = Vec::new();

    if let Some(name) = name {
        let portfolio = config.get_portfolio(name)?;
        let statement = load_portfolio(config, portfolio, false)?;
        portfolios.push((portfolio, statement));
    } else {
        if config.portfolios.is_empty() {
            return Err!("There is no any portfolio defined in the configuration file")
        }

        for portfolio in &config.portfolios {
            let statement = load_portfolio(config, portfolio, false)?;
            portfolios.push((portfolio, statement));
        }
    }

    Ok(portfolios)
}

fn load_portfolio(config: &Config, portfolio: &PortfolioConfig, strict_mode: bool) -> GenericResult<BrokerStatement> {
    let broker = portfolio.broker.get_info(config, portfolio.plan.as_ref())?;
    BrokerStatement::read(
        broker, &portfolio.statements, &portfolio.symbol_remapping, &portfolio.instrument_names,
        portfolio.get_tax_remapping()?, &portfolio.corporate_actions, strict_mode)
}

fn load_tools(config: &Config) -> GenericResult<(CurrencyConverterRc, Rc<Quotes>)> {
    let database = db::connect(&config.db_path)?;
    let quotes = Rc::new(Quotes::new(&config, database.clone())?);
    let converter = CurrencyConverter::new(database, Some(quotes.clone()), false);
    Ok((converter, quotes))
}