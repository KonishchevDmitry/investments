use std::collections::{HashMap, BTreeMap};

use cast::From as CastFrom;
#[cfg(test)] use chrono::Duration;
use log::{self, debug, log_enabled, trace, warn};
use num_traits::Zero;
use static_table_derive::StaticTable;

use crate::broker_statement::BrokerStatement;
use crate::config::PortfolioConfig;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
use crate::formatting::{self, table::{Cell, Style}};
use crate::localities::Country;
use crate::taxes::NetTaxCalculator;
use crate::types::{Date, Decimal};
use crate::util;

use super::deposit_emulator::{DepositEmulator, Transaction, InterestPeriod};

#[derive(StaticTable)]
struct Row {
    #[column(name="Instrument")]
    instrument: String,
    #[column(name="Investments")]
    investments: Cell,
    #[column(name="Profit")]
    profit: Cell,
    #[column(name="Result")]
    result: Cell,
    #[column(name="Duration", align="right")]
    duration: String,
    #[column(name="Interest", align="right")]
    interest: String,
}

/// Calculates average rate of return from cash investments by comparing portfolio performance to
/// performance of a bank deposit with exactly the same investments and monthly capitalization.
pub struct PortfolioPerformanceAnalyser<'a> {
    country: Country,
    currency: &'a str,
    converter: &'a CurrencyConverter,
    show_closed_positions: bool,

    transactions: Vec<Transaction>,
    instruments: Option<HashMap<String, StockDepositView>>,
    current_assets: Decimal,
    table: Table,
}

impl <'a> PortfolioPerformanceAnalyser<'a> {
    pub fn new(
        country: Country, currency: &'a str, converter: &'a CurrencyConverter,
        show_closed_positions: bool,
    ) -> PortfolioPerformanceAnalyser<'a> {
        PortfolioPerformanceAnalyser {
            country,
            currency,
            converter,
            show_closed_positions,

            transactions: Vec::new(),
            instruments: Some(HashMap::new()),
            current_assets: dec!(0),
            table: Table::new(),
        }
    }

    pub fn add(&mut self, statement: &BrokerStatement, portfolio: &PortfolioConfig) -> EmptyResult {
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

        self.current_assets += statement.cash_assets.total_assets_real_time(
            self.currency, self.converter)?;

        for (symbol, deposit_view) in self.instruments.as_mut().unwrap().iter_mut() {
            if deposit_view.name.is_none() {
                deposit_view.name.replace(statement.get_instrument_name(&symbol));
            }
        }

        Ok(())
    }

    pub fn analyse(mut self) -> EmptyResult {
        self.calculate_open_position_periods()?;

        let mut instruments = self.instruments.take().unwrap();
        let mut instruments = instruments.drain().collect::<Vec<_>>();
        instruments.sort_by(|a, b| a.0.cmp(&b.0));

        for (symbol, deposit_view) in instruments {
            self.analyse_instrument_performance(&symbol, deposit_view)?;
        }

        self.analyse_portfolio_performance()?;
        self.table.print(&format!(
            "Average rate of return from cash investments in {}", self.currency));

        Ok(())
    }

    fn analyse_instrument_performance(&mut self, symbol: &str, mut deposit_view: StockDepositView) -> EmptyResult {
        if deposit_view.closed && !self.show_closed_positions {
            return Ok(());
        }

        let days = get_total_activity_duration(&deposit_view.interest_periods);
        deposit_view.transactions.sort_by_key(|transaction| transaction.date);

        let (interest, difference) = compare_to_bank_deposit(
            &deposit_view.transactions, &deposit_view.interest_periods, dec!(0))?;

        check_emulation_precision(
            symbol, self.currency, deposit_view.last_sell_volume.unwrap(), difference)?;

        let mut investments = dec!(0);
        let mut result = dec!(0);

        for transaction in &deposit_view.transactions {
            if transaction.amount.is_sign_positive() {
                investments += transaction.amount;
            } else {
                result += -transaction.amount;
            }
        }

        self.add_results(
            &deposit_view.name.unwrap(), investments, result, interest, days, deposit_view.closed);

        Ok(())
    }

    fn analyse_portfolio_performance(&mut self) -> EmptyResult {
        if self.transactions.is_empty() {
            return Err!("The portfolio has no activity yet");
        }

        self.transactions.sort_by_key(|transaction| transaction.date);
        let activity_periods = vec![InterestPeriod::new(
            self.transactions.first().unwrap().date, util::today())];

        let mut investments = dec!(0);
        for transaction in &self.transactions {
            investments += transaction.amount;
        }

        let (interest, difference) = compare_to_bank_deposit(
            &self.transactions, &activity_periods, self.current_assets)?;

        check_emulation_precision("portfolio", self.currency, self.current_assets, difference)?;

        let days = get_total_activity_duration(&activity_periods);
        self.add_results("", investments, self.current_assets, interest, days, false);

        Ok(())
    }

    fn add_results(
        &mut self, name: &str, investments: Decimal, result: Decimal, interest: Decimal,
        days: i64, inactive: bool
    ) {
        let investments = util::round(investments, 0);
        let result = util::round(result, 0);
        let profit = result - investments;

        let (duration_name, duration_days) = if days >= 365 {
            ("y", 365)
        } else if days >= 30 {
            ("m", 30)
        } else {
            ("d", 1)
        };
        let duration = format!(
            "{}{}", util::round(Decimal::from(days) / Decimal::from(duration_days), 1),
            duration_name);

        let mut row = self.table.add_row(Row {
            instrument: name.to_owned(),
            investments: Cell::new_round_decimal(investments),
            profit: Cell::new_round_decimal(profit),
            result: Cell::new_round_decimal(result),
            duration: duration,
            interest: format!("{}%", interest),
        });

        if inactive {
            let style = Style::new().dimmed();
            for cell in &mut row {
                cell.style(style);
            }
        }
    }

    fn calculate_open_position_periods(&mut self) -> EmptyResult {
        struct OpenPosition {
            start_date: Date,
            quantity: i32,
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
                        quantity: 0,
                    }
                });
                current.quantity += quantity;

                if current.quantity > 0 {
                    continue;
                } else if current.quantity < 0 {
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
        }

        Ok(())
    }

    fn process_positions(&mut self, statement: &BrokerStatement, portfolio: &PortfolioConfig) -> EmptyResult {
        let mut taxes = NetTaxCalculator::new(self.country, portfolio.tax_payment_day);
        let mut stock_taxes = HashMap::new();

        for stock_buy in &statement.stock_buys {
            let mut assets = self.converter.convert_to(
                stock_buy.execution_date, stock_buy.volume, self.currency)?;

            assets += self.converter.convert_to(
                stock_buy.conclusion_date, stock_buy.commission, self.currency)?;

            let deposit_view = self.get_deposit_view(&stock_buy.symbol);
            deposit_view.trade(stock_buy.conclusion_date, i32::cast(stock_buy.quantity).unwrap());
            deposit_view.transaction(stock_buy.conclusion_date, assets);
        }

        for stock_sell in &statement.stock_sells {
            let assets = self.converter.convert_to(
                stock_sell.execution_date, stock_sell.volume, self.currency)?;

            let commission = self.converter.convert_to(
                stock_sell.conclusion_date, stock_sell.commission, self.currency)?;

            {
                let deposit_view = self.get_deposit_view(&stock_sell.symbol);

                deposit_view.trade(stock_sell.conclusion_date, -i32::cast(stock_sell.quantity).unwrap());
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
        }

        for (&symbol, symbol_taxes) in stock_taxes.iter() {
            for (&tax_payment_date, &tax_to_pay) in symbol_taxes.get_taxes().iter() {
                if let Some(deposit_amount) = self.map_tax_to_deposit_amount(tax_payment_date, tax_to_pay)? {
                    trace!("* {} selling {} tax: {}",
                           symbol, formatting::format_date(tax_payment_date), deposit_amount);

                    self.get_deposit_view(symbol).transaction(tax_payment_date, deposit_amount);
                }
            }
        }

        for (&tax_payment_date, &tax_to_pay) in taxes.get_taxes().iter() {
            if let Some(deposit_amount) = self.map_tax_to_deposit_amount(tax_payment_date, tax_to_pay)? {
                trace!("* Stock selling {} tax: {}",
                       formatting::format_date(tax_payment_date), deposit_amount);
                self.transaction(tax_payment_date, deposit_amount);
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

            let tax_to_pay = dividend.tax_to_pay(&self.country, self.converter)?;
            let tax_payment_date = portfolio.tax_payment_day.get(dividend.date);

            if let Some(deposit_amount) = self.map_tax_to_deposit_amount(tax_payment_date, tax_to_pay)? {
                trace!("* {} {} dividend {} tax: {}",
                       dividend.issuer, formatting::format_date(dividend.date),
                       formatting::format_date(tax_payment_date), deposit_amount);

                self.get_deposit_view(&dividend.issuer).transaction(tax_payment_date, deposit_amount);
                self.transaction(tax_payment_date, deposit_amount);
            }
        }

        Ok(())
    }

    fn process_interest(&mut self, statement: &BrokerStatement, portfolio: &PortfolioConfig) -> EmptyResult {
        for interest in &statement.idle_cash_interest {
            let tax_to_pay = interest.tax_to_pay(&self.country, self.converter)?;
            let tax_payment_date = portfolio.tax_payment_day.get(interest.date);

            if let Some(deposit_amount) = self.map_tax_to_deposit_amount(tax_payment_date, tax_to_pay)? {
                trace!("* {} idle cash interest {} tax: {}",
                       formatting::format_date(interest.date),
                       formatting::format_date(tax_payment_date), deposit_amount);

                self.transaction(tax_payment_date, deposit_amount);
            }
        }

        Ok(())
    }

    fn process_tax_deductions(&mut self, portfolio: &PortfolioConfig) -> EmptyResult {
        for &(date, amount) in &portfolio.tax_deductions {
            let amount = self.converter.convert(self.country.currency, self.currency, date, amount)?;
            trace!("* Tax deduction {}: {}", formatting::format_date(date), -amount);
            self.transaction(date, -amount);
        }

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

        let today = util::today();
        let conversion_date = if tax_payment_date > today {
            today
        } else {
            tax_payment_date
        };

        Ok(Some(self.converter.convert_to(conversion_date, tax_to_pay, self.currency)?))
    }
}

struct StockDepositView {
    name: Option<String>,
    trades: BTreeMap<Date, i32>,
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

    fn trade(&mut self, date: Date, quantity: i32) {
        self.trades.entry(date)
            .and_modify(|total| *total += quantity)
            .or_insert(quantity);
    }

    fn transaction(&mut self, date: Date, amount: Decimal) {
        self.transactions.push(Transaction::new(date, amount))
    }
}

fn compare_to_bank_deposit(
    transactions: &[Transaction], interest_periods: &[InterestPeriod], current_assets: Decimal
) -> GenericResult<(Decimal, Decimal)> {
    let start_date = std::cmp::min(
        transactions.first().unwrap().date,
        interest_periods.first().unwrap().start,
    );

    let end_date = std::cmp::max(
        transactions.last().unwrap().date,
        interest_periods.last().unwrap().end,
    );

    let emulate = |interest: Decimal| -> Decimal {
        let result_assets = DepositEmulator::new(start_date, end_date, interest)
            .with_interest_periods(interest_periods)
            .emulate(transactions);

        (current_assets - result_assets).abs()
    };

    let mut interest = dec!(0);
    let mut difference = emulate(interest);

    for mut step in [dec!(10), dec!(1), dec!(0.1), dec!(0.01)].iter().cloned() {
        let decreasing_difference = emulate(interest - step);
        let increasing_difference = emulate(interest + step);

        if decreasing_difference > difference && difference < increasing_difference {
            continue;
        }

        if decreasing_difference < increasing_difference {
            assert!(decreasing_difference < difference);
            step = -step;
        } else if decreasing_difference > increasing_difference {
            assert!(increasing_difference < difference);
        } else {
            unreachable!();
        }

        interest += step;

        loop {
            let next_interest = interest + step;
            let next_difference = emulate(next_interest);

            if next_difference > difference {
                break;
            }

            difference = next_difference;
            interest = next_interest;
        }
    }

    Ok((interest, difference))
}

fn check_emulation_precision(name: &str, currency: &str, assets: Decimal, difference: Decimal) -> EmptyResult {
    let precision = (difference / assets).abs() * dec!(100);
    let difference = Cash::new(currency, difference).round();

    if precision >= dec!(1) {
        let message = format!(concat!(
            "Failed to compare {} {} performance to bank deposit: ",
            "got a result with too low precision ({}%, {})"),
            name, currency, util::round(precision, 3), difference);

        if cfg!(debug_assertions) {
            return Err(message.into());
        }

        warn!("{}.", message);
        return Ok(());
    }

    debug!("Got a result of comparing {} {} performance to bank deposit: {}% precision ({}).",
           name, currency, util::round(precision, 4), difference);

    Ok(())
}

fn get_total_activity_duration(periods: &[InterestPeriod]) -> i64 {
    periods.iter().map(|period| (period.end - period.start).num_days()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_joint_deposits() {
        let compare = |transactions: &[Transaction], interest_periods: &[InterestPeriod], current_assets: Decimal| {
            let (interest, difference) = compare_to_bank_deposit(
                transactions, interest_periods, current_assets).unwrap();

            assert_eq!(interest, dec!(7));
            assert!(difference < dec!(0.01));
        };

        let mut transactions = Vec::new();
        let mut interest_periods = Vec::new();

        {
            // First deposit

            let open_date = date!(28, 7, 2018);
            let close_date = date!(28, 1, 2019);

            transactions.extend(&[
                // Fake transaction outside of interest period
                Transaction::new(open_date - Duration::days(100), dec!(400_000)),

                // Deposit opening transaction
                Transaction::new(open_date, dec!(200_000)),
            ]);

            for &(capitalization_date, assets) in &[
                (date!(28,  8, 2018), dec!(603_567.12)),
                (date!(28,  9, 2018), dec!(607_155.45)),
                (date!(28, 10, 2018), dec!(610_648.68)),
                (date!(28, 11, 2018), dec!(614_279.11)),
                (date!(28, 12, 2018), dec!(617_813.32)),
                (date!(28,  1, 2019), dec!(621_486.34)),
            ] {
                let mut interest_periods = interest_periods.clone();
                interest_periods.push(InterestPeriod::new(open_date, capitalization_date));
                compare(&transactions, &interest_periods, assets);
            }

            interest_periods.push(InterestPeriod::new(open_date, close_date));
            compare(&transactions, &interest_periods, dec!(621_486.34));

            // Withdraw some assets at close date
            transactions.push(Transaction::new(close_date, dec!(-321_486.34)));
            compare(&transactions, &interest_periods, dec!(300_000));

            // Withdraw some assets between interest periods
            transactions.push(Transaction::new(date!(29, 1, 2019), dec!(-200_000)));
            compare(&transactions, &interest_periods, dec!(100_000));

            // Deposit some assets between interest periods
            transactions.push(Transaction::new(date!(30, 1, 2019), dec!(50_000)));
            compare(&transactions, &interest_periods, dec!(150_000));
        }

        {
            // Second deposit

            let open_date = date!(31, 1, 2019);
            let close_date = date!(31, 7, 2019);

            // Deposit more assets at open date
            transactions.push(Transaction::new(open_date, dec!(40_000)));
            compare(&transactions, &interest_periods, dec!(190_000));

            // Deposit contributions
            transactions.extend(&[
                Transaction::new(date!( 5, 2, 2019), dec!(60_000)),
                Transaction::new(date!(21, 2, 2019), dec!(50_000)),
            ]);

            for &(capitalization_date, assets) in &[
                (date!(28, 2, 2019), dec!(301_352.05)),
                (date!(31, 3, 2019), dec!(303_143.65)),
                (date!(30, 4, 2019), dec!(304_887.77)),
                (date!(31, 5, 2019), dec!(306_700.39)),
                (date!(30, 6, 2019), dec!(308_464.97)),
                (date!(31, 7, 2019), dec!(310_298.85)),
            ] {
                let mut interest_periods = interest_periods.clone();
                interest_periods.push(InterestPeriod::new(open_date, capitalization_date));
                compare(&transactions, &interest_periods, assets);
            }

            interest_periods.push(InterestPeriod::new(open_date, close_date));
            compare(&transactions, &interest_periods, dec!(310_298.85));

            // Withdraw some assets at close date
            transactions.push(Transaction::new(close_date, dec!(-110_298.85)));
            compare(&transactions, &interest_periods, dec!(200_000));

            // Withdraw more assets
            transactions.push(Transaction::new(close_date + Duration::days(100), dec!(-100_000)));
            compare(&transactions, &interest_periods, dec!(100_000));

            // Withdraw the rest
            transactions.push(Transaction::new(close_date + Duration::days(200), dec!(-100_000)));
            compare(&transactions, &interest_periods, dec!(0));

            // Get into negative balance
            transactions.push(Transaction::new(close_date + Duration::days(300), dec!(-100_000)));
            compare(&transactions, &interest_periods, dec!(-100_000));
        }
    }
}
