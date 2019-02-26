use std;
use std::collections::{HashMap, BTreeMap};

use cast::From as CastFrom;
use chrono::Duration;
use log::debug;
use num_traits::Zero;
use prettytable::{Table, Row, Cell};
use prettytable::format::Alignment;

use crate::broker_statement::BrokerStatement;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, CashAssets};
use crate::currency::converter::CurrencyConverter;
use crate::formatting;
use crate::localities::{self, Country};
use crate::types::{Date, Decimal};
use crate::util;

use super::deposit_emulator::{DepositEmulator, Transaction, InterestPeriod};

/// Calculates average rate of return from cash investments by comparing portfolio performance to
/// performance of a bank deposit with exactly the same investments and monthly capitalization.
pub struct PortfolioPerformanceAnalyser<'a> {
    statement: &'a BrokerStatement,
    currency: &'a str,
    converter: &'a CurrencyConverter,

    country: Country,
    transactions: Vec<Transaction>,
    instruments: Option<HashMap<String, StockDepositView>>,
    table: Table,
}

impl <'a> PortfolioPerformanceAnalyser<'a> {
    pub fn analyse(
        statement: &BrokerStatement, tax_deductions: &[CashAssets], currency: &str,
        converter: &CurrencyConverter
    ) -> EmptyResult {
        let mut analyser = PortfolioPerformanceAnalyser {
            statement: statement,
            currency: currency,
            converter: converter,

            country: localities::russia(),
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
        analyser.process_deposits_and_withdrawals()?;
        analyser.process_positions()?;
        analyser.process_dividends()?;
        analyser.process_tax_deductions(tax_deductions)?;

        let mut instruments = analyser.instruments.take().unwrap();
        let mut instruments = instruments.drain().collect::<Vec<_>>();
        instruments.sort_by(|a, b| a.0.cmp(&b.0));

        for (symbol, deposit_view) in instruments {
            analyser.analyse_instrument_performance(&symbol, deposit_view)?;
        }

        analyser.analyse_portfolio_performance()?;

        formatting::print_statement(
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

        self.table.add_row(Row::new(vec![
            Cell::new(name),
            formatting::round_decimal_cell(investments),
            formatting::round_decimal_cell(profit),
            formatting::round_decimal_cell(result),
            Cell::new_align(&duration, Alignment::RIGHT),
            Cell::new_align(&format!("{}%", interest), Alignment::RIGHT),
        ]));
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
            self.transactions.push(Transaction::new(cash_flow.date, amount));
        }

        Ok(())
    }

    fn process_positions(&mut self) -> EmptyResult {
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

            let tax_to_pay = stock_sell.tax_to_pay(&self.country, self.converter)?;
            self.process_tax(stock_sell.execution_date, &stock_sell.symbol, tax_to_pay)?;
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
            self.process_tax(dividend.date, &dividend.issuer, tax_to_pay)?;
        }

        Ok(())
    }

    fn process_tax_deductions(&mut self, tax_deductions: &[CashAssets]) -> EmptyResult {
        for tax_deduction in tax_deductions {
            let amount = self.converter.convert_to(
                tax_deduction.date, tax_deduction.cash, self.currency)?;

            self.transactions.push(Transaction::new(tax_deduction.date, -amount));
        }

        Ok(())
    }

    fn process_tax(&mut self, income_date: Date, symbol: &str, tax_to_pay: Decimal) -> EmptyResult {
        // Treat tax payment as an ordinary deposit which we transfer to the account at tax payment
        // day.

        if tax_to_pay.is_zero() {
            return Ok(());
        }
        assert!(tax_to_pay.is_sign_positive());

        let tax_to_pay = Cash::new(self.country.currency, tax_to_pay);
        let tax_payment_date = self.country.get_tax_payment_date(income_date);

        let today = util::today();
        let conversion_date = if tax_payment_date > today {
            today
        } else {
            tax_payment_date
        };

        let deposit_amount = self.converter.convert_to(
            conversion_date, tax_to_pay, self.currency)?;

        self.get_deposit_view(symbol)?.transactions.push(
            Transaction::new(tax_payment_date, deposit_amount));

        self.transactions.push(
            Transaction::new(tax_payment_date, deposit_amount));

        Ok(())
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
    fn real_deposit() {
        let (interest, difference) = compare_to_bank_deposit(
            &[Transaction::new(date!(28, 7, 2018), dec!(600_000))],
            &[InterestPeriod::new(date!(28, 7, 2018), date!(28, 1, 2019))],
            dec!(621486.34),
        ).unwrap();

        assert_eq!(interest, dec!(7));
        assert!(difference < dec!(0.01));
    }

    #[test]
    fn real_deposit_fake_transactions() {
        let (interest, difference) = compare_to_bank_deposit(
            &[
                Transaction::new(date!( 2, 2, 2018), dec!(200_000)),
                Transaction::new(date!(28, 7, 2018), dec!(400_000)),
                Transaction::new(date!( 3, 3, 2019), dec!(-300_000)),
            ],
            &[InterestPeriod::new(date!(28, 7, 2018), date!(28, 1, 2019))],
            dec!(321486.34),
        ).unwrap();

        assert_eq!(interest, dec!(7));
        assert!(difference < dec!(0.01));
    }
}