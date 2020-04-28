use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use chrono::Duration;
use diesel::{self, prelude::*};
#[cfg(test)] use matches::assert_matches;
#[cfg(test)] use tempfile::NamedTempFile;

use crate::core::{GenericResult, GenericError, EmptyResult};
use crate::currency::CurrencyRate;
use crate::db::{self, schema::currency_rates, models};
use crate::formatting;
use crate::types::{Date, Decimal};
use crate::util::{self, DecimalRestrictions};

// Official CBR currency rate is calculated as following:
// 1. Every weekday a weighted average price is calculated for 10:00 - 11:30 period.
// 2. The calculated value is published around 15:00 and will be the official currency rate starting
//    from the next day.
// 3. The calculated currency rate will be valid until the next official currency rate.
//
// See https://bcs-express.ru/novosti-i-analitika/ofitsial-nyi-kurs-tsb-rf-kak-on-schitaetsia-i-kto-im-pol-zuetsia
// for details.
//
// We request data until tomorrow only to be able to fill today date if it's monday (when there is
// no data from sunday for monday, but will be data from monday for tuesday), but don't save
// tomorrow's currency rates - just in case: we don't actually need them, but by not saving them we
// can handle a possible corrections, for example.
pub struct CurrencyRateCache {
    today: Date,
    tomorrow: Date,

    db: db::Connection,
    in_memory_missing: Mutex<HashMap<String, HashSet<Date>>>,
}

impl CurrencyRateCache {
    pub fn new(connection: db::Connection) -> CurrencyRateCache {
        let today = util::today();
        CurrencyRateCache {
            today: today,
            tomorrow: today + Duration::days(1),

            db: connection,
            in_memory_missing: Mutex::new(HashMap::new()),
        }
    }

    #[cfg(test)]
    pub fn new_temporary() -> (NamedTempFile, CurrencyRateCache) {
        let (database, connection) = db::new_temporary();
        (database, CurrencyRateCache::new(connection))
    }

    pub fn today(&self) -> Date {
        self.today
    }

    pub fn get(&self, currency: &str, date: Date) -> GenericResult<CurrencyRateCacheResult> {
        if date > self.today {
            return Err!("An attempt to get currency rate for the future")
        }

        if let Some(missing) = self.in_memory_missing.lock().unwrap().get(currency) {
            if missing.get(&date).is_some() {
                return Ok(CurrencyRateCacheResult::Exists(None));
            }
        }

        self.db.transaction::<_, GenericError, _>(|| {
            let result = currency_rates::table
                .select(currency_rates::price)
                .filter(currency_rates::currency.eq(currency))
                .filter(currency_rates::date.eq(date))
                .get_result::<Option<String>>(&*self.db).optional()?;

            if let Some(cached_price) = result {
                return Ok(CurrencyRateCacheResult::Exists(match cached_price {
                    Some(price) => Some(
                        util::parse_decimal(&price, DecimalRestrictions::StrictlyPositive).map_err(|_| format!(
                            "Got an invalid price from the database: {:?}", price))?),
                    None => None,
                }));
            }

            let start_date = {
                let result = currency_rates::table
                    .select(currency_rates::date)
                    .filter(currency_rates::currency.eq(currency))
                    .filter(currency_rates::date.lt(date))
                    .order(currency_rates::date.desc())
                    .limit(1)
                    .get_result::<Date>(&*self.db).optional()?;

                match result {
                    Some(last_date) => last_date + Duration::days(1),
                    None => date - Duration::days(365),
                }
            };

            let end_date = {
                let result = currency_rates::table
                    .select(currency_rates::date)
                    .filter(currency_rates::currency.eq(currency))
                    .filter(currency_rates::date.gt(date))
                    .filter(currency_rates::price.is_not_null())
                    .order(currency_rates::date.asc())
                    .limit(1)
                    .get_result::<Date>(&*self.db).optional()?;

                match result {
                    Some(first_date) => first_date,
                    None => self.tomorrow,
                }
            };

            assert!(start_date <= end_date);
            Ok(CurrencyRateCacheResult::Missing(start_date, end_date))
        })
    }

    pub fn save(&self, currency: &str, start_date: Date, end_date: Date, mut rates: Vec<CurrencyRate>) -> EmptyResult {
        if start_date > end_date {
            return Err!("Invalid date range: {} - {}",
                formatting::format_date(start_date), formatting::format_date(end_date));
        } else if end_date > self.tomorrow {
            return Err!("An attempt to save currency rates for the future");
        }

        if !rates.is_empty() {
            rates.sort_by_key(|rate| rate.date);
            if rates.first().unwrap().date < start_date || rates.last().unwrap().date > end_date {
                return Err!("The specified currency rates don't match the specified date range");
            }
        }

        let mut last_date = None;
        let mut rows = Vec::new();

        for rate in &rates {
            {
                let mut date = match last_date {
                    Some(date) => date + Duration::days(1),
                    None => start_date,
                };

                while date < rate.date {
                    rows.push(models::NewCurrencyRate {
                        currency: currency,
                        date: date,
                        price: None,
                    });
                    date += Duration::days(1);
                }
            }
            last_date.replace(rate.date);

            if rate.date == self.tomorrow {
                continue;
            }
            assert!(rate.date <= self.today);

            rows.push(models::NewCurrencyRate {
                currency: currency,
                date: rate.date,
                price: Some(rate.price.to_string()),
            });
        }

        {
            let mut date = match last_date {
                Some(date) => date + Duration::days(1),
                None => start_date,
            };
            debug_assert!(date > end_date || end_date == self.tomorrow);

            while date <= std::cmp::min(end_date, self.today) {
                self.in_memory_missing.lock().unwrap()
                    .entry(currency.to_owned())
                    .or_insert_with(HashSet::new)
                    .insert(date);
                date += Duration::days(1);
            }
        }

        diesel::replace_into(currency_rates::table)
            .values(rows)
            .execute(&*self.db)?;

        Ok(())
    }
}

#[derive(Debug)]
pub enum CurrencyRateCacheResult {
    Exists(Option<Decimal>),
    Missing(Date, Date),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_cache() {
        let currency = "USD";
        let (_database, mut cache) = CurrencyRateCache::new_temporary();

        let today = date!(8, 2, 2018);
        let tomorrow = today + Duration::days(1);
        cache.today = today;
        cache.tomorrow = tomorrow;

        let first_date = date!(10, 1, 2018);
        let last_date = date!(4, 2, 2018);
        let currency_rates = vec![CurrencyRate {
            date: last_date,
            price: dec!(1) / dec!(3),
        }, CurrencyRate {
            date: first_date,
            price: dec!(1) / dec!(7),
        }];

        let cache_start_date = last_date - Duration::days(365);
        let cache_end_date = today;

        assert_matches!(
            cache.get(currency, tomorrow),
            Err(ref e) if e.to_string() == "An attempt to get currency rate for the future"
        );

        assert_matches!(
            cache.get(currency, last_date).unwrap(),
            CurrencyRateCacheResult::Missing(from, to) if from == cache_start_date && to == tomorrow
        );
        cache.save(currency, cache_start_date, tomorrow, currency_rates.clone()).unwrap();

        for &clear_in_memory_cache in &[false, true] {
            let mut date = cache_start_date - Duration::days(1);
            if clear_in_memory_cache {
                cache.in_memory_missing.lock().unwrap().clear();
            }

            assert_matches!(
                cache.get(currency, date).unwrap(),
                CurrencyRateCacheResult::Missing(from, to)
                    if from == date - Duration::days(365) && to == first_date
            );

            'date_loop: loop {
                date += Duration::days(1);
                if date > cache_end_date {
                    break;
                }

                for currency_rate in &currency_rates {
                    if date == currency_rate.date {
                        assert_matches!(
                            cache.get(currency, date).unwrap(),
                            CurrencyRateCacheResult::Exists(Some(ref price)) if *price == currency_rate.price
                        );
                        continue 'date_loop;
                    }
                }

                let result = cache.get(currency, date).unwrap();

                if clear_in_memory_cache && last_date < date {
                    assert_matches!(result, CurrencyRateCacheResult::Missing(from, to)
                        if from == last_date + Duration::days(1) && to == tomorrow);
                } else {
                    assert_matches!(result, CurrencyRateCacheResult::Exists(None));
                }
            }

            assert_matches!(
                cache.get(currency, date),
                Err(ref e) if e.to_string() == "An attempt to get currency rate for the future"
            );
        }

        cache.today += Duration::days(10);
        cache.tomorrow += Duration::days(10);

        assert_matches!(
            cache.get(currency, tomorrow).unwrap(),
            CurrencyRateCacheResult::Missing(from, to)
                if from == last_date + Duration::days(1) && to == cache.tomorrow
        );
    }
}