use chrono::{Duration, Datelike};

use core::GenericResult;
use currency::{Cash, CurrencyRate};
use currency::rate_cache::{CurrencyRateCache, CurrencyRateCacheResult};
use db;
use types::{Date, Decimal};

pub struct CurrencyConverter {
    backend: Box<CurrencyConverterBackend>,
}

impl CurrencyConverter {
    pub fn new(connection: db::Connection) -> CurrencyConverter {
        let rate_cache = CurrencyRateCache::new(connection);
        let backend = CurrencyRateCacheBackend::new(rate_cache);
        CurrencyConverter::new_with_backend(backend)
    }

    pub fn new_with_backend(source: Box<CurrencyConverterBackend>) -> CurrencyConverter {
        return CurrencyConverter { backend: source }
    }

    pub fn convert_to(&self, date: Date, cash: Cash, to: &str) -> GenericResult<Decimal> {
        self.convert(cash.currency, to, date, cash.amount)
    }

    fn convert(&self, from: &str, to: &str, date: Date, amount: Decimal) -> GenericResult<Decimal> {
        self.backend.convert(from, to, date, amount)
    }
}

pub trait CurrencyConverterBackend {
    fn convert(&self, from: &str, to: &str, date: Date, amount: Decimal) -> GenericResult<Decimal>;
}

struct CurrencyRateCacheBackend {
    rate_cache: CurrencyRateCache,
}

impl CurrencyRateCacheBackend {
    pub fn new(rate_cache: CurrencyRateCache) -> Box<CurrencyConverterBackend> {
        return Box::new(CurrencyRateCacheBackend {rate_cache})
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

impl CurrencyConverterBackend for CurrencyRateCacheBackend {
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
            date: date!(1, 9, 2018),
            price: decs!("68.0447"),
        },
        CurrencyRate {
            date: date!(4, 9, 2018),
            price: decs!("67.7443"),
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert() {
        let (_database, cache) = CurrencyRateCache::new_temporary();

        let amount = dec!(3);
        let today = cache.today();
        let converter = CurrencyConverter::new_with_backend(CurrencyRateCacheBackend::new(cache));

        for currency in ["RUB", "USD"].iter() {
            assert_eq!(converter.convert(currency, currency, today, amount).unwrap(), amount);
        }

        for (from, to, value, result) in [
            ("USD", "RUB", amount, decs!("68.0447") * amount),
            ("RUB", "USD", decs!("68.0447") * amount, amount),
        ].iter() {
            assert_matches!(
                converter.convert(from, to, date!(31, 8, 2018), *value),
                Err(ref e) if e.to_string().starts_with("Unable to find USD currency rate")
            );

            for day in 1..4 {
                assert_eq!(
                    converter.convert(from, to, date!(day, 9, 2018), *value).unwrap(),
                    *result
                );
            }
        }

        for (from, to, value, result) in [
            ("USD", "RUB", amount, decs!("67.7443") * amount),
            ("RUB", "USD", decs!("67.7443") * amount, amount),
        ].iter() {
            let mut date = date!(4, 9, 2018);

            for _ in 0..4 {
                assert_eq!(converter.convert(from, to, date, *value).unwrap(), *result);
                date += Duration::days(1);
            }

            assert_matches!(
                converter.convert(from, to, date, *value),
                Err(ref e) if e.to_string().starts_with("Unable to find USD currency rate")
            );
        }
    }
}