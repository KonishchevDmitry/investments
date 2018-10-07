// FIXME: https://github.com/diesel-rs/diesel/issues/1785
#![allow(proc_macro_derive_resolution_fallback)]

use db::schema::currency_rates;
use types::Date;

// FIXME
//#[derive(Queryable)]
//pub struct CurrencyRate {
//    pub currency: String,
//    pub date: Date,
//    pub price: String,
//}

#[derive(Insertable)]
#[table_name="currency_rates"]
pub struct NewCurrencyRate<'a> {
    pub currency: &'a str,
    pub date: Date,
    pub price: String,
}