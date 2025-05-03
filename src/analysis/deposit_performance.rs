use std::cmp::Ordering;

#[cfg(test)] use chrono::Duration;
use indoc::indoc;
use itertools::Itertools;
use log::{self, log_enabled, trace, warn};

use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::formatting;
use crate::types::Decimal;
use crate::util;

use super::deposit_emulator::{DepositEmulator, Transaction, InterestPeriod};

pub fn compare_instrument_to_bank_deposit(
    name: &str, currency: &str, transactions: &[Transaction], interest_periods: &[InterestPeriod],
    current_assets: Decimal
) -> GenericResult<Option<Decimal>> {
    compare_to_bank_deposit(transactions, interest_periods, current_assets)
        .map(|(interest, difference)| -> GenericResult<Decimal> {
            check_emulation_precision(name, currency, transactions, current_assets, interest, difference)?;
            Ok(interest)
        })
        .transpose()
}

fn compare_to_bank_deposit(
    transactions: &[Transaction], interest_periods: &[InterestPeriod], current_assets: Decimal
) -> Option<(Decimal, Decimal)> {
    if log_enabled!(log::Level::Trace) {
        let transactions = transactions.iter().map(|transaction| {
            format!("{}: {}", formatting::format_date(transaction.date), transaction.amount)
        }).join(", ");

        let interest_periods = interest_periods.iter().map(|period| {
            format!("{} - {}", formatting::format_date(period.start), formatting::format_date(period.end))
        }).join(", ");

        trace!(indoc!("
            Comparing the following cash flows to deposit performance:
            * Transactions: {}
            * Interest periods: {}
            * Result: {}"),
            transactions, interest_periods, current_assets);
    }

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

    for (index, mut step) in [dec!(10), dec!(1), dec!(0.1), dec!(0.01)].iter().cloned().enumerate() {
        let decreasing_difference = emulate(interest - step);
        let increasing_difference = emulate(interest + step);

        if decreasing_difference > difference && difference < increasing_difference {
            continue;
        }

        let next_difference = match decreasing_difference.cmp(&increasing_difference) {
            Ordering::Less => {
                step = -step;
                decreasing_difference
            },

            Ordering::Greater => {
                increasing_difference
            },

            Ordering::Equal => decreasing_difference,
        };

        if next_difference == difference {
            if index == 0 {
                // Some assets can be acquired for free due to corporate actions or other non-trading operations. In
                // this case we can't calculate their performance.
                //
                // An example is spinoff corporate action, where we have:
                // * no buy transaction (it's zero cost)
                // * a big negative transaction from stock selling
                // * two small positive (commission + tax)
                // * effectively zero interest period with positive balance
                return None;
            } else {
                // When we have a very big/small interest (huge profit / liquidation), it's OK that small changes in
                // interest may not affect the calculation result.
                break;
            }
        }

        interest += step;
        difference = next_difference;

        loop {
            let next_interest = interest + step;
            let next_difference = emulate(next_interest);

            if next_difference >= difference {
                break;
            }

            difference = next_difference;
            interest = next_interest;
        }
    }

    Some((interest, difference))
}

fn check_emulation_precision(
    name: &str, currency: &str, transactions: &[Transaction], current_assets: Decimal,
    interest: Decimal, difference: Decimal,
) -> EmptyResult {
    // It's actually hard to find the suitable assets amount to check the difference against:
    // 1. Cash assets may be very small or even zero
    // 2. Last stock selling transaction very small (fractional shares as an extremum)
    // 3. Stock selling transaction may be followed by years of inactivity and then - small tax
    //    payment transaction (for accounts that are taxed on their close).
    //
    // Considering the said above, don't try to overcomplicate the logic just for the sake of
    // emulation precision checking.

    let assets = std::cmp::max(current_assets, transactions.iter().map(|transaction| {
        transaction.amount.abs()
    }).max().unwrap());

    let precision = (difference / assets).abs() * dec!(100);
    let difference = Cash::new(currency, difference).round();

    if precision >= dec!(0.1) {
        warn!(concat!(
            "Failed to compare {} {} performance to bank deposit: ",
            "got a result with too low precision: {}% ({}%, {})."
        ), name, currency, interest, util::round(precision, 3), difference);
        return Ok(());
    }

    trace!("Got a result of comparing {} {} performance to bank deposit: {}% ({}% precision, {}).",
           name, currency, interest, util::round(precision, 4), difference);

    Ok(())
}

#[cfg(test)]
mod tests {
    use matches::assert_matches;
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

            let open_date = date!(2018, 7, 28);
            let close_date = date!(2019, 1, 28);

            transactions.extend([
                // Fake transaction outside of interest period
                Transaction::new(open_date - Duration::days(100), dec!(400_000)),

                // Deposit opening transaction
                Transaction::new(open_date, dec!(200_000)),
            ]);

            for &(capitalization_date, assets) in &[
                (date!(2018,  8, 28), dec!(603_567.12)),
                (date!(2018,  9, 28), dec!(607_155.45)),
                (date!(2018, 10, 28), dec!(610_648.68)),
                (date!(2018, 11, 28), dec!(614_279.11)),
                (date!(2018, 12, 28), dec!(617_813.32)),
                (date!(2019,  1, 28), dec!(621_486.34)),
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
            transactions.push(Transaction::new(date!(2019, 1, 29), dec!(-200_000)));
            compare(&transactions, &interest_periods, dec!(100_000));

            // Deposit some assets between interest periods
            transactions.push(Transaction::new(date!(2019, 1, 30), dec!(50_000)));
            compare(&transactions, &interest_periods, dec!(150_000));
        }

        {
            // Second deposit

            let open_date = date!(2019, 1, 31);
            let close_date = date!(2019, 7, 31);

            // Deposit more assets at open date
            transactions.push(Transaction::new(open_date, dec!(40_000)));
            compare(&transactions, &interest_periods, dec!(190_000));

            // Deposit contributions
            transactions.extend([
                Transaction::new(date!(2019, 2,  5), dec!(60_000)),
                Transaction::new(date!(2019, 2, 21), dec!(50_000)),
            ]);

            for &(capitalization_date, assets) in &[
                (date!(2019, 2, 28), dec!(301_352.05)),
                (date!(2019, 3, 31), dec!(303_143.65)),
                (date!(2019, 4, 30), dec!(304_887.77)),
                (date!(2019, 5, 31), dec!(306_700.39)),
                (date!(2019, 6, 30), dec!(308_464.97)),
                (date!(2019, 7, 31), dec!(310_298.85)),
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

    #[test]
    fn spinoff_sell_simulation() {
        // This is an example of transactions which we can get during sell simulation of stock which
        // we've got "for free" due to spinoff corporate action.

        let transactions = vec![
            // Sell
            Transaction::new(date!(2022, 2,  4), dec!(-16.58)),

            // Commission
            Transaction::new(date!(2022, 2,  4), dec!(1)),

            // Tax
            Transaction::new(date!(2023, 3, 15), dec!(2.30)),
        ];

        let interest_periods = vec![
            InterestPeriod::new(date!(2021, 11, 12), date!(2022, 2, 4)),
        ];

        assert_matches!(compare_to_bank_deposit(&transactions, &interest_periods, dec!(0)), None);
    }
}