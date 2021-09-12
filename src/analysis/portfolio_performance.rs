use std::cmp::Ordering;
use std::collections::{HashMap, BTreeMap};

use log::{self, log_enabled, trace};
use num_traits::Zero;

use crate::broker_statement::{BrokerStatement, StockSource, StockSellType};
use crate::config::PortfolioConfig;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
use crate::formatting;
use crate::localities::Country;
use crate::taxes::{NetTax, NetTaxCalculator, NetLtoDeduction, NetLtoDeductionCalculator};
use crate::time::{self, Date, DateOptTime};
use crate::types::Decimal;

use super::deposit_emulator::{Transaction, InterestPeriod};
use super::deposit_performance;
use super::portfolio_analysis::{
    PortfolioPerformanceAnalysis, InstrumentPerformanceAnalysis, IncomeStructure};

/// Calculates average rate of return from cash investments by comparing portfolio performance to
/// performance of a bank deposit with exactly the same investments and monthly capitalization.
pub struct PortfolioPerformanceAnalyser<'a> {
    today: Date,
    country: &'a Country,
    currency: &'a str,
    converter: &'a CurrencyConverter,
    include_closed_positions: bool,

    transactions: Vec<Transaction>,
    income_structure: IncomeStructure,
    instruments: Option<BTreeMap<String, StockDepositView>>,
    net_lto_calc: NetLtoDeductionCalculator,
    current_assets: Decimal,
}

impl <'a> PortfolioPerformanceAnalyser<'a> {
    pub fn new(
        country: &'a Country, currency: &'a str, converter: &'a CurrencyConverter,
        include_closed_positions: bool,
    ) -> PortfolioPerformanceAnalyser<'a> {
        PortfolioPerformanceAnalyser {
            today: time::today(),
            country,
            currency,
            converter,
            include_closed_positions,

            transactions: Vec::new(),
            income_structure: Default::default(),
            instruments: Some(BTreeMap::new()),
            net_lto_calc: NetLtoDeductionCalculator::new(),
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
        self.process_tax_agent_withholdings(statement)?;
        self.process_tax_deductions(portfolio)?;
        self.process_cash_assets(statement)?;

        for (symbol, deposit_view) in self.instruments.as_mut().unwrap().iter_mut() {
            if deposit_view.name.is_none() {
                deposit_view.name.replace(statement.instrument_info.get_name(symbol));
            }
        }

        Ok(())
    }

    pub fn analyse(mut self) -> GenericResult<(PortfolioPerformanceAnalysis, BTreeMap<i32, NetLtoDeduction>)> {
        let mut instrument_performance = BTreeMap::new();

        self.calculate_open_position_periods()?;

        for (symbol, deposit_view) in self.instruments.take().unwrap() {
            if deposit_view.closed && !self.include_closed_positions {
                continue;
            }

            let analysis = self.analyse_instrument_performance(&symbol, deposit_view)?;
            assert!(instrument_performance.insert(symbol, analysis).is_none());
        }

        let portfolio_performance = self.analyse_portfolio_performance()?;
        self.income_structure.net_profit = portfolio_performance.net_profit();

        Ok((PortfolioPerformanceAnalysis {
            income_structure: self.income_structure,
            instruments: instrument_performance,
            portfolio: portfolio_performance,
        }, self.net_lto_calc.calculate()))
    }

    fn analyse_instrument_performance(
        &mut self, symbol: &str, mut deposit_view: StockDepositView
    ) -> GenericResult<InstrumentPerformanceAnalysis> {
        deposit_view.transactions.sort_by_key(|transaction| transaction.date);

        let interest = deposit_performance::compare_to_bank_deposit(
            &deposit_view.transactions, &deposit_view.interest_periods, dec!(0),
        ).map(|(interest, difference)| -> GenericResult<Decimal> {
            deposit_performance::check_emulation_precision(
                symbol, self.currency, &deposit_view.transactions,
                dec!(0), difference)?;
            Ok(interest)
        }).transpose()?;

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

        let interest = deposit_performance::compare_to_bank_deposit(
            &self.transactions, &activity_periods, self.current_assets,
        ).map(|(interest, difference)| -> GenericResult<Decimal> {
            deposit_performance::check_emulation_precision(
                "portfolio", self.currency, &self.transactions,
                self.current_assets, difference)?;
            Ok(interest)
        }).transpose()?;

        let days = get_total_activity_duration(&activity_periods);
        let investments = self.transactions.iter()
            .map(|transaction| transaction.amount)
            .sum();

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

                match current.quantity.cmp(&Decimal::zero()) {
                    Ordering::Greater => continue,
                    Ordering::Equal => {},
                    Ordering::Less => {
                        return Err!(
                            "Error while processing {} sell operations: Got a negative balance on {}",
                            symbol, formatting::format_date(date));
                    }
                };

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
        for mut assets in statement.deposits_and_withdrawals.iter().cloned() {
            if assets.cash.is_positive() {
                assets.cash.amount += statement.broker.get_deposit_commission(assets)?;
            }

            let amount = self.converter.convert_to(assets.date, assets.cash, self.currency)?;

            trace!("* {} {}: {}", if amount.is_sign_positive() {
                "Deposit"
            } else {
                "Withdrawal"
            }, formatting::format_date(assets.date), amount.normalize());

            self.transaction(assets.date, amount);
        }

        Ok(())
    }

    fn process_positions(&mut self, statement: &BrokerStatement, portfolio: &PortfolioConfig) -> EmptyResult {
        let mut taxes = NetTaxCalculator::new(self.country.clone(), portfolio.tax_payment_day());
        let mut stock_taxes = HashMap::new();

        for trade in &statement.stock_buys {
            let multiplier = statement.stock_splits.get_multiplier(
                &trade.symbol, trade.conclusion_time, DateOptTime::new_max_time(self.today));
            let quantity = multiplier * trade.quantity;

            match trade.type_ {
                StockSource::Trade {volume, commission, ..} => {
                    let volume = self.converter.convert_to(
                        trade.execution_date, volume, self.currency)?;

                    let commission = self.converter.convert_to(
                        trade.conclusion_time.date, commission, self.currency)?;
                    self.income_structure.commissions += commission;

                    let deposit_view = self.get_deposit_view(&trade.symbol);
                    deposit_view.trade(trade.conclusion_time, quantity);
                    deposit_view.transaction(trade.conclusion_time, volume);
                    deposit_view.transaction(trade.conclusion_time, commission);
                },
                StockSource::CorporateAction => {
                    let deposit_view = self.get_deposit_view(&trade.symbol);
                    deposit_view.trade(trade.conclusion_time, quantity);
                },
            };
        }

        for trade in &statement.stock_sells {
            let multiplier = statement.stock_splits.get_multiplier(
                &trade.symbol, trade.conclusion_time, DateOptTime::new_max_time(self.today));
            let quantity = multiplier * trade.quantity;

            match trade.type_ {
                StockSellType::Trade {volume, commission, ..} => {
                    let volume = self.converter.convert_to(
                        trade.execution_date, volume, self.currency)?;

                    let commission = self.converter.convert_to(
                        trade.conclusion_time.date, commission, self.currency)?;
                    self.income_structure.commissions += commission;

                    {
                        let deposit_view = self.get_deposit_view(&trade.symbol);

                        deposit_view.trade(trade.conclusion_time, -quantity);
                        deposit_view.transaction(trade.conclusion_time, -volume);
                        deposit_view.transaction(trade.conclusion_time, commission);

                        if trade.emulation {
                            deposit_view.closed = false;
                        }
                    }

                    let (tax_year, _) = portfolio.tax_payment_day().get(trade.execution_date, true);
                    let details = trade.calculate(self.country, tax_year, &portfolio.tax_exemptions, self.converter)?;

                    let mut lto_deductibles = Vec::new();

                    for fifo in &details.fifo {
                        if let Some(lto) = fifo.long_term_ownership_deductible {
                            self.net_lto_calc.add_profit(tax_year, lto.profit, lto.years, trade.emulation);
                            lto_deductibles.push(lto);
                        }
                    }

                    stock_taxes.entry(&trade.symbol)
                        .or_insert_with(|| NetTaxCalculator::new(
                            self.country.clone(), portfolio.tax_payment_day()))
                        .add_profit(
                            trade.execution_date, details.local_profit, details.taxable_local_profit,
                            &lto_deductibles, trade.emulation);

                    taxes.add_profit(
                        trade.execution_date, details.local_profit, details.taxable_local_profit,
                        &lto_deductibles, trade.emulation);
                },
                StockSellType::CorporateAction => {
                    let deposit_view = self.get_deposit_view(&trade.symbol);
                    deposit_view.trade(trade.conclusion_time, -quantity);
                },
            };
        }

        for (symbol, symbol_taxes) in stock_taxes.into_iter() {
            for (_, NetTax{tax_payment_date, tax_to_pay, ..}) in symbol_taxes.calculate().into_iter() {
                if let Some(amount) = self.map_tax_to_deposit_amount(tax_payment_date, tax_to_pay)? {
                    trace!("* {} selling {} tax: {}",
                           symbol, formatting::format_date(tax_payment_date), amount);

                    self.get_deposit_view(symbol).transaction(tax_payment_date.into(), amount);
                }
            }
        }

        for (tax_year, NetTax {
            tax_payment_date, tax_to_pay, tax_deduction,
            lto_deduction, lto_loss,
        }) in taxes.calculate().into_iter() {
            if let Some(amount) = self.map_tax_to_deposit_amount(tax_payment_date, tax_to_pay)? {
                trace!("* Stock selling {} tax: {}", formatting::format_date(tax_payment_date), amount);
                self.transaction(tax_payment_date, amount);
                self.income_structure.taxes += amount;
            }

            if let Some(amount) = self.map_tax_to_deposit_amount(tax_payment_date, tax_deduction)? {
                trace!("* {} tax deduction: {}", formatting::format_date(tax_payment_date), amount);
                self.income_structure.tax_deductions += amount;
            }

            self.net_lto_calc.add_applied_deduction(tax_year, lto_deduction.amount, lto_loss.amount);
        }

        Ok(())
    }

    fn process_dividends(&mut self, statement: &BrokerStatement, portfolio: &PortfolioConfig) -> EmptyResult {
        for dividend in &statement.dividends {
            let income = dividend.amount.sub(dividend.paid_tax).map_err(|e| format!(
                "{}: The tax is paid in currency different from the dividend currency: {}",
                dividend.description(), e))?;

            let income = self.converter.convert_to(dividend.date, income, self.currency)?;
            self.get_deposit_view(&dividend.issuer).transaction(dividend.date.into(), -income);
            self.income_structure.dividends += income;

            let tax_to_pay = dividend.tax_to_pay(self.country, self.converter)?;
            let (_, tax_payment_date) = portfolio.tax_payment_day().get(dividend.date, false);

            if let Some(amount) = self.map_tax_to_deposit_amount(tax_payment_date, tax_to_pay)? {
                trace!("* {} {} dividend {} tax: {}",
                    dividend.original_issuer, formatting::format_date(dividend.date),
                    formatting::format_date(tax_payment_date), amount);

                self.get_deposit_view(&dividend.issuer).transaction(tax_payment_date.into(), amount);
                self.transaction(tax_payment_date, amount);
                self.income_structure.taxes += amount;
            }
        }

        Ok(())
    }

    fn process_interest(&mut self, statement: &BrokerStatement, portfolio: &PortfolioConfig) -> EmptyResult {
        for interest in &statement.idle_cash_interest {
            self.income_structure.interest += self.converter.convert_to(
                interest.date, interest.amount, self.currency)?;

            let tax_to_pay = interest.tax_to_pay(self.country, self.converter)?;
            let (_, tax_payment_date) = portfolio.tax_payment_day().get(interest.date, false);

            if let Some(amount) = self.map_tax_to_deposit_amount(tax_payment_date, tax_to_pay)? {
                trace!("* {} idle cash interest {} tax: {}",
                       formatting::format_date(interest.date),
                       formatting::format_date(tax_payment_date), amount);

                self.transaction(tax_payment_date, amount);
                self.income_structure.taxes += amount;
            }
        }

        Ok(())
    }

    fn process_tax_agent_withholdings(&mut self, statement: &BrokerStatement) -> EmptyResult {
        for withholding in &statement.tax_agent_withholdings {
            let amount = self.converter.convert_to(withholding.date, withholding.amount, self.currency)?;
            trace!("* Tax withholding {}: {}", formatting::format_date(withholding.date), amount);
            self.transaction(withholding.date, -amount);
        }

        Ok(())
    }

    fn process_tax_deductions(&mut self, portfolio: &PortfolioConfig) -> EmptyResult {
        for &(date, amount) in &portfolio.tax_deductions {
            let amount = self.converter.convert(self.country.currency, self.currency, date, amount)?;
            trace!("* Tax deduction {}: {}", formatting::format_date(date), amount);
            self.transaction(date, -amount);
            self.income_structure.tax_deductions += amount;
        }

        Ok(())
    }

    fn process_cash_assets(&mut self, statement: &BrokerStatement) -> EmptyResult {
        self.current_assets += statement.cash_assets.total_assets_real_time(
            self.currency, self.converter)?;
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

    fn map_tax_to_deposit_amount(&self, tax_payment_date: Date, tax_to_pay: Cash) -> GenericResult<Option<Decimal>> {
        // Treat tax payment as an ordinary deposit which we transfer to the account at tax payment
        // day.

        if tax_to_pay.is_zero() {
            return Ok(None);
        }
        assert!(tax_to_pay.is_positive());

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
    closed: bool,
}

impl StockDepositView {
    fn new() -> StockDepositView {
        StockDepositView {
            name: None,
            trades: BTreeMap::new(),
            transactions: Vec::new(),
            interest_periods: Vec::new(),
            closed: true,
        }
    }

    fn trade(&mut self, time: DateOptTime, quantity: Decimal) {
        self.trades.entry(time.date)
            .and_modify(|total| *total += quantity)
            .or_insert(quantity);
    }

    fn transaction(&mut self, time: DateOptTime, amount: Decimal) {
        // Some assets can be acquired for free due to corporate actions or other non-trading
        // operations.
        if !amount.is_zero() {
            self.transactions.push(Transaction::new(time.date, amount))
        }
    }
}

fn get_total_activity_duration(periods: &[InterestPeriod]) -> u32 {
    periods.iter().map(InterestPeriod::days).sum()
}