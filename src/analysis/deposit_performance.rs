#[cfg(test)] use chrono::Duration;

use log::{debug, warn};

use crate::core::{GenericResult, EmptyResult};
use crate::currency::Cash;
use crate::types::Decimal;
use crate::util;

use super::deposit_emulator::{DepositEmulator, Transaction, InterestPeriod};

pub fn compare_to_bank_deposit(
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

pub fn check_emulation_precision(name: &str, currency: &str, assets: Decimal, difference: Decimal) -> EmptyResult {
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