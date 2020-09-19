use std::collections::{HashMap, BTreeMap};

use log::{self, log_enabled, trace};
use num_traits::Zero;

use crate::broker_statement::BrokerStatement;
use crate::config::PortfolioConfig;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
use crate::formatting;
use crate::localities::Country;
use crate::taxes::NetTaxCalculator;
use crate::types::{Date, Decimal};
use crate::util;

use super::deposit_emulator::{Transaction, InterestPeriod};
use super::deposit_performance;
use super::portfolio_analysis::{PortfolioPerformanceAnalysis, InstrumentPerformanceAnalysis};

// FIXME(konishchev): Split into submodules
// FIXME(konishchev): Check all usage
#[derive(Default)]
pub struct IncomeStructure {
    balance: Decimal, // FIXME(konishchev): Get rid of it?

    pub net_profit: Decimal,

    pub trading_profit: Decimal,
    pub dividends: Decimal,
    pub interest: Decimal,

    pub commissions: Decimal,
    pub tax_deductions: Decimal,
    pub taxes: Decimal,
}

/// Calculates average rate of return from cash investments by comparing portfolio performance to
/// performance of a bank deposit with exactly the same investments and monthly capitalization.
pub struct PortfolioPerformanceAnalyser<'a> {
    today: Date,
    country: Country,
    currency: &'a str,
    converter: &'a CurrencyConverter,
    include_closed_positions: bool,

    transactions: Vec<Transaction>,
    income_structure: IncomeStructure,
    instruments: Option<BTreeMap<String, StockDepositView>>,
    current_assets: Decimal,
}

impl <'a> PortfolioPerformanceAnalyser<'a> {
    pub fn new(
        country: Country, currency: &'a str, converter: &'a CurrencyConverter,
        include_closed_positions: bool,
    ) -> PortfolioPerformanceAnalyser<'a> {
        PortfolioPerformanceAnalyser {
            today: util::today(),
            country,
            currency,
            converter,
            include_closed_positions,

            transactions: Vec::new(),
            income_structure: Default::default(),
            instruments: Some(BTreeMap::new()),
            current_assets: dec!(0),
        }
    }

    pub fn add(&mut self, portfolio: &PortfolioConfig, statement: &BrokerStatement) -> EmptyResult {
        // Assume that the caller has simulated sellout and just check it here
        if !statement.open_positions.is_empty() {
            return Err!(
                "Unable to calculate current assets: The broker statement has open positions");
        }

        trace!("Deposit emulator transactions for {:?}:", portfolio.name);
        self.process_deposits_and_withdrawals(statement)?;
        self.process_positions(statement, portfolio)?;
        self.process_dividends(statement, portfolio)?;
        self.process_interest(statement, portfolio)?;
        self.process_tax_deductions(portfolio)?;

        // FIXME(konishchev): Get rid of it?
        self.process_cash_assets(statement)?;

        for (symbol, deposit_view) in self.instruments.as_mut().unwrap().iter_mut() {
            if deposit_view.name.is_none() {
                deposit_view.name.replace(statement.get_instrument_name(&symbol));
            }
        }

        Ok(())
    }

    pub fn analyse(mut self) -> GenericResult<(PortfolioPerformanceAnalysis, IncomeStructure)> {
        let mut instruments = BTreeMap::new();

        self.calculate_open_position_periods()?;

        for (symbol, deposit_view) in self.instruments.take().unwrap() {
            if deposit_view.closed && !self.include_closed_positions {
                continue;
            }

            let analysis = self.analyse_instrument_performance(&symbol, deposit_view)?;
            assert!(instruments.insert(symbol, analysis).is_none());
        }

        Ok((PortfolioPerformanceAnalysis {
            instruments,
            portfolio: self.analyse_portfolio_performance()?,
        }, self.income_structure))
    }

    fn analyse_instrument_performance(
        &mut self, symbol: &str, mut deposit_view: StockDepositView
    ) -> GenericResult<InstrumentPerformanceAnalysis> {
        deposit_view.transactions.sort_by_key(|transaction| transaction.date);

        let (interest, difference) = deposit_performance::compare_to_bank_deposit(
            &deposit_view.transactions, &deposit_view.interest_periods, dec!(0))?;

        deposit_performance::check_emulation_precision(
            symbol, self.currency, deposit_view.last_sell_volume.unwrap(), difference)?;

        let name = deposit_view.name.unwrap();
        let days = get_total_activity_duration(&deposit_view.interest_periods);

        let mut investments = dec!(0);
        let mut result = dec!(0);

        for transaction in &deposit_view.transactions {
            if transaction.amount.is_sign_positive() {
                investments += transaction.amount;
            } else {
                result += -transaction.amount;
            }
        }

        Ok(InstrumentPerformanceAnalysis {
            name, days, investments, result, interest,
            inactive: deposit_view.closed,
        })
    }

    fn analyse_portfolio_performance(&mut self) -> GenericResult<InstrumentPerformanceAnalysis> {
        if self.transactions.is_empty() {
            return Err!("The portfolio has no activity yet");
        }

        self.transactions.sort_by_key(|transaction| transaction.date);

        let activity_periods = vec![InterestPeriod::new(
            self.transactions.first().unwrap().date, self.today)];

        let (interest, difference) = deposit_performance::compare_to_bank_deposit(
            &self.transactions, &activity_periods, self.current_assets)?;

        deposit_performance::check_emulation_precision(
            "portfolio", self.currency, self.current_assets, difference)?;

        let days = get_total_activity_duration(&activity_periods);
        let investments = self.transactions.iter()
            .map(|transaction| transaction.amount)
            .sum();

        self.income_structure.net_profit = self.current_assets - investments;
        self.income_structure.trading_profit = self.income_structure.net_profit
            + self.income_structure.taxes
            + self.income_structure.commissions
            - self.income_structure.tax_deductions
            - self.income_structure.dividends
            - self.income_structure.interest;

        Ok(InstrumentPerformanceAnalysis {
            name: s!("Portfolio"),
            days, investments,
            result: self.current_assets,
            interest,
            inactive: false
        })
    }

    fn calculate_open_position_periods(&mut self) -> EmptyResult {
        struct OpenPosition {
            start_date: Date,
            quantity: Decimal,
        }

        trace!("Open positions periods:");

        for (symbol, deposit_view) in self.instruments.as_mut().unwrap() {
            if deposit_view.trades.is_empty() {
                return Err!("Got an unexpected transaction for {} which has no trades", symbol)
            }

            let mut open_position = None;

            for (&date, &quantity) in &deposit_view.trades {
                let current = open_position.get_or_insert_with(|| {
                    OpenPosition {
                        start_date: date,
                        quantity: dec!(0),
                    }
                });
                current.quantity += quantity;

                if current.quantity > dec!(0) {
                    continue;
                } else if current.quantity < dec!(0) {
                    return Err!(
                        "Error while processing {} sell operations: Got a negative balance on {}",
                        symbol, formatting::format_date(date));
                }

                let start_date = current.start_date;
                let end_date = if date == start_date {
                    date.succ()
                } else {
                    date
                };

                match deposit_view.interest_periods.last_mut() {
                    Some(ref mut period) if period.end >= start_date => {
                        assert_eq!(period.end, start_date);
                        assert!(period.end < end_date);
                        period.end = end_date;
                    },
                    _ => deposit_view.interest_periods.push(InterestPeriod::new(start_date, end_date)),
                };

                open_position = None;
            }

            if open_position.is_some() {
                return Err!(
                    "The portfolio contains unsold {} stocks when sellout simulation is expected",
                    symbol);
            }
            assert!(!deposit_view.interest_periods.is_empty());

            if log_enabled!(log::Level::Trace) {
                let periods = deposit_view.interest_periods.iter()
                    .map(|period| format!(
                        "{} - {}", formatting::format_date(period.start),
                        formatting::format_date(period.end)))
                    .collect::<Vec<_>>()
                    .join(", ");

                trace!("* {}: {}", symbol, periods);
            }
        }

        Ok(())
    }

    fn process_deposits_and_withdrawals(&mut self, statement: &BrokerStatement) -> EmptyResult {
        for mut cash_flow in statement.cash_flows.iter().cloned() {
            if cash_flow.cash.is_positive() {
                cash_flow.cash.amount += statement.broker.get_deposit_commission(cash_flow)?;
            }

            let amount = self.converter.convert_to(cash_flow.date, cash_flow.cash, self.currency)?;

            trace!("* {} {}: {}", if amount.is_sign_positive() {
                "Deposit"
            } else {
                "Withdrawal"
            }, formatting::format_date(cash_flow.date), amount.normalize());

            self.transaction(cash_flow.date, amount);
            self.income_structure.balance += amount;
        }

        Ok(())
    }

    fn process_positions(&mut self, statement: &BrokerStatement, portfolio: &PortfolioConfig) -> EmptyResult {
        let mut taxes = NetTaxCalculator::new(self.country, portfolio.tax_payment_day);
        let mut stock_taxes = HashMap::new();

        for stock_buy in &statement.stock_buys {
            let multiplier = statement.stock_splits.get_multiplier(
                &stock_buy.symbol, stock_buy.conclusion_date, self.today);

            let commission = self.converter.convert_to(
                stock_buy.conclusion_date, stock_buy.commission, self.currency)?;

            let mut assets = self.converter.convert_to(
                stock_buy.execution_date, stock_buy.volume, self.currency)?;
            assets += commission;

            let deposit_view = self.get_deposit_view(&stock_buy.symbol);
            deposit_view.trade(stock_buy.conclusion_date, multiplier * stock_buy.quantity);
            deposit_view.transaction(stock_buy.conclusion_date, assets);

            self.income_structure.commissions += commission;
        }

        for stock_sell in &statement.stock_sells {
            let multiplier = statement.stock_splits.get_multiplier(
                &stock_sell.symbol, stock_sell.conclusion_date, self.today);

            let assets = self.converter.convert_to(
                stock_sell.execution_date, stock_sell.volume, self.currency)?;

            let commission = self.converter.convert_to(
                stock_sell.conclusion_date, stock_sell.commission, self.currency)?;

            {
                let deposit_view = self.get_deposit_view(&stock_sell.symbol);

                deposit_view.trade(stock_sell.conclusion_date, multiplier * -stock_sell.quantity);
                deposit_view.transaction(stock_sell.conclusion_date, -assets);
                deposit_view.transaction(stock_sell.conclusion_date, commission);

                deposit_view.last_sell_volume.replace(assets);
                if stock_sell.emulation {
                    deposit_view.closed = false;
                }
            }

            let local_profit = stock_sell.calculate(&self.country, self.converter)?.local_profit.amount;

            stock_taxes.entry(&stock_sell.symbol)
                .or_insert_with(|| NetTaxCalculator::new(self.country, portfolio.tax_payment_day))
                .add_profit(stock_sell.execution_date, local_profit);

            taxes.add_profit(stock_sell.execution_date, local_profit);

            self.income_structure.commissions += commission;
        }

        for (&symbol, symbol_taxes) in stock_taxes.iter() {
            for (&tax_payment_date, &tax_to_pay) in symbol_taxes.get_taxes().iter() {
                if let Some(deposit_amount) = self.map_tax_to_deposit_amount(tax_payment_date, tax_to_pay)? {
                    trace!("* {} selling {} tax: {}",
                           symbol, formatting::format_date(tax_payment_date), deposit_amount);

                    self.get_deposit_view(symbol).transaction(tax_payment_date, deposit_amount);
                    self.income_structure.taxes += deposit_amount;
                }
            }
        }

        for (&tax_payment_date, &tax_to_pay) in taxes.get_taxes().iter() {
            if let Some(deposit_amount) = self.map_tax_to_deposit_amount(tax_payment_date, tax_to_pay)? {
                trace!("* Stock selling {} tax: {}",
                       formatting::format_date(tax_payment_date), deposit_amount);
                self.transaction(tax_payment_date, deposit_amount);
                self.income_structure.balance += deposit_amount;
                self.income_structure.taxes += deposit_amount;
            }
        }

        Ok(())
    }

    fn process_dividends(&mut self, statement: &BrokerStatement, portfolio: &PortfolioConfig) -> EmptyResult {
        for dividend in &statement.dividends {
            let profit = dividend.amount.sub(dividend.paid_tax).map_err(|e| format!(
                "{}: The tax is paid in currency different from the dividend currency: {}",
                dividend.description(), e))?;

            let profit = self.converter.convert_to(dividend.date, profit, self.currency)?;
            self.get_deposit_view(&dividend.issuer).transaction(dividend.date, -profit);
            self.income_structure.dividends += profit; // FIXME(konishchev): Use full amount?

            let tax_to_pay = dividend.tax_to_pay(&self.country, self.converter)?;
            let tax_payment_date = portfolio.tax_payment_day.get(dividend.date);

            if let Some(deposit_amount) = self.map_tax_to_deposit_amount(tax_payment_date, tax_to_pay)? {
                trace!("* {} {} dividend {} tax: {}",
                       dividend.issuer, formatting::format_date(dividend.date),
                       formatting::format_date(tax_payment_date), deposit_amount);

                self.get_deposit_view(&dividend.issuer).transaction(tax_payment_date, deposit_amount);
                self.transaction(tax_payment_date, deposit_amount);
                self.income_structure.balance += deposit_amount;
                self.income_structure.taxes += deposit_amount;
            }
        }

        Ok(())
    }

    fn process_interest(&mut self, statement: &BrokerStatement, portfolio: &PortfolioConfig) -> EmptyResult {
        for interest in &statement.idle_cash_interest {
            self.income_structure.interest += self.converter.convert_to(
                interest.date, interest.amount, self.currency)?;

            let tax_to_pay = interest.tax_to_pay(&self.country, self.converter)?;
            let tax_payment_date = portfolio.tax_payment_day.get(interest.date);

            if let Some(deposit_amount) = self.map_tax_to_deposit_amount(tax_payment_date, tax_to_pay)? {
                trace!("* {} idle cash interest {} tax: {}",
                       formatting::format_date(interest.date),
                       formatting::format_date(tax_payment_date), deposit_amount);

                self.transaction(tax_payment_date, deposit_amount);
                self.income_structure.balance += deposit_amount;
                self.income_structure.taxes += deposit_amount;
            }
        }

        Ok(())
    }

    fn process_tax_deductions(&mut self, portfolio: &PortfolioConfig) -> EmptyResult {
        for &(date, amount) in &portfolio.tax_deductions {
            let amount = self.converter.convert(self.country.currency, self.currency, date, amount)?;
            trace!("* Tax deduction {}: {}", formatting::format_date(date), -amount);
            self.transaction(date, -amount);
            self.income_structure.balance -= amount;
            self.income_structure.tax_deductions += amount;
        }

        Ok(())
    }

    fn process_cash_assets(&mut self, statement: &BrokerStatement) -> EmptyResult {
        let cash_assets = statement.cash_assets.total_assets_real_time(
            self.currency, self.converter)?;

        self.current_assets += cash_assets;

        Ok(())
    }

    fn get_deposit_view(&mut self, symbol: &str) -> &mut StockDepositView {
        self.instruments.as_mut().unwrap()
            .entry(symbol.to_owned())
            .or_insert_with(StockDepositView::new)
    }

    fn transaction(&mut self, date: Date, amount: Decimal) {
        self.transactions.push(Transaction::new(date, amount));
    }

    fn map_tax_to_deposit_amount(&self, tax_payment_date: Date, tax_to_pay: Decimal) -> GenericResult<Option<Decimal>> {
        // Treat tax payment as an ordinary deposit which we transfer to the account at tax payment
        // day.

        if tax_to_pay.is_zero() {
            return Ok(None);
        }
        assert!(tax_to_pay.is_sign_positive());

        let tax_to_pay = Cash::new(self.country.currency, tax_to_pay);

        let conversion_date = if tax_payment_date > self.today {
            self.today
        } else {
            tax_payment_date
        };

        Ok(Some(self.converter.convert_to(conversion_date, tax_to_pay, self.currency)?))
    }
}

struct StockDepositView {
    name: Option<String>,
    trades: BTreeMap<Date, Decimal>,
    transactions: Vec<Transaction>,
    interest_periods: Vec<InterestPeriod>,
    last_sell_volume: Option<Decimal>,
    closed: bool,
}

impl StockDepositView {
    fn new() -> StockDepositView {
        StockDepositView {
            name: None,
            trades: BTreeMap::new(),
            transactions: Vec::new(),
            interest_periods: Vec::new(),
            last_sell_volume: None,
            closed: true,
        }
    }

    fn trade(&mut self, date: Date, quantity: Decimal) {
        self.trades.entry(date)
            .and_modify(|total| *total += quantity)
            .or_insert(quantity);
    }

    fn transaction(&mut self, date: Date, amount: Decimal) {
        self.transactions.push(Transaction::new(date, amount))
    }
}

fn get_total_activity_duration(periods: &[InterestPeriod]) -> u32 {
    periods.iter().map(InterestPeriod::days).sum()
}