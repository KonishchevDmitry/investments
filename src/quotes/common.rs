use chrono::{DateTime, NaiveDateTime, TimeZone, Utc, Local};
use lazy_static::lazy_static;
use log::trace;
use regex::Regex;
use reqwest::Url;
use reqwest::blocking::{Client, Response};
use serde::de::DeserializeOwned;

use crate::core::GenericResult;
use crate::time;

pub fn parse_currency_pair(pair: &str) -> GenericResult<(&str, &str)> {
    lazy_static! {
        static ref REGEX: Regex = Regex::new(r"^(?P<base>[A-Z]{3})/(?P<quote>[A-Z]{3})$").unwrap();
    }

    let captures = REGEX.captures(pair).ok_or_else(|| format!(
        "Invalid currency pair: {:?}", pair))?;

    Ok((
        captures.name("base").unwrap().as_str(),
        captures.name("quote").unwrap().as_str(),
    ))
}

pub fn is_outdated_unix_time(time: i64, test_outdated_time: i64) -> GenericResult<Option<DateTime<Local>>> {
    let test_outdated_time = NaiveDateTime::from_timestamp(test_outdated_time, 0);
    let naive_date_time = NaiveDateTime::from_timestamp_opt(time, 0).ok_or_else(|| format!(
        "Got an invalid UNIX time: {}", time))?;

    Ok(is_outdated_time::<Utc>(DateTime::from_utc(naive_date_time, Utc), test_outdated_time))
}

pub fn is_outdated_time<T: TimeZone>(date_time: DateTime<T>, test_outdated_time: NaiveDateTime) -> Option<DateTime<Local>> {
    let naive_utc = date_time.naive_utc();

    let outdated = if cfg!(test) {
        naive_utc <= test_outdated_time
    } else {
        (time::utc_now() - naive_utc).num_days() >= 5
    };

    if outdated {
        Some(date_time.with_timezone(&Local))
    } else {
        None
    }
}

pub fn send_request(client: &Client, url: &Url) -> GenericResult<Response> {
    trace!("Sending request to {}...", url);
    let response = client.get(url.as_str()).send()?;
    trace!("Got response from {}.", url);

    if !response.status().is_success() {
        return Err!("Server returned an error: {}", response.status());
    }

    Ok(response)
}

pub fn parse_response<T: DeserializeOwned>(response: &str) -> GenericResult<T> {
    Ok(serde_json::from_str(response).map_err(|e| format!("Got an unexpected response: {}", e))?)
}