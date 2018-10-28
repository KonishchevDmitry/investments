use chrono::Duration;

use broker_statement::BrokerStatement;
use core::GenericResult;
use currency::CashAssets;
use currency::converter::CurrencyConverter;
use types::{Date, Decimal};

use super::deposit_emulator::{DepositEmulator, Transaction};

/// Calculates average rate of return from cash investments by comparing portfolio performance to
/// performance of a bank deposit with exactly the same investments and monthly capitalization.
pub struct AverageRateOfReturnCalculator<'a> {
    statement: &'a BrokerStatement,
    currency: &'a str,
    converter: &'a CurrencyConverter,
    result_date: Date,
}

impl <'a>AverageRateOfReturnCalculator<'a> {
    pub fn calculate(
        statement: &BrokerStatement, currency: &str, converter: &CurrencyConverter
    ) -> GenericResult<Decimal> {
        let calculator = AverageRateOfReturnCalculator {
            statement: statement,
            currency: currency,
            converter: converter,
            result_date: statement.period.1 - Duration::days(1),
        };

        let transactions = calculator.get_transactions()?;
        let result_assets = calculator.get_result_assets()?;
        let interest = calculator.compare_to_bank_deposit(&transactions, result_assets)?;

        Ok(interest)
    }

    fn get_transactions(&self) -> GenericResult<Vec<Transaction>> {
        if self.statement.deposits.is_empty() {
            return Err!("Broker statement contains no deposits");
        }

        // TODO: Withdrawals support
        let mut transactions = Vec::<Transaction>::new();

        for mut deposit in self.statement.deposits.iter().cloned() {
            assert!(deposit.cash.is_positive());
            deposit.cash.amount += self.statement.broker.get_deposit_commission(deposit)?;
            let amount = self.converter.convert_to(deposit.date, deposit.cash, self.currency)?;

            transactions.push(Transaction::new(deposit.date, amount));
        }

        transactions.sort_by_key(|assets| assets.date);

        Ok(transactions)
    }

    fn get_result_assets(&self) -> GenericResult<Decimal> {
        // FIXME: Calculate manually, take taxes into account
        let total_value = CashAssets::new_from_cash(
            // FIXME: HERE
            self.statement.period.1 /*- Duration::days(1)*/, self.statement.total_value);

        // FIXME: Take taxes into account
        let result_assets = self.converter.convert_to(
            total_value.date, total_value.cash, self.currency)?;

        Ok(result_assets)
    }

    fn compare_to_bank_deposit(
        &self, transactions: &Vec<Transaction>, result_assets: Decimal
    ) -> GenericResult<Decimal> {
        let start_date = self.statement.period.0;
        let start_assets = dec!(0);

        let emulate = |interest: Decimal| -> GenericResult<Decimal> {
            let assets = DepositEmulator::emulate(
                start_date, start_assets, &transactions, self.result_date, interest)?;

            let difference = (result_assets - assets).abs();

            Ok(difference)
        };

        let mut interest = dec!(0);
        let mut difference = emulate(interest)?;

        for mut step in [decs!("1"), decs!("0.1"), decs!("0.01")].iter().cloned() {
            let decreasing_difference = emulate(interest - step)?;
            let increasing_difference = emulate(interest + step)?;

            if decreasing_difference > difference && difference < increasing_difference {
                return Ok(interest);
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

        Ok(interest)
    }
}