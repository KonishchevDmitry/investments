use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::Mutex;

use chrono::Duration;
use diesel::{self, prelude::*};
#[cfg(test)] use tempfile::NamedTempFile;

use crate::core::{GenericResult, EmptyResult};
use crate::currency::Cash;
use crate::db::{self, schema::quotes, models};
use crate::time;
use crate::util::{self, DecimalRestrictions};

pub struct Cache {
    db: db::Connection,
    expire_time: Duration,
    cache: Option<Mutex<HashMap<String, Cash>>>,
}

impl Cache {
    pub fn new(connection: db::Connection, expire_time: Duration, in_memory_cache: bool) -> Cache {
        Cache {
            db: connection,
            expire_time: expire_time,
            cache: if in_memory_cache {
                Some(Mutex::new(HashMap::new()))
            } else {
                None
            },
        }
    }

    #[cfg(test)]
    pub fn new_temporary() -> (NamedTempFile, Cache) {
        let (database, connection) = db::new_temporary();
        (database, Cache::new(connection, Duration::minutes(1), false))
    }

    pub fn get(&self, symbol: &str) -> GenericResult<Option<Cash>> {
        if let Some(ref cache) = self.cache {
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
            "Got an invalid price from the database: {:?}", price))?;

        let price = Cash::new(&currency, price);
        if let Some(ref cache) = self.cache {
            cache.lock().unwrap().entry(symbol.to_owned()).or_insert(price);
        }

        Ok(Some(price))
    }

    pub fn save(&self, symbol: &str, price: Cash) -> EmptyResult {
        if let Some(ref cache) = self.cache {
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

        assert_eq!(cache.get(symbol).unwrap(), None);
        assert_eq!(cache.get(other_symbol).unwrap(), None);

        cache.save(symbol, price).unwrap();
        assert_eq!(cache.get(symbol).unwrap(), Some(price));
        assert_eq!(cache.get(other_symbol).unwrap(), None);

        cache.save(other_symbol, other_price).unwrap();
        assert_eq!(cache.get(symbol).unwrap(), Some(price));
        assert_eq!(cache.get(other_symbol).unwrap(), Some(other_price));

        cache.expire_time = Duration::seconds(0);
        assert_eq!(cache.get(symbol).unwrap(), None);
        assert_eq!(cache.get(other_symbol).unwrap(), None);
    }
}