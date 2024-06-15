use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{DateTime, NaiveDateTime, TimeZone, Utc, Local};
use log::trace;
use rayon::prelude::*;
use reqwest::blocking::{Client, Response};
use serde::de::DeserializeOwned;

use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::quotes::QuotesMap;
use crate::time::{SystemTime, TimeProvider};

pub fn parallelize_quotes<F>(symbols: &[&str], get_quote: F) -> GenericResult<QuotesMap>
    where F: Fn(&str) -> GenericResult<Option<Cash>> + Sync + Send
{
    let quotes = Mutex::new(HashMap::new());

    if let Some(error) = symbols.par_iter().map(|&symbol| -> EmptyResult {
        if let Some(price) = get_quote(symbol)? {
            let mut quotes = quotes.lock().unwrap();
            quotes.insert(symbol.to_owned(), price);
        }
        Ok(())
    }).find_map_any(|result| match result {
        Err(error) => Some(error),
        Ok(()) => None,
    }) {
        return Err(error);
    }

    Ok(quotes.into_inner().unwrap())
}

pub fn is_outdated_quote<T: TimeZone>(date_time: DateTime<T>, now_provider: &dyn TimeProvider) -> Option<DateTime<Local>> {
    let lifetime = now_provider.now().naive_utc() - date_time.naive_utc();
    if lifetime.num_days() >= 5 {
        return Some(date_time.with_timezone(&Local))
    }
    None
}

pub fn is_outdated_unix_time(time: i64, test_outdated_time: i64) -> GenericResult<Option<DateTime<Local>>> {
    let test_outdated_time = DateTime::from_timestamp(test_outdated_time, 0).ok_or_else(|| format!(
        "Got an invalid UNIX time: {}", test_outdated_time))?;

    let date_time = DateTime::from_timestamp(time, 0).ok_or_else(|| format!(
        "Got an invalid UNIX time: {}", time))?;

    Ok(is_outdated_time::<Utc>(date_time, test_outdated_time.naive_utc()))
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