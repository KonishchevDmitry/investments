use std::str::FromStr;

use chrono::{self, Datelike, Duration};
use diesel::prelude::*;
use tempfile::NamedTempFile;

use core::{GenericResult, GenericError};
use db::{self, schema::currency_rates, models};
use types::{Date, Decimal};

pub struct CurrencyRateCache {
    today: Date,
    db: db::Connection,
}

impl CurrencyRateCache {
    pub fn new(connection: db::Connection) -> CurrencyRateCache {
        let today = chrono::Local::today();

        CurrencyRateCache {
            today: Date::from_ymd(today.year(), today.month(), today.day()),
            db: connection,
        }
    }

    pub fn get(&self, currency: &str, date: Date) -> GenericResult<CurrencyRateCacheResult> {
        if date >= self.today {
            return Err!("An attempt to get price for the future")
        }

        self.db.transaction::<_, GenericError, _>(|| {
            let result = currency_rates::table
                .select(currency_rates::price)
                .filter(currency_rates::currency.eq(currency))
                .filter(currency_rates::date.eq(&date))
                .get_result::<Option<String>>(&self.db).optional()?;

            if let Some(cached_price) = result {
                return Ok(CurrencyRateCacheResult::Exists(match cached_price {
                    Some(price) => Some(Decimal::from_str(&price).map_err(|_| format!(
                        "Got an invalid price from the database: {:?}", price))?),
                    None => None,
                }));
            }

            let year_start = Date::from_ymd(date.year(), 1, 1);
            let year_end = Date::from_ymd(date.year(), 12, 31);

            let last_date = currency_rates::table
                .select(currency_rates::date)
                .filter(currency_rates::currency.eq(currency))
                .filter(currency_rates::date.ge(year_start))
                .filter(currency_rates::date.le(year_end))
                .order(currency_rates::date.desc())
                .limit(1)
                .get_result::<Date>(&self.db).optional()?;

            let start_date = match last_date {
                Some(last_date) => last_date + Duration::days(1),
                None => year_start,
            };

            let end_date = if year_end >= self.today {
                self.today - Duration::days(1)
            } else {
                year_end
            };

            assert!(start_date <= end_date);
            assert!(end_date < self.today);

            Ok(CurrencyRateCacheResult::Missing(start_date, end_date))
        })
    }

    pub fn save() {

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
        let database = NamedTempFile::new().unwrap();
        let connection = db::connect(database.path().to_str().unwrap()).unwrap();

        let mut cache = CurrencyRateCache::new(connection);
        cache.today = Date::from_ymd(2018, 2, 9);

        let mut date = cache.today.with_day(4).unwrap();

        assert_matches!(
            cache.get(currency, date).unwrap(),
            CurrencyRateCacheResult::Missing(from, to) if (
                from == Date::from_ymd(2018, 1, 1) && to == Date::from_ymd(2018, 2, 8))
        );

        // FIXME: prev year
        /*
        use std::str::FromStr;
        use diesel;
        use types::{Date, Decimal};
        use diesel::prelude::*;
        use db::models::*;
        use db::schema::currency_rates::dsl::*;


        use db::schema::currency_rates;
        let new_post = NewCurrencyRate {
            currency: "USD",
            date: Date::from_ymd(1, 1, 1),
            price: Decimal::from_str("1").unwrap().to_string(),
        };

        diesel::replace_into(currency_rates::table).values(&new_post).execute(&connection).unwrap();

        schema::users::dsl::*;
        insert_into(users) -> insert_into(users::table)

        QueryResult<Option<T>>).get_result(...).optional().

        let target = posts.filter(publish_at.lt(now));
        diesel::update(target).set(draft.eq(false))

        diesel::insert_into(currency_rates::table)
            .values(&new_post)
            .execute(&connection)
//        .get_result(conn)
            .expect("Error saving new post");

        diesel::update(posts.find(new_post.id))
            .set(published.eq(true))
            .execute(&connection)
//        .get_result::<Post>(&connection)
            .expect(&format!("Unable to find post {}", new_post.id));

        let results = currency_rates.filter(currency.eq("USD"))
            .limit(5)
            .load::<CurrencyRate>(&connection)
            .expect("Error loading posts");

        println!("Displaying {} posts", results.len());
        for post in results {
            println!("{} - {}", post.date, post.price);
        }
        */
    }
}