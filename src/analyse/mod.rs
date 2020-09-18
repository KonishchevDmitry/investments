use std::collections::BTreeMap;
use std::rc::Rc;

use crate::broker_statement::BrokerStatement;
use crate::commissions::CommissionCalc;
use crate::config::{Config, PortfolioConfig, PerformanceMergingConfig};
use crate::core::{GenericResult, EmptyResult};
use crate::currency::converter::CurrencyConverter;
use crate::db;
use crate::localities;
use crate::quotes::Quotes;
use crate::types::Decimal;

use self::performance::{PortfolioPerformanceAnalyser, IncomeStructure};

pub mod deposit_emulator;
mod performance;
mod sell_simulation;

pub struct PortfolioStatistics {
    pub currencies: Vec<CurrencyStatistics>,
}

impl PortfolioStatistics {
    fn new(currencies: &[&str]) -> PortfolioStatistics {
        PortfolioStatistics {
            currencies: currencies.iter().map(|&currency| (
                CurrencyStatistics {
                    currency: currency.to_owned(),

                    assets: BTreeMap::new(),
                    performance: BTreeMap::new(),
                    income_structure: Default::default(), // FIXME(konishchev): Change it?
                    assets_after_sellout: BTreeMap::new(),

                    total_value: dec!(0),
                    expected_taxes: dec!(0),
                    expected_commissions: dec!(0),
                }
            )).collect(),
        }
    }

    fn process<F>(&mut self, mut handler: F) -> EmptyResult
        where F: FnMut(&mut CurrencyStatistics) -> EmptyResult
    {
        for statistics in &mut self.currencies {
            handler(statistics)?;
        }

        Ok(())
    }
}

pub struct CurrencyStatistics {
    pub currency: String,

    pub assets: BTreeMap<String, Decimal>,
    pub performance: BTreeMap<String, Decimal>,
    pub income_structure: IncomeStructure,
    pub assets_after_sellout: BTreeMap<String, Decimal>, // FIXME(konishchev): Deprecate?

    pub total_value: Decimal, // FIXME(konishchev): Deprecate?
    pub expected_taxes: Decimal,
    pub expected_commissions: Decimal,
}

impl CurrencyStatistics {
    const CASH: &'static str = "cash";
    const PORTFOLIO: &'static str = "portfolio";

    fn add_assets(&mut self, instrument: &str, amount: Decimal) {
        *self.assets.entry(instrument.to_owned()).or_default() += amount;
    }

    fn add_assets_after_sellout(&mut self, instrument: &str, amount: Decimal) {
        *self.assets_after_sellout.entry(instrument.to_owned()).or_default() += amount;
    }
}

pub fn analyse(
    config: &Config, portfolio_name: Option<&str>, include_closed_positions: bool,
    merge_performance: Option<&PerformanceMergingConfig>, interactive: bool,
) -> GenericResult<(PortfolioStatistics, CurrencyConverter)> {
    let mut portfolios = load_portfolios(config, portfolio_name)?;

    let currencies = ["USD", "RUB"];
    let country = localities::russia();
    let (converter, quotes) = load_tools(config)?;
    let mut statistics = PortfolioStatistics::new(&currencies);

    for (_, statement) in &mut portfolios {
        statement.batch_quotes(&quotes);
    }

    for (portfolio, statement) in &mut portfolios {
        if interactive {
            statement.check_date();
        }

        statistics.process(|statistics| {
            let cash_assets = statement.cash_assets.total_assets_real_time(&statistics.currency, &converter)?;
            statistics.add_assets(CurrencyStatistics::CASH, cash_assets);
            statistics.add_assets_after_sellout(CurrencyStatistics::CASH, cash_assets);
            statistics.total_value += cash_assets;
            Ok(())
        })?;

        let mut commission_calc = CommissionCalc::new(statement.broker.commission_spec.clone());

        for (symbol, quantity) in statement.open_positions.clone() {
            let price = quotes.get(&symbol)?;
            statement.emulate_sell(&symbol, quantity, price, &mut commission_calc)?;
        }

        let additional_commissions = statement.emulate_commissions(commission_calc);
        statement.process_trades()?;

        for trade in statement.stock_sells.iter().rev() {
            if !trade.emulation {
                break;
            }

            let details = trade.calculate(&country, &converter)?;

            statistics.process(|statistics| {
                let currency = &statistics.currency;
                let volume = converter.real_time_convert_to(trade.volume, currency)?;
                let commission = converter.real_time_convert_to(trade.commission, currency)?;
                let tax_to_pay = converter.real_time_convert_to(details.tax_to_pay, currency)?;

                statistics.add_assets_after_sellout(&trade.symbol, volume - commission - tax_to_pay);
                statistics.add_assets(&trade.symbol, volume);
                statistics.expected_commissions += commission;
                statistics.expected_taxes += tax_to_pay;

                Ok(())
            })?;
        }

        statistics.process(|statistics| {
            let commissions = additional_commissions.total_assets_real_time(&statistics.currency, &converter)?;
            statistics.add_assets_after_sellout(CurrencyStatistics::CASH, -commissions);
            statistics.expected_commissions += commissions;
            Ok(())
        })?;

        // FIXME(konishchev): Optimize
        statement.merge_symbols(&portfolio.merge_performance, true).map_err(|e| format!(
            "Invalid performance merging configuration: {}", e))?;

        if let Some(merge_performance) = merge_performance {
            statement.merge_symbols(merge_performance, false)?;
        }
    }

    statistics.process(|statistics| {
        let mut analyser = PortfolioPerformanceAnalyser::new(
            country, &statistics.currency, &converter, include_closed_positions, interactive);

        for (portfolio, statement) in &mut portfolios {
            analyser.add(&portfolio, &statement)?;
        }

        let (portfolio_performance, mut instrument_performance, income_structure) = analyser.analyse()?;

        let portfolio_symbol = CurrencyStatistics::PORTFOLIO;
        if instrument_performance.insert(portfolio_symbol.to_owned(), portfolio_performance).is_some() {
            return Err!("Got an unexpected symbol: {:?}", portfolio_symbol)
        }

        statistics.performance = instrument_performance;
        statistics.income_structure = income_structure;

        Ok(())
    })?;

    Ok((statistics, converter))
}

pub fn simulate_sell(config: &Config, portfolio_name: &str, positions: &[(String, Option<Decimal>)]) -> EmptyResult {
    let portfolio = config.get_portfolio(portfolio_name)?;
    let statement = load_portfolio(config, portfolio, true)?;
    let (converter, quotes) = load_tools(config)?;
    sell_simulation::simulate_sell(portfolio, statement, &converter, &quotes, positions)
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
        portfolio.get_tax_remapping()?, strict_mode)
}

fn load_tools(config: &Config) -> GenericResult<(CurrencyConverter, Rc<Quotes>)> {
    let database = db::connect(&config.db_path)?;
    let quotes = Rc::new(Quotes::new(&config, database.clone())?);
    let converter = CurrencyConverter::new(database, Some(quotes.clone()), false);
    Ok((converter, quotes))
}