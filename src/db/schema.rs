// TODO: https://github.com/diesel-rs/diesel/issues/1785
#![allow(proc_macro_derive_resolution_fallback)]

table! {
    currency_rates (currency, date) {
        currency -> Text,
        date -> Date,
        price -> Nullable<Text>,
    }
}

table! {
    quotes (symbol) {
        symbol -> Text,
        time -> Timestamp,
        currency -> Text,
        price -> Text,
    }
}