use chrono::offset::LocalResult;
use prost_types::Timestamp;

use crate::time::{TimeZone, TzDateTime};

pub fn new_timestamp<Tz: TimeZone>(time: TzDateTime<Tz>) -> Timestamp {
    Timestamp {
        seconds: time.timestamp(),
        nanos: time.timestamp_subsec_nanos() as i32,
    }
}

pub fn parse_timestamp<Tz: TimeZone>(timestamp: Timestamp, tz: Tz) -> Option<TzDateTime<Tz>> {
    match tz.timestamp_opt(timestamp.seconds, timestamp.nanos as u32) {
        LocalResult::Single(time) => Some(time),
        _ => None,
    }
}