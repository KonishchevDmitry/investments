use std::collections::HashMap;

use num_traits::{ToPrimitive, Zero};
use prettytable::{Table, Row, Cell};
use prettytable::format::Alignment;
use separator::Separatable;

use broker_statement::BrokerStatement;
use core::{EmptyResult, GenericResult};
use currency::Cash;
use currency::converter::CurrencyConverter;
use formatting;
use regulations::{self, Country};
use types::{Date, Decimal};
use util;

use super::deposit_emulator::{DepositEmulator, Transaction};

/// Calculates average rate of return from cash investments by comparing portfolio performance to
/// performance of a bank deposit with exactly the same investments and monthly capitalization.
pub struct PortfolioPerformanceAnalyser<'a> {
    statement: &'a BrokerStatement,
    currency: &'a str,
    converter: &'a CurrencyConverter,

    date: Date,
    country: Country,
    transactions: Vec<Transaction>,
    instruments: Option<HashMap<String, StockDepositView>>,
    table: Table,
}

impl <'a> PortfolioPerformanceAnalyser<'a> {
    pub fn analyse(
        statement: &BrokerStatement, currency: &str, converter: &CurrencyConverter
    ) -> EmptyResult {
        let mut analyser = PortfolioPerformanceAnalyser {
            statement: statement,
            currency: currency,
            converter: converter,

            date: util::today(),
            country: regulations::russia(),
            transactions: Vec::new(),
            instruments: Some(HashMap::new()),
            table: Table::new(),
        };

        // Assume that the caller has simulated sellout and just check it here
        if !statement.open_positions.is_empty() {
            return Err!(
                "Unable to calculate current assets: The broker statement has open positions");
        }

        // TODO: Withdrawals support
        analyser.process_deposits()?;
        analyser.process_positions()?;
        analyser.process_dividends()?;

        let mut instruments = analyser.instruments.take().unwrap();
        let mut instruments = instruments.drain().collect::<Vec<_>>();
        instruments.sort_by(|a, b| a.0.cmp(&b.0));

        for (symbol, deposit_view) in instruments {
            analyser.analyse_instrument_performance(&symbol, deposit_view)?;
        }

        analyser.analyse_portfolio_performance()?;

        formatting::print_statement(
            &format!("Average rate of return from cash investments in {}", currency),
            vec!["Instrument", "Investments", "Profit", "Result", "Duration", "Interest"],
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

        let cash_cell = |amount: Decimal| Cell::new_align(
            &amount.to_i64().unwrap().separated_string(), Alignment::RIGHT);

        self.table.add_row(Row::new(vec![
            Cell::new(name), cash_cell(investments), cash_cell(profit), cash_cell(result),
            Cell::new_align(&duration, Alignment::RIGHT),
            Cell::new_align(&format!("{}%", interest), Alignment::RIGHT),
        ]));
    }

    fn analyse_instrument_performance(&mut self, symbol: &str, mut deposit_view: StockDepositView) -> EmptyResult {
        deposit_view.transactions.sort_by_key(|transaction| transaction.date);

        let (sell_date, sell_volume) = match deposit_view.sell_transaction {
            Some(transaction) => transaction,
            None => return Err!(concat!(
                "Inconsistent broker statement: ",
                "It has income/expenses for {} but doesn't have any buy/sell transactions for it"),
                symbol),
        };

        let (interest, difference) = compare_to_bank_deposit(
            &deposit_view.transactions, sell_date, dec!(0))?;

        check_emulation_precision(
            &format!("{} {}", symbol, self.currency), sell_volume, difference)?;

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
        // TODO: Zero assets pauses
        let days = (sell_date - deposit_view.transactions.first().unwrap().date).num_days();

        self.add_results(&name, investments, result, interest, days);

        Ok(())
    }

    fn analyse_portfolio_performance(&mut self) -> EmptyResult {
        self.transactions.sort_by_key(|transaction| transaction.date);

        let mut investments = dec!(0);
        for transaction in &self.transactions {
            investments += transaction.amount;
        }

        let current_assets = self.statement.cash_assets.total_assets(
            self.currency, self.converter)?;

        let (interest, difference) = compare_to_bank_deposit(
            &self.transactions, self.date, current_assets)?;

        check_emulation_precision(
            &format!("portfolio {}", self.currency), current_assets, difference)?;

        // TODO: Zero assets pauses
        let start_date = self.transactions.first()
            .map(|transaction| transaction.date)
            .unwrap_or(self.statement.period.0);
        let days = (self.date - start_date).num_days();

        self.add_results("", investments, current_assets, interest, days);

        Ok(())
    }

    fn get_deposit_view(&mut self, symbol: &String) -> &mut StockDepositView {
        if !self.instruments.as_ref().unwrap().contains_key(symbol) {
            self.instruments.as_mut().unwrap().insert(symbol.clone(), StockDepositView::new());
        }
        self.instruments.as_mut().unwrap().get_mut(symbol).unwrap()
    }

    fn process_deposits(&mut self) -> EmptyResult {
        if self.statement.deposits.is_empty() {
            return Err!("Broker statement contains no deposits");
        }

        for mut deposit in self.statement.deposits.iter().cloned() {
            assert!(deposit.cash.is_positive());
            deposit.cash.amount += self.statement.broker.get_deposit_commission(deposit)?;
            let amount = self.converter.convert_to(deposit.date, deposit.cash, self.currency)?;

            self.transactions.push(Transaction::new(deposit.date, amount));
        }

        Ok(())
    }

    fn process_positions(&mut self) -> EmptyResult {
        for stock_buy in &self.statement.stock_buys {
            let mut assets = self.converter.convert_to(
                stock_buy.date, stock_buy.price * stock_buy.quantity, self.currency)?;

            assets += self.converter.convert_to(
                stock_buy.date, stock_buy.commission, self.currency)?;

            self.get_deposit_view(&stock_buy.symbol).transactions.push(
                Transaction::new(stock_buy.date, assets));
        }

        for stock_sell in &self.statement.stock_sells {
            {
                let deposit_view = self.get_deposit_view(&stock_sell.symbol);

                let assets = self.converter.convert_to(
                    stock_sell.date, stock_sell.price * stock_sell.quantity, self.currency)?;
                deposit_view.transactions.push(Transaction::new(stock_sell.date, -assets));

                let commission = self.converter.convert_to(
                    stock_sell.date, stock_sell.commission, self.currency)?;
                deposit_view.transactions.push(Transaction::new(stock_sell.date, commission));

                // TODO: Consider to send a marker to deposit emulator when for some period we have no
                // open positions.
                deposit_view.sell_transaction = Some((stock_sell.date, assets));
            }

            let tax_to_pay = stock_sell.tax_to_pay(&self.country, self.converter)?;
            self.process_tax(stock_sell.date, &stock_sell.symbol, tax_to_pay)?;
        }

        Ok(())
    }

    fn process_dividends(&mut self) -> EmptyResult {
        for dividend in &self.statement.dividends {
            if dividend.paid_tax.currency != dividend.amount.currency {
                return Err!(
                    "Dividend from {} at {}: The tax is paid in currency different from the dividend currency: {} vs {}",
                    dividend.issuer, formatting::format_date(dividend.date), dividend.paid_tax.currency,
                    dividend.amount.currency);
            }

            let mut amount = dividend.amount;
            amount.sub(dividend.paid_tax);
            let amount = self.converter.convert_to(dividend.date, amount, self.currency)?;

            self.get_deposit_view(&dividend.issuer).transactions.push(
                Transaction::new(dividend.date, -amount));

            let tax_to_pay = dividend.tax_to_pay(&self.country, self.converter)?;
            self.process_tax(dividend.date, &dividend.issuer, tax_to_pay)?;
        }

        Ok(())
    }

    fn process_tax(&mut self, income_date: Date, symbol: &String, tax_to_pay: Decimal) -> EmptyResult {
        // Treat tax payment as an ordinary deposit which we transfer to the account at tax payment
        // day.

        if tax_to_pay.is_zero() {
            return Ok(());
        }
        assert!(tax_to_pay.is_sign_positive());

        let tax_to_pay = Cash::new(self.country.currency, tax_to_pay);

        let mut tax_payment_date = self.country.get_tax_payment_date(income_date);
        if tax_payment_date > self.date {
            tax_payment_date = self.date;
        }

        let deposit_amount = self.converter.convert_to(
            tax_payment_date, tax_to_pay, self.currency)?;

        self.get_deposit_view(symbol).transactions.push(
            Transaction::new(tax_payment_date, deposit_amount));

        self.transactions.push(
            Transaction::new(tax_payment_date, deposit_amount));

        Ok(())
    }
}

struct StockDepositView {
    transactions: Vec<Transaction>,
    sell_transaction: Option<(Date, Decimal)>,
}

impl StockDepositView {
    fn new() -> StockDepositView {
        StockDepositView {
            transactions: Vec::new(),
            sell_transaction: None,
        }
    }
}

fn compare_to_bank_deposit(
    transactions: &Vec<Transaction>, current_date: Date, current_assets: Decimal
) -> GenericResult<(Decimal, Decimal)> {
    let start_date = transactions.first().unwrap().date;
    let start_assets = dec!(0);

    let emulate = |interest: Decimal| -> Decimal {
        let result_assets = DepositEmulator::emulate(
            start_date, start_assets, transactions, current_date, interest);

        let difference = (current_assets - result_assets).abs();

        difference
    };

    let mut interest = dec!(0);
    let mut difference = emulate(interest);

    for mut step in [decs!("1"), decs!("0.1"), decs!("0.01")].iter().cloned() {
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

    if precision >= decs!("0.01") {
        return Err!(concat!(
            "Failed to compare {} performance to bank deposit: ",
            "got a result with too low precision ({})"), name, util::round_to(precision, 3));
    }

    debug!("Got a result of comparing {} performance to bank deposit: {}% precision.",
           name, util::round_to(precision * dec!(100), 4));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparing_to_bank_deposit() {
        let (interest, difference) = compare_to_bank_deposit(
            &vec![Transaction::new(date!(28, 7, 2018), dec!(600000))],
            date!(28, 1, 2019), decs!("621486.34"),
        ).unwrap();

        assert_eq!(interest, dec!(7));
        assert!(difference < decs!("0.01"));
    }
}