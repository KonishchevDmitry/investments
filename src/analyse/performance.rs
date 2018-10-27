use core::GenericResult;
use currency::CashAssets;
use currency::converter::CurrencyConverter;
#[cfg(test)] use currency::converter::CurrencyConverterBackend;
use types::{Date, Decimal};
use util;

use super::deposit_emulator::DepositEmulator;

// FIXME: Support:
// * Withdrawals
// * Take taxes into account
// * Deposit fees
/// Calculates average rate of return from cash investments by comparing portfolio performance to
/// performance of bank deposit with the exactly same investments and monthly capitalization.
pub fn get_average_rate_of_return(
    deposits: &Vec<CashAssets>, current_assets: CashAssets, currency: &str,
    converter: &CurrencyConverter
) -> GenericResult<Decimal> {
    let mut transactions = Vec::<CashAssets>::new();

    for deposit in deposits {
        if deposit.date > current_assets.date {
            return Err!("Got a deposit from the future ({})", util::format_date(deposit.date));
        }

        assert!(deposit.cash.is_positive());
        transactions.push(*deposit);
    }

    transactions.sort_by_key(|assets| assets.date);

    // FIXME: Support custom starting point
    assert_ne!(transactions.len(), 0);
    let start_date = transactions[0].date;
    let start_assets = dec!(0);

    let result_assets = converter.convert_to(current_assets.date, current_assets.cash, currency)?;

    // FIXME: DepositEmulator shouldn't know anything about currency conversion
    let emulate = |interest: Decimal| -> GenericResult<Decimal> {
        let assets = DepositEmulator::emulate(
            start_date, start_assets, &transactions, current_assets.date, currency, interest,
            converter)?;

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