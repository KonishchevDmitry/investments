use chrono::Duration;

use broker_statement::BrokerStatement;
use core::{EmptyResult, GenericResult};
use currency::Cash;
use currency::converter::CurrencyConverter;
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
}

impl <'a> PortfolioPerformanceAnalyser<'a> {
    pub fn analyse(
        statement: &BrokerStatement, currency: &str, converter: &CurrencyConverter
    ) -> GenericResult<(Decimal, Decimal, Decimal)> {
        let mut analyser = PortfolioPerformanceAnalyser {
            statement: statement,
            currency: currency,
            converter: converter,

            date: statement.period.1 - Duration::days(1),
            country: regulations::russia(),
            transactions: Vec::new(),
        };

        // TODO: Withdrawals support
        analyser.process_deposits()?;
        analyser.process_dividends()?;
        analyser.transactions.sort_by_key(|assets| assets.date);

        let mut deposits = dec!(0);
        for transaction in &analyser.transactions {
            deposits += transaction.amount;
        }

        // FIXME: Take taxes from positions selling into account
        // Assume that the caller has simulated sellout and just check it here
        if !statement.open_positions.is_empty() {
            return Err!("Unable to calculate current assets: The broker statement has open positions");
        }

        // FIXME: date
        let current_assets = statement.cash_assets.total_assets(currency, converter)?;
        let (interest, precision) = analyser.compare_to_bank_deposit(current_assets)?;

        debug!(concat!(
            "Got a result of comparing portfolio performance to bank deposit ",
            "for {} currency with {}% precision."),
            currency, util::round_to(precision * dec!(100), 4));

        Ok((deposits, current_assets, interest))
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

    fn process_dividends(&mut self) -> EmptyResult {
        // Treat tax from dividends as an ordinary deposit which we transfer to the account at tax
        // payment day.

        for dividend in &self.statement.dividends {
            let tax_to_pay = Cash::new(
                self.country.currency, dividend.tax_to_pay(&self.country, self.converter)?);

            let mut tax_payment_date = self.country.get_tax_payment_date(dividend.date);
            if tax_payment_date > self.date {
                tax_payment_date = self.date;
            }

            let deposit_amount = self.converter.convert_to(
                tax_payment_date, tax_to_pay, self.currency)?;

            self.transactions.push(Transaction::new(tax_payment_date, deposit_amount));
        }

        Ok(())
    }

    fn compare_to_bank_deposit(&self, current_assets: Decimal) -> GenericResult<(Decimal, Decimal)> {
        let start_date = self.statement.period.0;
        let start_assets = dec!(0);

        let emulate = |interest: Decimal| -> GenericResult<Decimal> {
            let result_assets = DepositEmulator::emulate(
                start_date, start_assets, &self.transactions, self.date, interest)?;

            let difference = (current_assets - result_assets).abs();

            Ok(difference)
        };

        let mut interest = dec!(0);
        let mut difference = emulate(interest)?;

        for mut step in [decs!("1"), decs!("0.1"), decs!("0.01")].iter().cloned() {
            let decreasing_difference = emulate(interest - step)?;
            let increasing_difference = emulate(interest + step)?;

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
                let next_difference = emulate(next_interest)?;

                if next_difference > difference {
                    break;
                }

                difference = next_difference;
                interest = next_interest;
            }
        }

        let precision = difference / current_assets;
        if precision >= decs!("0.01") {
            return Err!(concat!(
                "Failed to compare portfolio performance to bank deposit: ",
                "got a result with too low precision ({})"), util::round_to(precision, 3));
        }

        Ok((interest, precision))
    }
}