#[cfg(test)] use std::str::FromStr;

use chrono::{Duration, Datelike};

use core::GenericResult;
use currency::CurrencyRate;
use currency::name_cache;
use currency::rate_cache::{CurrencyRateCache, CurrencyRateCacheResult};
use types::{Date, Decimal};

pub struct CurrencyConverter {
    base_currency: &'static str,
    rate_cache: CurrencyRateCache,
}

impl CurrencyConverter {
    pub fn new(base_currency: &str, rate_cache: CurrencyRateCache) -> CurrencyConverter {
        return CurrencyConverter {
            base_currency: name_cache::get(base_currency),
            rate_cache: rate_cache,
        }
    }

    fn convert(&self, from: &str, to: &str, date: Date, amount: Decimal) -> GenericResult<Decimal> {
        if from == to {
            return Ok(amount);
        }

        let (currency, inverse) = match (from, to) {
            ("USD", "RUB") => ("USD", false),
            ("RUB", "USD") => ("USD", true),
            _ => return Err!("Unsupported currency conversion: {} -> {}", from, to),
        };

        let today = self.rate_cache.today();
        if date > today {
            return Err!("An attempt to make currency conversion for future date: {}", date);
        }

        let mut cur_date = date;
        if cur_date == today {
            // FIXME: Should we return a error in this case by default?
            cur_date -= Duration::days(1);
        }

        let min_date = match date {
            date if date.month() == 1 && date.day() < 10 => Date::from_ymd(date.year(), 12, 30),
            date if (date.month() == 3 || date.month() == 5) && date.day() < 13 => date - Duration::days(5),
            date => date - Duration::days(3),
        };

        while cur_date >= min_date {
            if let Some(price) = self.get_price(currency, cur_date, false)? {
                return Ok(if inverse {
                    amount / price
                } else {
                    price * amount
                });
            }

            cur_date -= Duration::days(1);
        }

        Err!("Unable to find {} currency rate for {} with {} days precision",
             currency, date, (date - min_date).num_days())
    }

    fn get_price(&self, currency: &str, date: Date, from_cache_only: bool) -> GenericResult<Option<Decimal>> {
        let cache_result = self.rate_cache.get(currency, date).map_err(|e| format!(
            "Failed to get currency rate from the currency rate cache: {}", e))?;

        Ok(match cache_result {
            CurrencyRateCacheResult::Exists(cached_value) => cached_value,
            CurrencyRateCacheResult::Missing(start_date, end_date) => {
                if from_cache_only {
                    return Err!(concat!(
                        "Failed to get {} currency rate for {}: ",
                        "it's expected to be in the cache, but actually it's missing"),
                        currency, date);
                }

                let currency_rates = get_currency_rates(currency, start_date, end_date)?;
                self.rate_cache.save(currency, start_date, end_date, currency_rates)?;

                self.get_price(currency, date, true)?
            },
        })
    }
}

#[cfg(not(test))]
fn get_currency_rates(currency: &str, start_date: Date, end_date: Date) -> GenericResult<Vec<CurrencyRate>> {
    Ok(::currency::cbr::get_rates(currency, start_date, end_date).map_err(|e| format!(
        "Failed to get currency rates from the Central Bank of the Russian Federation: {}", e))?)
}

#[cfg(test)]
fn get_currency_rates(currency: &str, _start_date: Date, _end_date: Date) -> GenericResult<Vec<CurrencyRate>> {
    assert_eq!(currency, "USD");

    Ok(vec![
        CurrencyRate {
            date: Date::from_ymd(2018, 9, 1),
            price: Decimal::from_str("68.0447").unwrap(),
        },
        CurrencyRate {
            date: Date::from_ymd(2018, 9, 4),
            price: Decimal::from_str("67.7443").unwrap(),
        },
    ])
}

// FIXME: HERE
#[cfg(test)]
mod tests {
    use bigdecimal::FromPrimitive;
    use super::*;

    #[test]
    fn convert() {
        let (_database, cache) = CurrencyRateCache::new_temporary();

        let today = cache.today();
        let converter = CurrencyConverter::new("RUB", cache);

        for currency in ["RUB", "USD"].iter() {
            let amount = Decimal::from_i64(123).unwrap();
            assert_eq!(converter.convert(currency, currency, today, amount.clone()).unwrap(), amount);
        }

        let precision = 3;
        let amount = Decimal::from_i64(3).unwrap();

        for day in 1..4 {
            for (from, to, value, result) in [
                ("USD", "RUB", amount.clone(), Decimal::from_str("68.0447").unwrap() * amount.clone()),
                ("RUB", "USD", Decimal::from_str("68.0447").unwrap() * amount.clone(), amount.clone()),
            ].iter() {
                assert_eq!(
                    converter.convert(from, to, Date::from_ymd(2018, 9, day), value.clone()).unwrap(),
                    result.clone()
                );
            }
        }

        for day in 0..(precision + 1) {
            assert_eq!(
                converter.convert("USD", "RUB", Date::from_ymd(2018, 9, 4) + Duration::days(day), Decimal::from_i64(1).unwrap()).unwrap(),
                Decimal::from_str("67.7443").unwrap(),
            );
        }

        assert_matches!(
            converter.convert("USD", "RUB", Date::from_ymd(2018, 9, 4) + Duration::days(precision + 1), Decimal::from_i64(1).unwrap()),
            Err(ref e) if e.to_string().starts_with("Unable to find USD currency rate")
        );
//
//        for day in 4..7 {
//            assert_eq!(
//                converter.convert("USD", "RUB", Date::from_ymd(2018, 9, day), Decimal::from_i64(1).unwrap()).unwrap(),
//                Decimal::from_str("67.7443").unwrap(),
//            );
//        }
    }
}