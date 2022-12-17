use chrono::{DateTime, NaiveDateTime, TimeZone, Utc, Local};
use lazy_static::lazy_static;
use log::trace;
use regex::Regex;
use reqwest::blocking::{Client, Response};
use serde::de::DeserializeOwned;

use crate::core::GenericResult;
use crate::time::{SystemTime, TimeProvider};

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

pub fn is_outdated_quote<T: TimeZone>(date_time: DateTime<T>, now_provider: &dyn TimeProvider) -> Option<DateTime<Local>> {
    let lifetime = now_provider.now().naive_utc() - date_time.naive_utc();
    if lifetime.num_days() >= 5 {
        return Some(date_time.with_timezone(&Local))
    }
    None
}

pub fn is_outdated_unix_time(time: i64, test_outdated_time: i64) -> GenericResult<Option<DateTime<Local>>> {
    let test_outdated_time = NaiveDateTime::from_timestamp(test_outdated_time, 0);
    let naive_date_time = NaiveDateTime::from_timestamp_opt(time, 0).ok_or_else(|| format!(
        "Got an invalid UNIX time: {}", time))?;

    Ok(is_outdated_time::<Utc>(DateTime::from_utc(naive_date_time, Utc), test_outdated_time))
}

pub fn is_outdated_time<T: TimeZone>(date_time: DateTime<T>, test_outdated_time: NaiveDateTime) -> Option<DateTime<Local>> {
    if cfg!(test) {
        if date_time.naive_utc() <= test_outdated_time {
            Some(date_time.with_timezone(&Local))
        } else {
            None
        }
    } else {
        is_outdated_quote(date_time, &SystemTime())
    }
}

pub fn send_request<U: AsRef<str>>(client: &Client, url: U, authorization: Option<&str>) -> GenericResult<Response> {
    let url = url.as_ref();

    let mut request = client.get(url);
    if let Some(authorization) = authorization {
        request = request.bearer_auth(authorization);
    }

    trace!("Sending request to {}...", url);
    let response = request.send()?;
    trace!("Got response from {}.", url);

    if !response.status().is_success() {
        return Err!("Server returned an error: {}", response.status());
    }

    Ok(response)
}

pub fn parse_response<T: DeserializeOwned>(response: &str) -> GenericResult<T> {
    Ok(serde_json::from_str(response).map_err(|e| format!("Got an unexpected response: {}", e))?)
}