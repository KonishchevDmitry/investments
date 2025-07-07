use std::collections::{BTreeMap, HashMap};
use std::ops::DerefMut;
use std::sync::Mutex;

use chrono::Duration;
use diesel::{self, prelude::*};
use log::{debug, trace};
#[cfg(test)] use tempfile::NamedTempFile;

use crate::core::{GenericResult, EmptyResult};
use crate::currency::Cash;
use crate::db::{self, schema::quotes, models};
use crate::exchanges::Exchange;
use crate::time::{self, Date, Period};
use crate::util::{self, DecimalRestrictions};

pub type HistoricalQuotes = BTreeMap<Date, Cash>;
pub type HistoricalQuotesKey = (Exchange, String);

pub struct Cache {
    db: db::Connection,
    expire_time: Duration,
    real_time: Option<Mutex<HashMap<String, Cash>>>,
    historical: Mutex<HashMap<HistoricalQuotesKey, BTreeMap<Period, HistoricalQuotes>>>,
}

impl Cache {
    pub fn new(connection: db::Connection, expire_time: Duration, in_memory_cache: bool) -> Cache {
        Cache {
            db: connection,
            expire_time: expire_time,
            real_time: if in_memory_cache {
                Some(Default::default())
            } else {
                None
            },
            historical: Default::default(),
        }
    }

    #[cfg(test)]
    pub fn new_temporary() -> (NamedTempFile, Cache) {
        let (database, connection) = db::new_temporary();
        (database, Cache::new(connection, Duration::minutes(1), false))
    }

    pub fn get_real_time(&self, symbol: &str) -> GenericResult<Option<Cash>> {
        if let Some(ref cache) = self.real_time {
            if let Some(price) = cache.lock().unwrap().get(symbol).copied() {
                return Ok(Some(price));
            }
        }

        let expire_time = time::now() - self.expire_time;
        let result = quotes::table
            .select((quotes::currency, quotes::price))
            .filter(quotes::symbol.eq(symbol))
            .filter(quotes::time.gt(&expire_time))
            .get_result::<(String, String)>(self.db.borrow().deref_mut()).optional()?;

        let (currency, price) = match result {
            Some(result) => result,
            None => return Ok(None),
        };

        let price = util::parse_decimal(&price, DecimalRestrictions::StrictlyPositive).map_err(|_| format!(
            "Got an invalid price from the database: {price:?}"))?;

        let price = Cash::new(&currency, price);
        if let Some(ref cache) = self.real_time {
            cache.lock().unwrap().entry(symbol.to_owned()).or_insert(price);
        }

        Ok(Some(price))
    }

    pub fn save_real_time(&self, symbol: &str, price: Cash) -> EmptyResult {
        if let Some(ref cache) = self.real_time {
            cache.lock().unwrap().insert(symbol.to_owned(), price);
        }

        diesel::replace_into(quotes::table)
            .values(models::NewQuote {
                symbol: symbol,
                time: time::now(),
                currency: price.currency,
                price: price.amount.to_string(),
            })
            .execute(self.db.borrow().deref_mut())?;

        Ok(())
    }

    pub fn get_historical(&self, exchange: Exchange, symbol: &str, period: Period) -> GenericResult<Option<HistoricalQuotes>> {
        let cache = self.historical.lock().unwrap();

        let Some(instrument_cache) = cache.get(&(exchange, symbol.to_owned())) else {
            return Ok(None);
        };

        for (cached_period, quotes) in instrument_cache {
            if cached_period.contains_period(period) {
                return Ok(Some(quotes.iter().filter_map(|(&date, &price)| {
                    period.contains(date).then_some((date, price))
                }).collect()));
            }
        }

        Ok(None)
    }

    // Historical data much harder to cache in the database and it must be invalidated after stock splits, so for now
    // cache it only in memory.
    pub fn save_historical(&self, exchange: Exchange, symbol: &str, mut period: Period, mut quotes: HistoricalQuotes) -> EmptyResult {
        // API may have very big granularity and return more data than we've requested, so try take advantage of it here
        if !quotes.is_empty() {
            period = Period::new(
                std::cmp::min(period.first_date(), *quotes.first_key_value().unwrap().0),
                std::cmp::max(period.last_date(), *quotes.last_key_value().unwrap().0),
            )?;
        }

        let mut cache = self.historical.lock().unwrap();
        let instrument_cache = cache.entry((exchange, symbol.to_owned())).or_default();

        'merge: loop {
            for &other_period in instrument_cache.keys() {
                if let Some(new_period) = period.try_union(other_period) {
                    debug!("Join historical quotes of {symbol} ({exchange}): {{{period}}} + {{{other_period}}} = {{{new_period}}}.");
                    quotes.extend(instrument_cache.remove(&other_period).unwrap());
                    period = new_period;
                    continue 'merge;
                }
            }

            trace!(
                "Caching historical quotes for {symbol} ({exchange}): {period}, {} days, {} quotes.",
                period.days(), quotes.len());

            assert!(instrument_cache.insert(period, quotes).is_none());
            break;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache() {
        let (_database, mut cache) = Cache::new_temporary();

        let symbol = "BND";
        let price = Cash::new("USD", dec!(1.234));

        let other_symbol = "FXRU";
        let other_price = Cash::new("RUB", dec!(1234.56));

        diesel::replace_into(quotes::table)
            .values(models::NewQuote {
                symbol: symbol,
                time: time::now() - cache.expire_time,
                currency: "EUR",
                price: s!("12.34"),
            })
            .execute(cache.db.borrow().deref_mut()).unwrap();

        assert_eq!(cache.get_real_time(symbol).unwrap(), None);
        assert_eq!(cache.get_real_time(other_symbol).unwrap(), None);

        cache.save_real_time(symbol, price).unwrap();
        assert_eq!(cache.get_real_time(symbol).unwrap(), Some(price));
        assert_eq!(cache.get_real_time(other_symbol).unwrap(), None);

        cache.save_real_time(other_symbol, other_price).unwrap();
        assert_eq!(cache.get_real_time(symbol).unwrap(), Some(price));
        assert_eq!(cache.get_real_time(other_symbol).unwrap(), Some(other_price));

        cache.expire_time = Duration::seconds(0);
        assert_eq!(cache.get_real_time(symbol).unwrap(), None);
        assert_eq!(cache.get_real_time(other_symbol).unwrap(), None);
    }
}