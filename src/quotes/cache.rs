use chrono::{self, Duration};
use diesel::{self, prelude::*};
#[cfg(test)] use tempfile::NamedTempFile;

use core::{GenericResult, EmptyResult};
use currency::Cash;
use db::{self, schema::quotes, models};
use util::{self, DecimalRestrictions};

pub struct Cache {
    db: db::Connection,
    expire_time: Duration,
}

impl Cache {
    pub fn new(connection: db::Connection) -> Cache {
        Cache {
            db: connection,
            expire_time: Duration::minutes(1),
        }
    }

    #[cfg(test)]
    pub fn new_temporary() -> (NamedTempFile, Cache) {
        let (database, connection) = db::new_temporary();
        (database, Cache::new(connection))
    }

    pub fn get(&self, symbol: &str) -> GenericResult<Option<Cash>> {
        let expire_time = chrono::Local::now().naive_local() - self.expire_time;

        let result = quotes::table
            .select((quotes::currency, quotes::price))
            .filter(quotes::symbol.eq(symbol))
            .filter(quotes::time.gt(&expire_time))
            .get_result::<(String, String)>(&*self.db).optional()?;

        let (currency, price) = match result {
            Some(result) => result,
            None => return Ok(None),
        };

        let price = util::parse_decimal(&price, DecimalRestrictions::StrictlyPositive).map_err(|_| format!(
            "Got an invalid price from the database: {:?}", price))?;

        Ok(Some(Cash::new(&currency, price)))
    }

    pub fn save(&self, symbol: &str, price: Cash) -> EmptyResult {
        let time = chrono::Local::now().naive_local();

        diesel::replace_into(quotes::table)
            .values(models::NewQuote {
                symbol: symbol,
                time: time,
                currency: price.currency,
                price: price.amount.to_string(),
            })
            .execute(&*self.db)?;

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
        let price = Cash::new("USD", decs!("1.234"));

        let other_symbol = "FXRU";
        let other_price = Cash::new("RUB", decs!("1234.56"));

        diesel::replace_into(quotes::table)
            .values(models::NewQuote {
                symbol: symbol,
                time: chrono::Local::now().naive_local() - cache.expire_time,
                currency: "EUR",
                price: s!("12.34"),
            })
            .execute(&*cache.db).unwrap();

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