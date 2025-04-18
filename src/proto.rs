use chrono::Utc;
use chrono::offset::LocalResult;
use prost_types::Timestamp;

use crate::time::{TimeZone, TzDateTime};

pub fn new_timestamp<T: TimeZone>(time: TzDateTime<T>) -> Timestamp {
    Timestamp {
        seconds: time.timestamp(),
        nanos: time.timestamp_subsec_nanos() as i32,
    }
}

pub fn parse_timestamp(timestamp: Timestamp) -> Option<TzDateTime<Utc>> {
    match Utc.timestamp_opt(timestamp.seconds, timestamp.nanos as u32) {
        LocalResult::Single(time) => Some(time),
        _ => None,
    }
}