use std::rc::Rc;

use chrono::Duration;
#[cfg(test)] use matches::assert_matches;

use crate::core::GenericResult;
use crate::currency::{self, Cash, CurrencyRate, cbr};
use crate::currency::rate_cache::{CurrencyRateCache, CurrencyRateCacheResult};
use crate::db;
use crate::formatting;
use crate::localities;
use crate::quotes::{Quotes, get_currency_pair};
use crate::time;
use crate::types::{Date, Decimal};
#[cfg(test)] use crate::util;

// Official CBR currency rate is calculated as following:
// 1. Every weekday a weighted average price is calculated for 10:00 - 11:30 period.
// 2. The calculated value is published around 15:00 and will be the official currency rate starting
//    from the next day.
// 3. The calculated currency rate will be valid until the next official currency rate.
//
// See https://bcs-express.ru/novosti-i-analitika/ofitsial-nyi-kurs-tsb-rf-kak-on-schitaetsia-i-kto-im-pol-zuetsia
// for details.
//
// So, effectively with some approximations CBR currency rate can be considered as T+2 relating to
// US stock market.
//
// The converter uses CBR currency rate for dates <= today and real time forex quotes for dates >
// today. It works great for both tax calculations where only official currency rates must be used
// and portfolio analysis / sell simulations where all calculations are processed in T+2 mode and
// forex quotes at conclusion date will be the closest approximation to the future CBR currency rate
// for trade execution date.
pub struct CurrencyConverter {
    backend: Box<dyn CurrencyConverterBackend>,
}

pub type CurrencyConverterRc = Rc<CurrencyConverter>;

impl CurrencyConverter {
    pub fn new(database: db::Connection, quotes: Option<Rc<Quotes>>, strict_mode: bool) -> CurrencyConverterRc {
        let rate_cache = CurrencyRateCache::new(database);
        let backend = CurrencyRateCacheBackend::new(rate_cache, quotes, strict_mode);
        Rc::new(CurrencyConverter::new_with_backend(backend))
    }

    #[cfg(test)]
    pub fn mock() -> CurrencyConverterRc {
        Rc::new(CurrencyConverter::new_with_backend(CurrencyRateCacheBackendMock::new()))
    }

    pub fn new_with_backend(source: Box<dyn CurrencyConverterBackend>) -> CurrencyConverter {
        CurrencyConverter { backend: source }
    }

    pub fn currency_rate(&self, date: Date, from: &str, to: &str) -> GenericResult<Decimal> {
        self.convert(from, to, date, dec!(1))
    }

    pub fn real_time_currency_rate(&self, from: &str, to: &str) -> GenericResult<Decimal> {
        self.currency_rate(self.real_time_date(), from, to)
    }

    /// Returns non-rounded currency rate. CBR provides currency rates with high precision like
    /// 56.3438 and tax statement uses currency rate value for 100 units like 5634.38.
    pub fn precise_currency_rate(&self, date: Date, from: &str, to: &str) -> GenericResult<Decimal> {
        self.currency_rate(date, from, to)
    }

    pub fn convert_to(&self, date: Date, cash: Cash, to: &str) -> GenericResult<Decimal> {
        self.convert(cash.currency, to, date, cash.amount)
    }

    pub fn real_time_convert_to(&self, cash: Cash, to: &str) -> GenericResult<Decimal> {
        self.convert_to(self.real_time_date(), cash, to)
    }

    // Implements rounding according to Russian taxation rules
    pub fn convert_to_rounding(&self, date: Date, cash: Cash, to: &str) -> GenericResult<Decimal> {
        Ok(currency::round(self.convert_to(date, cash.round(), to)?))
    }

    // Implements rounding according to Russian taxation rules
    pub fn convert_to_cash_rounding(&self, date: Date, cash: Cash, to: &str) -> GenericResult<Cash> {
        Ok(self.convert_to_cash(date, cash.round(), to)?.round())
    }

    pub fn convert_to_cash(&self, date: Date, cash: Cash, to: &str) -> GenericResult<Cash> {
        Ok(Cash::new(to, self.convert_to(date, cash, to)?))
    }

    pub fn convert(&self, from: &str, to: &str, date: Date, mut amount: Decimal) -> GenericResult<Decimal> {
        if from == to {
            return Ok(amount);
        }

        let (multiplier, divider) = self.backend.currency_rate(from, to, date)?;
        if let Some(multiplier) = multiplier {
            amount *= multiplier;
        }
        if let Some(divider) = divider {
            amount /= divider;
        }

        Ok(amount)
    }

    pub fn real_time_date(&self) -> Date {
        time::today_trade_execution_date()
    }
}

pub trait CurrencyConverterBackend {
    fn currency_rate(&self, from: &str, to: &str, date: Date) -> GenericResult<(Option<Decimal>, Option<Decimal>)>;
}

struct CurrencyRateCacheBackend {
    #[cfg(not(test))]
    cbr: cbr::Cbr,
    quotes: Option<Rc<Quotes>>,
    rate_cache: CurrencyRateCache,
    strict_mode: bool,
}

impl CurrencyRateCacheBackend {
    pub fn new(rate_cache: CurrencyRateCache, quotes: Option<Rc<Quotes>>, strict_mode: bool) -> Box<dyn CurrencyConverterBackend> {
        Box::new(CurrencyRateCacheBackend {
            #[cfg(not(test))]
            cbr: cbr::Cbr::new(),
            quotes,
            rate_cache,
            strict_mode,
        })
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
                        currency, formatting::format_date(date));
                }

                let currency_rates = self.get_rates(currency, start_date, end_date)?;
                self.rate_cache.save(currency, start_date, end_date, currency_rates)?;

                self.get_price(currency, date, true)?
            },
        })
    }

    #[cfg(not(test))]
    fn get_rates(&self, currency: &str, start_date: Date, end_date: Date) -> GenericResult<Vec<CurrencyRate>> {
        Ok(self.cbr.get_currency_rates(currency, start_date, end_date).map_err(|e| format!(
            "Failed to get currency rates from the Central Bank of the Russian Federation: {}", e))?)
    }

    #[cfg(test)]
    #[allow(clippy::unnecessary_wraps)]
    fn get_rates(&self, currency: &str, _start_date: Date, _end_date: Date) -> GenericResult<Vec<CurrencyRate>> {
        Ok(match currency {
            "USD" => vec![
                CurrencyRate {
                    date: date!(2018, 9, 1),
                    price: dec!(68.0447),
                },
                CurrencyRate {
                    date: date!(2018, 9, 4),
                    price: dec!(67.7443),
                },
            ],
            "EUR" => vec![
                CurrencyRate {
                    date: date!(2018, 9, 1),
                    price: dec!(79.4966),
                },
                CurrencyRate {
                    date: date!(2018, 9, 4),
                    price: dec!(78.6376),
                },
            ],
            _ => unreachable!(),
        })
    }
}

impl CurrencyConverterBackend for CurrencyRateCacheBackend {
    fn currency_rate(&self, from: &str, to: &str, date: Date) -> GenericResult<(Option<Decimal>, Option<Decimal>)> {
        let today = self.rate_cache.today();

        if
            // Strict mode is for tax calculations when we must provide only official currency rates
            self.strict_mode && date > today ||

            // Default mode for portfolio performance and other calculations where we have to
            // operate with future dates because of T+2 trade mode with vacations
            !self.strict_mode && date > today + Duration::days(7)
        {
            return Err!("An attempt to make currency conversion for future date: {}",
                formatting::format_date(date));
        }

        if !self.strict_mode && date > today {
            if let Some(ref quotes) = self.quotes {
                let price = quotes.get(&get_currency_pair(from, to))?;
                assert_eq!(price.currency, to);
                return Ok((Some(price.amount), None));
            }
        }

        let mut cur_date = date;
        let min_date = localities::get_russian_central_bank_min_last_working_day(cur_date);

        while cur_date >= min_date {
            let multiplier = if from == cbr::BASE_CURRENCY {
                None
            } else {
                Some(match self.get_price(from, cur_date, false)? {
                    Some(price) => price,
                    None => {
                        cur_date = cur_date.pred();
                        continue
                    },
                })
            };

            let divider = if to == cbr::BASE_CURRENCY {
                None
            } else {
                Some(match self.get_price(to, cur_date, false)? {
                    Some(price) => price,
                    None => {
                        cur_date = cur_date.pred();
                        continue
                    },
                })
            };

            return Ok((multiplier, divider));
        }

        Err!("Unable to find {}/{} currency rate for {} with {} days precision",
             from, to, formatting::format_date(date), (date - min_date).num_days())
    }
}

#[cfg(test)]
struct CurrencyRateCacheBackendMock {
}

#[cfg(test)]
impl CurrencyRateCacheBackendMock {
    fn new() -> Box<dyn CurrencyConverterBackend> {
        Box::new(CurrencyRateCacheBackendMock {})
    }
}

#[cfg(test)]
impl CurrencyConverterBackend for CurrencyRateCacheBackendMock {
    fn currency_rate(&self, from: &str, to: &str, _date: Date) -> GenericResult<(Option<Decimal>, Option<Decimal>)> {
        Err!("Unsupported currency rate conversion: {} -> {}", from, to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert() {
        let (_database, cache) = CurrencyRateCache::new_temporary();

        let amount = dec!(3);
        let today = cache.today();
        let converter = CurrencyConverter::new_with_backend(
            CurrencyRateCacheBackend::new(cache, None, true));

        for &currency in &["RUB", "USD", "EUR"] {
            assert_eq!(converter.convert(currency, currency, today, amount).unwrap(), amount);
        }

        let check = |from, to, date, amount, expected| {
            let precision = crate::types::DECIMAL_PRECISION - 2;
            let result = converter.convert(from, to, date, amount).unwrap();
            assert_eq!(util::round(result, precision), util::round(expected, precision));
        };

        for &(from, to, value, result) in &[
            ("USD", "RUB", amount, amount * dec!(68.0447)),
            ("RUB", "USD", amount * dec!(68.0447), amount),
            ("EUR", "RUB", amount, amount * dec!(79.4966)),
            ("RUB", "EUR", amount * dec!(79.4966), amount),
            ("EUR", "USD", amount, amount * dec!(79.4966) / dec!(68.0447)),
            ("USD", "EUR", amount * dec!(79.4966) / dec!(68.0447), amount),
        ] {
            assert_matches!(
                converter.convert(from, to, date!(2018, 8, 31), value),
                Err(ref e) if e.to_string().starts_with(&format!(
                    "Unable to find {}/{} currency rate", from, to))
            );

            for day in 1..4 {
                check(from, to, date!(2018, 9, day), value, result);
            }
        }

        for &(from, to, value, result) in &[
            ("USD", "RUB", amount, amount * dec!(67.7443)),
            ("RUB", "USD", amount * dec!(67.7443), amount),
            ("EUR", "RUB", amount, amount * dec!(78.6376)),
            ("RUB", "EUR", amount * dec!(78.6376), amount),
            ("EUR", "USD", amount, amount * dec!(78.6376) / dec!(67.7443)),
            ("USD", "EUR", amount * dec!(78.6376) / dec!(67.7443), amount),
        ] {
            let mut date = date!(2018, 9, 4);

            for _ in 0..6 {
                check(from, to, date, value, result);
                date = date.succ();
            }

            assert_matches!(
                converter.convert(from, to, date, value),
                Err(ref e) if e.to_string().starts_with(&format!(
                    "Unable to find {}/{} currency rate", from, to))
            );
        }
    }
}