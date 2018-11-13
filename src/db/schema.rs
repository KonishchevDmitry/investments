// TODO: https://github.com/diesel-rs/diesel/issues/1785
#![allow(proc_macro_derive_resolution_fallback)]

#[derive(DbEnum, Debug)]
pub enum AssetType {
    Stock,
    Cash,
}

table! {
    use diesel::sql_types::Text;
    use super::AssetType;

    my_table (portfolio, asset_type, symbol) {
        portfolio -> Text,
        asset_type -> AssetType,
        symbol -> Text,
        quantity -> Text,
    }
}

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