table! {
    currency_rates (currency, date) {
        currency -> Text,
        date -> Date,
        price -> Nullable<Text>,
    }
}
