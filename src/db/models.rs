use types::{Date, Decimal};

use super::schema::currency_rates;

#[derive(Queryable)]
pub struct CurrencyRate {
    pub currency: String,
    pub date: Date,
    pub price: String,
}

#[derive(Insertable)]
#[table_name="currency_rates"]
pub struct NewCurrencyRate<'a> {
    pub currency: &'a str,
    pub date: Date,
    pub price: String,
}