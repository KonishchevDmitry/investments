// TODO: https://github.com/diesel-rs/diesel/issues/1785
#![allow(proc_macro_derive_resolution_fallback)]

use db::schema::currency_rates;
use types::Date;

#[derive(Insertable)]
#[table_name="currency_rates"]
pub struct NewCurrencyRate<'a> {
    pub currency: &'a str,
    pub date: Date,
    pub price: Option<String>,
}