use tempfile::NamedTempFile;

use db::{self, models};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_cache() {
        let database = NamedTempFile::new().unwrap();
        let connection = db::connect(database.path().to_str().unwrap()).unwrap();

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