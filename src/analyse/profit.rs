use chrono::Duration;

use core::{EmptyResult, GenericResult};
use currency::CacheAssets;
use currency::converter::{CurrencyConverter, CurrencyConverterBackend};
use types::{Date, Decimal};

struct AssetsInterval {
    duration: Duration,
    amount: Decimal,
}

// FIXME: Support:
// * Withdrawals
// * Non-zero starting point
pub fn get_average_profit_from_cache(
    mut deposits: Vec<CacheAssets>, current_assets: CacheAssets, converter: &CurrencyConverter
) -> GenericResult<Decimal> {
    let currency = "RUB"; // FIXME

    let mut deposit_assets = Decimal::from(0);
    let mut interval_assets = Decimal::from(0);
    let mut interval_income = Decimal::from(0);

    let mut assets_intervals = Vec::<AssetsInterval>::new();

    deposits.sort_by_key(|assets| assets.date);

    for (index, assets) in deposits.iter().enumerate() {
        if assets.date >= current_assets.date {
            break
        }

        let amount = converter.convert_to(assets.date, assets.cash, currency)?;
        deposit_assets += amount;
        interval_assets += amount;

        let end_date = if index < deposits.len() - 1 {
            deposits[index + 1].date
        } else {
            current_assets.date
        };

        let duration = end_date - assets.date;

        interval_income += interval_assets * Decimal::from(duration.num_days());

        assets_intervals.push(AssetsInterval {
            duration: duration,
            amount: interval_assets,
        })
    }

    let profit = converter.convert_to(current_assets.date, current_assets.cash, currency)? - deposit_assets;
    let profit_interest = profit / interval_income * deci!(365);

    Ok(profit_interest)
}

#[cfg(test)]
mod tests {
    use super::*;

    /*
    macro_rules! parametrized_tests {
        ($($name:ident: $args:expr,)*) => {
        $(
            #[test]
            fn $name() {
                basic($args);
            }
        )*
        }
    }

    fib_tests! {
        fib_0: (0, 0),
        fib_1: (1, 1),
        fib_2: (2, 1),
        fib_3: (3, 2),
        fib_4: (4, 3),
        fib_5: (5, 5),
        fib_6: (6, 8),
    }
    */

    #[test]
    fn basic() {
        struct ConverterBackendMock {
        }

        impl CurrencyConverterBackend for ConverterBackendMock {
            fn convert(&self, from: &str, to: &str, date: Date, amount: Decimal) -> GenericResult<Decimal> {
                Ok(match (from, to) {
                    ("RUB", "RUB") => amount,
                    ("USD", "RUB") => amount * deci!(100),
                    _ => unreachable!(),
                })
            }
        }

        let currency = "RUB";
        let converter = CurrencyConverter::new(Box::new(ConverterBackendMock {}));

        for (other_currency, other_currency_amount) in [
            ("RUB", deci!(100)),
            ("USD", deci!(1)),
        ].iter() {
            let deposits = vec![
                CacheAssets::new(date!(10, 1, 2017), currency, deci!(100)),
                CacheAssets::new(date!(10, 9, 2017), other_currency, *other_currency_amount),
            ];

            // Emulate a deposit with 12% interest
            let year_interest = decs!("0.12");
            let month_interest = year_interest / deci!(12);
            let current_assets = (
                deci!(200) +
                deci!(100) * month_interest * deci!(8) +
    //            deci!(200) * monthly_interest * deci!(4)
                deci!(208) * month_interest * deci!(4)
            );
            assert_eq!(current_assets, decs!("216.32"));
    //        assert_eq!(current_assets, deci!(216));

            let current_assets = CacheAssets::new(date!(10, 1, 2018), currency, current_assets);

            let year_interest_with_capitalization = (
                deci!(100) * month_interest * deci!(8) +
                (deci!(200) + deci!(100) * month_interest * deci!(8)) * month_interest * deci!(4)
            ) / Decimal::from(8 * 100 + 4 * 200) * deci!(12);
            assert_eq!(year_interest_with_capitalization, decs!("0.1224"));

            let profit_interest = get_average_profit_from_cache(
                deposits, current_assets, &converter).unwrap();

            assert!(year_interest < profit_interest);
            assert!(profit_interest < year_interest_with_capitalization);
        }
    }
}