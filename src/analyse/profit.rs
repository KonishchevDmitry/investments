use core::GenericResult;
use currency::CacheAssets;
use currency::converter::{CurrencyConverter, CurrencyConverterBackend};
use types::{Date, Decimal};

// FIXME: Support:
// * Withdrawals
// * Non-zero starting point
// * Compare with complex interest
// * Calculate taxes
pub fn get_average_profit(
    deposits: &Vec<CacheAssets>, current_assets: CacheAssets, currency: &str,
    converter: &CurrencyConverter
) -> GenericResult<Decimal> {
    // Calculates average profit from cash income. Splits the whole period into intervals, where
    // we have a "constant" assets in each interval.
    //
    // profit = current_assets - total_income
    // (assets * days + assets * days + ...) * interest = profit

    let mut total_income = dec!(0);
    let mut relative_contributions = dec!(0);

    let mut transactions = Vec::<CacheAssets>::new();

    for deposit in deposits {
        if deposit.date > current_assets.date {
            continue;
        }

        assert!(deposit.cash.amount > dec!(0));
        transactions.push(*deposit);
    }

    transactions.sort_by_key(|assets| assets.date);

    for (index, assets) in transactions.iter().enumerate() {
        total_income += converter.convert_to(assets.date, assets.cash, currency)?;
        if total_income < dec!(0) {
            return Err!("Portfolio got negative balance on {}", assets.date);
        }

        let end_date = if index < deposits.len() - 1 {
            deposits[index + 1].date
        } else {
            current_assets.date
        };

        let days = (end_date - assets.date).num_days();
        relative_contributions += total_income * Decimal::from(days);
    }

    if relative_contributions == dec!(0) {
        return Err!("There are no deposits for the specified period")
    }

    let converted_current_assets = converter.convert_to(
        current_assets.date, current_assets.cash, currency)?;

    let profit = converted_current_assets - total_income;
    let interest = profit / relative_contributions * dec!(365);

    Ok(interest)
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! basic_tests {
        ($($name:ident: $args:expr,)*) => {
        $(
            #[test]
            fn $name() {
                let (other_currency, other_amount) = $args;
                basic(other_currency, other_amount);
            }
        )*
        }
    }

    basic_tests! {
        basic_rub: ("RUB", dec!(100)),
        basic_usd: ("USD", dec!(1)),
    }

    fn basic(other_currency: &str, other_amount: Decimal) {
        struct ConverterMock {}
        impl CurrencyConverterBackend for ConverterMock {
            fn convert(&self, from: &str, to: &str, _date: Date, amount: Decimal) -> GenericResult<Decimal> {
                Ok(match (from, to) {
                    ("RUB", "RUB") => amount,
                    ("USD", "RUB") => amount * dec!(100),
                    _ => unreachable!(),
                })
            }
        }

        let currency = "RUB";
        let converter = CurrencyConverter::new(Box::new(ConverterMock {}));

        let deposits = vec![
            CacheAssets::new(date!(10, 1, 2013), currency, dec!(100)),
            CacheAssets::new(date!(10, 5, 2014), other_currency, other_amount),
        ];

        let year_interest = decs!("0.12");
        let month_interest = year_interest / dec!(12);

        // Emulate a bank deposit with 12% interest and capitalization on each income

        let current_assets =
            dec!(200) +
            dec!(100) * month_interest * dec!(16) +
            (dec!(200) + dec!(100) * month_interest * dec!(16)) * month_interest * dec!(8);
        assert_eq!(current_assets, decs!("233.28"));

        let year_interest_with_capitalization =
            (current_assets - dec!(200)) / Decimal::from(16 * 100 + 8 * 200) * dec!(12);
        assert_eq!(year_interest_with_capitalization, decs!("0.1248"));

        let current_assets = CacheAssets::new(date!(10, 1, 2015), currency, current_assets);
        let profit_interest = get_average_profit(
            &deposits, current_assets, currency, &converter).unwrap();

        assert!(year_interest < profit_interest);
        assert!(profit_interest < year_interest_with_capitalization);
    }
}