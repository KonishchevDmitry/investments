use serde::{Serialize, Serializer};

use crate::time::Date;

pub use rust_decimal::serde::float::serialize as serialize_decimal;

pub fn serialize_date<S: Serializer>(date: &Date, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&date.format("%Y-%m-%dT00:00:00").to_string())
}

pub fn serialize_with_default<S, T>(value: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
where S: Serializer, T: Serialize + Default,
{
    match value {
        Some(value) => value.serialize(serializer),
        None => T::default().serialize(serializer),
    }
}