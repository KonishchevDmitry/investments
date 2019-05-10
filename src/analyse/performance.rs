use std;
use std::collections::{HashMap, BTreeMap};

use cast::From as CastFrom;
use chrono::Duration;
use log::{self, debug, log_enabled, trace};
use num_traits::Zero;

use crate::broker_statement::BrokerStatement;
use crate::config::PortfolioConfig;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
use crate::formatting;
use crate::formatting::table::{Table, Row, Cell, Alignment, print_table};
use crate::localities::Country;
use crate::taxes::NetTaxCalculator;
use crate::types::{Date, Decimal};
use crate::util;

use super::deposit_emulator::{DepositEmulator, Transaction, InterestPeriod};

/// Calculates average rate of return from cash investments by comparing portfolio performance to
/// performance of a bank deposit with exactly the same investments and monthly capitalization.
pub struct PortfolioPerformanceAnalyser<'a> {
    statement: &'a BrokerStatement,
    portfolio: &'a PortfolioConfig,

    currency: &'a str,
    converter: &'a CurrencyConverter,

    country: Country,
    transactions: Vec<Transaction>,
    instruments: Option<HashMap<String, StockDepositView>>,
    table: Table,
}

impl <'a> PortfolioPerformanceAnalyser<'a> {
    pub fn analyse(
        statement: &BrokerStatement, portfolio: &PortfolioConfig, currency: &str,
        converter: &CurrencyConverter
    ) -> EmptyResult {
        let mut analyser = PortfolioPerformanceAnalyser {
            statement,
            portfolio,

            currency,
            converter,

            country: portfolio.get_tax_country(),
            transactions: Vec::new(),
            instruments: Some(HashMap::new()),
            table: Table::new(),
        };

        // Assume that the caller has simulated sellout and just check it here
        if !statement.open_positions.is_empty() {
            return Err!(
                "Unable to calculate current assets: The broker statement has open positions");
        }

        analyser.calculate_open_position_periods()?;

        trace!("Deposit emulator transactions:");
        analyser.process_deposits_and_withdrawals()?;
        analyser.process_positions()?;
        analyser.process_dividends()?;
        analyser.process_interest()?;
        analyser.process_tax_deductions()?;

        let mut instruments = analyser.instruments.take().unwrap();
        let mut instruments = instruments.drain().collect::<Vec<_>>();
        instruments.sort_by(|a, b| a.0.cmp(&b.0));

        for (symbol, deposit_view) in instruments {
            analyser.analyse_instrument_performance(&symbol, deposit_view)?;
        }

        analyser.analyse_portfolio_performance()?;

        print_table(
            &format!("Average rate of return from cash investments in {}", currency),
            &["Instrument", "Investments", "Profit", "Result", "Duration", "Interest"],
            analyser.table,
        );

        Ok(())
    }

    fn add_results(&mut self, name: &str, investments: Decimal, result: Decimal, interest: Decimal, days: i64) {
        let investments = util::round_to(investments, 0);
        let result = util::round_to(result, 0);
        let profit = result - investments;

        let (duration_name, duration_days) = if days >= 365 {
            ("y", 365)
        } else if days >= 30 {
            ("m", 30)
        } else {
            ("d", 1)
        };
        let duration = format!(
            "{}{}", util::round_to(Decimal::from(days) / Decimal::from(duration_days), 1),
            duration_name);

        let row = vec![
            Cell::new(name),
            Cell::new_round_decimal(investments),
            Cell::new_round_decimal(profit),
            Cell::new_round_decimal(result),
            Cell::new_align(&duration, Alignment::RIGHT),
            Cell::new_align(&format!("{}%", interest), Alignment::RIGHT),
        ];

        self.table.add_row(Row::new(&row));
    }

    fn analyse_instrument_performance(&mut self, symbol: &str, mut deposit_view: StockDepositView) -> EmptyResult {
        deposit_view.transactions.sort_by_key(|transaction| transaction.date);

        let (interest, difference) = compare_to_bank_deposit(
            &deposit_view.transactions, &deposit_view.interest_periods, dec!(0))?;

        check_emulation_precision(
            &format!("{} {}", symbol, self.currency),
            deposit_view.last_sell_volume.unwrap(), difference)?;

        let mut investments = dec!(0);
        let mut result = dec!(0);

        for transaction in &deposit_view.transactions {
            if transaction.amount.is_sign_positive() {
                investments += transaction.amount;
            } else {
                result += -transaction.amount;
            }
        }

        let name = self.statement.get_instrument_name(symbol)?;
        let days = get_total_activity_duration(&deposit_view.interest_periods);

        self.add_results(&name, investments, result, interest, days);

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

        let current_assets = self.statement.cash_assets.total_assets(
            self.currency, self.converter)?;

        let (interest, difference) = compare_to_bank_deposit(
            &self.transactions, &activity_periods, current_assets)?;

        check_emulation_precision(
            &format!("portfolio {}", self.currency), current_assets, difference)?;

        let days = get_total_activity_duration(&activity_periods);
        self.add_results("", investments, current_assets, interest, days);

        Ok(())
    }

    fn get_deposit_view(&mut self, symbol: &str) -> GenericResult<&mut StockDepositView> {
        Ok(self.instruments.as_mut().unwrap().get_mut(symbol).ok_or_else(|| format!(
            "Got an unexpected transaction for {} which had no open positions", symbol))?)
    }

    fn calculate_open_position_periods(&mut self) -> EmptyResult {
        struct Trade {
            quantity: i32,
            conclusion_date: Date,
        }
        type Trades = BTreeMap<Date, Trade>;
        let mut trades: HashMap<&str, Trades> = HashMap::new();

        let add_trade = |
            symbol_trades: std::collections::hash_map::Entry<&str, Trades>, quantity: i32,
            conclusion_date: Date, execution_date: Date
        | {
            symbol_trades.or_insert_with(BTreeMap::new)
                .entry(execution_date)
                .and_modify(|trade| {
                    if quantity > 0 {
                        trade.conclusion_date = std::cmp::min(trade.conclusion_date, conclusion_date);
                    }
                    trade.quantity += quantity;
                })
                .or_insert_with(|| Trade {
                    conclusion_date: conclusion_date,
                    quantity: quantity,
                });
        };

        for stock_buy in &self.statement.stock_buys {
            add_trade(trades.entry(&stock_buy.symbol),
                      i32::cast(stock_buy.quantity).unwrap(),
                      stock_buy.conclusion_date, stock_buy.execution_date);
        }

        for stock_sell in &self.statement.stock_sells {
            add_trade(trades.entry(&stock_sell.symbol),
                      -i32::cast(stock_sell.quantity).unwrap(),
                      stock_sell.conclusion_date, stock_sell.execution_date);
        }

        struct OpenPosition {
            start_date: Date,
            quantity: i32,
        }

        trace!("Open positions periods:");

        for (symbol, symbol_trades) in &trades {
            let symbol = *symbol;
            let mut open_position = None;
            let mut open_periods: Vec<InterestPeriod> = Vec::new();

            for (execution_date, trade) in symbol_trades {
                let execution_date = *execution_date;
                let current = open_position.get_or_insert_with(|| {
                    OpenPosition {
                        start_date: trade.conclusion_date,
                        quantity: 0,
                    }
                });

                current.quantity += trade.quantity;

                if current.quantity > 0 {
                    continue;
                } else if current.quantity < 0 {
                    return Err!(
                        "Error while processing {} sell operations: Got a negative balance on {}",
                        symbol, formatting::format_date(execution_date));
                }

                let start_date = current.start_date;
                let end_date = if execution_date == start_date {
                    start_date + Duration::days(1)
                } else {
                    execution_date
                };

                match open_periods.last_mut() {
                    Some(ref mut period) if period.end >= start_date => {
                        assert!(period.end < end_date);
                        period.end = end_date;
                    },
                    _ => open_periods.push(InterestPeriod::new(start_date, end_date)),
                };

                open_position = None;
            }

            if open_position.is_some() {
                return Err!(
                    "The portfolio contains unsold {} stocks when sellout simulation is expected",
                    symbol);
            }

            assert!(!open_periods.is_empty());

            if log_enabled!(log::Level::Trace) {
                let periods = open_periods.iter()
                    .map(|period| format!(
                        "{} - {}", formatting::format_date(period.start),
                        formatting::format_date(period.end)))
                    .collect::<Vec<_>>()
                    .join(", ");

                trace!("* {}: {}", symbol, periods);
            }

            let deposit_view = StockDepositView {
                transactions: Vec::new(),
                interest_periods: open_periods,
                last_sell_volume: None,
            };

            assert!(self.instruments.as_mut().unwrap()
                .insert(symbol.to_string(), deposit_view).is_none());
        }

        Ok(())
    }

    fn process_deposits_and_withdrawals(&mut self) -> EmptyResult {
        for mut cash_flow in self.statement.cash_flows.iter().cloned() {
            if cash_flow.cash.is_positive() {
                cash_flow.cash.amount += self.statement.broker.get_deposit_commission(cash_flow)?;
            }

            let amount = self.converter.convert_to(cash_flow.date, cash_flow.cash, self.currency)?;

            trace!("* {} {}: {}", if amount.is_sign_positive() {
                "Deposit"
            } else {
                "Withdrawal"
            }, formatting::format_date(cash_flow.date), amount.normalize());

            self.transactions.push(Transaction::new(cash_flow.date, amount));
        }

        Ok(())
    }

    fn process_positions(&mut self) -> EmptyResult {
        let mut taxes = NetTaxCalculator::new(self.country, self.portfolio.tax_payment_day);
        let mut stock_taxes = HashMap::new();

        for stock_buy in &self.statement.stock_buys {
            let mut assets = self.converter.convert_to(
                stock_buy.conclusion_date, stock_buy.price * stock_buy.quantity, self.currency)?;

            assets += self.converter.convert_to(
                stock_buy.conclusion_date, stock_buy.commission, self.currency)?;

            self.get_deposit_view(&stock_buy.symbol)?.transactions.push(
                Transaction::new(stock_buy.conclusion_date, assets));
        }

        for stock_sell in &self.statement.stock_sells {
            let assets = self.converter.convert_to(
                stock_sell.execution_date, stock_sell.price * stock_sell.quantity, self.currency)?;

            let commission = self.converter.convert_to(
                stock_sell.conclusion_date, stock_sell.commission, self.currency)?;

            {
                let deposit_view = self.get_deposit_view(&stock_sell.symbol)?;

                deposit_view.transactions.push(Transaction::new(
                    stock_sell.execution_date, -assets));

                deposit_view.transactions.push(Transaction::new(
                    stock_sell.conclusion_date, commission));

                deposit_view.last_sell_volume.replace(assets);
            }

            let local_profit = stock_sell.calculate(&self.country, self.converter)?.local_profit.amount;

            stock_taxes.entry(&stock_sell.symbol)
                .or_insert_with(|| NetTaxCalculator::new(self.country, self.portfolio.tax_payment_day))
                .add_profit(stock_sell.execution_date, local_profit);

            taxes.add_profit(stock_sell.execution_date, local_profit);
        }

        for (&symbol, symbol_taxes) in stock_taxes.iter() {
            for (&tax_payment_date, &tax_to_pay) in symbol_taxes.get_taxes().iter() {
                if let Some(deposit_amount) = self.map_tax_to_deposit_amount(tax_payment_date, tax_to_pay)? {
                    trace!("* {} selling {} tax: {}",
                           symbol, formatting::format_date(tax_payment_date), deposit_amount);

                    self.get_deposit_view(symbol)?.transactions.push(Transaction::new(
                        tax_payment_date, deposit_amount));
                }
            }
        }

        for (&tax_payment_date, &tax_to_pay) in taxes.get_taxes().iter() {
            if let Some(deposit_amount) = self.map_tax_to_deposit_amount(tax_payment_date, tax_to_pay)? {
                trace!("* Stock selling {} tax: {}",
                       formatting::format_date(tax_payment_date), deposit_amount);
                self.transactions.push(Transaction::new(tax_payment_date, deposit_amount));
            }
        }

        Ok(())
    }

    fn process_dividends(&mut self) -> EmptyResult {
        for dividend in &self.statement.dividends {
            let profit = dividend.amount.sub(dividend.paid_tax).map_err(|e| format!(
                "{}: The tax is paid in currency different from the dividend currency: {}",
                dividend.description(), e))?;

            let profit = self.converter.convert_to(dividend.date, profit, self.currency)?;
            self.get_deposit_view(&dividend.issuer)?.transactions.push(
                Transaction::new(dividend.date, -profit));

            let tax_to_pay = dividend.tax_to_pay(&self.country, self.converter)?;
            let tax_payment_date = self.portfolio.tax_payment_day.get(dividend.date);

            if let Some(deposit_amount) = self.map_tax_to_deposit_amount(tax_payment_date, tax_to_pay)? {
                trace!("* {} {} dividend {} tax: {}",
                       dividend.issuer, formatting::format_date(dividend.date),
                       formatting::format_date(tax_payment_date), deposit_amount);

                self.get_deposit_view(&dividend.issuer)?.transactions.push(
                    Transaction::new(tax_payment_date, deposit_amount));

                self.transactions.push(Transaction::new(tax_payment_date, deposit_amount));
            }
        }

        Ok(())
    }

    fn process_interest(&mut self) -> EmptyResult {
        for interest in &self.statement.idle_cash_interest {
            let tax_to_pay = interest.tax_to_pay(&self.country, self.converter)?;
            let tax_payment_date = self.portfolio.tax_payment_day.get(interest.date);

            if let Some(deposit_amount) = self.map_tax_to_deposit_amount(tax_payment_date, tax_to_pay)? {
                trace!("* {} idle cash interest {} tax: {}",
                       formatting::format_date(interest.date),
                       formatting::format_date(tax_payment_date), deposit_amount);

                self.transactions.push(Transaction::new(tax_payment_date, deposit_amount));
            }
        }

        Ok(())
    }

    fn process_tax_deductions(&mut self) -> EmptyResult {
        for tax_deduction in &self.portfolio.tax_deductions {
            let amount = self.converter.convert_to(
                tax_deduction.date, tax_deduction.cash, self.currency)?;

            trace!("* Tax deduction {}: {}", formatting::format_date(tax_deduction.date), -amount);
            self.transactions.push(Transaction::new(tax_deduction.date, -amount));
        }

        Ok(())
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
    transactions: Vec<Transaction>,
    interest_periods: Vec<InterestPeriod>,
    last_sell_volume: Option<Decimal>,
}

fn compare_to_bank_deposit(
    transactions: &[Transaction], interest_periods: &[InterestPeriod], current_assets: Decimal
) -> GenericResult<(Decimal, Decimal)> {
    let start_assets = dec!(0);

    let start_date = std::cmp::min(
        transactions.first().unwrap().date,
        interest_periods.first().unwrap().start,
    );

    let end_date = std::cmp::max(
        transactions.last().unwrap().date,
        interest_periods.last().unwrap().end,
    );

    let emulate = |interest: Decimal| -> Decimal {
        let result_assets = DepositEmulator::emulate(
            start_date, start_assets, transactions, end_date, interest, Some(interest_periods));

        (current_assets - result_assets).abs()
    };

    let mut interest = dec!(0);
    let mut difference = emulate(interest);

    for mut step in [dec!(1), dec!(0.1), dec!(0.01)].iter().cloned() {
        let decreasing_difference = emulate(interest - step);
        let increasing_difference = emulate(interest + step);

        if decreasing_difference > difference && difference < increasing_difference {
            break;
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

fn check_emulation_precision(name: &str, assets: Decimal, difference: Decimal) -> EmptyResult {
    let precision = (difference / assets).abs();

    if precision >= dec!(0.01) {
        return Err!(concat!(
            "Failed to compare {} performance to bank deposit: ",
            "got a result with too low precision ({})"), name, util::round_to(precision, 3));
    }

    debug!("Got a result of comparing {} performance to bank deposit: {}% precision.",
           name, util::round_to(precision * dec!(100), 4));

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