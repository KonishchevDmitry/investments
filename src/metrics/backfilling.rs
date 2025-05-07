use std::collections::HashMap;

use async_stream::try_stream;
use chrono::{Duration, TimeZone, Utc};
use futures_core::stream::Stream;
use log::info;
use reqwest::{self, Body, ClientBuilder};
use serde::Serialize;
use url::Url;

use crate::core::{EmptyResult, GenericResult};
use crate::time::Date;
use crate::util;

pub struct BackfillingConfig {
    pub url: Url,
    pub scrape_interval: Duration,
}

pub struct DailyTimeSeries {
    labels: HashMap<String, String>,
    values: Vec<(Date, f64)>,
}

impl DailyTimeSeries {
    pub fn new(name: &str) -> DailyTimeSeries {
        DailyTimeSeries {
            labels: hashmap! {
                s!("__name__") => name.to_owned(),
            },
            values: Vec::new(),
        }
    }

    pub fn with_label(mut self, name: &str, value: &str) -> Self {
        self.labels.insert(name.to_owned(), value.to_owned());
        self
    }

    pub fn add_value(&mut self, date: Date, value: f64) {
        self.values.push((date, value));
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

#[tokio::main(flavor = "current_thread")]
pub async fn backfill(config: &BackfillingConfig, metrics: Vec<DailyTimeSeries>) -> EmptyResult {
    let client = ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .no_brotli()
        .no_deflate()
        .no_gzip()
        .no_zstd()
        .build()?;

    let import_url = config.url.join("/api/v1/import").map_err(|e| format!(
        "Invalid URL: {e}"))?;

    let import_stream = get_import_stream(metrics, config.scrape_interval).await;

    info!("Backfilling the metrics to VictoriaMetrics...");
    let response = client.post(import_url).body(Body::wrap_stream(import_stream)).send().await.map_err(|err| {
        if err.is_connect() {
            format!("Failed to establish connection to VictoriaMetrics: {}", util::humanize_reqwest_error(err))
        } else if err.is_body() {
            util::humanize_reqwest_error(err)
        } else {
            format!("VictoriaMetrics connection error: {}", util::humanize_reqwest_error(err))
        }
    })?;

    let status = response.status();
    if !status.is_success() {
        let message = response.text().await.unwrap_or_else(|e| e.to_string());
        return Err!("VictoriaMetrics returned an error ({status}): {}", message.trim());
    }

    Ok(())
}

async fn get_import_stream(metrics: Vec<DailyTimeSeries>, scrape_interval: Duration) -> impl Stream<Item = GenericResult<Vec<u8>>> {
    try_stream! {
        if scrape_interval < Duration::seconds(1) || scrape_interval > Duration::days(1) {
            return Err("Invalid scrape interval")?;
        }

        let scrape_interval = scrape_interval.num_seconds();

        let mut sent = 0;
        let mut logged_percent = 0;
        let total: usize = metrics.iter().map(|time_series| time_series.values.len()).sum();

        for time_series in metrics {
            for (date, value) in time_series.values {
                let mut timestamps = Vec::new();
                let mut values = Vec::new();

                let mut cur_time = Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap()).timestamp();
                let end_time = Utc.from_utc_datetime(&date.succ_opt().unwrap().and_hms_opt(0, 0, 0).unwrap()).timestamp();

                while cur_time < end_time {
                    timestamps.push(cur_time * 1000);
                    values.push(value);
                    cur_time += scrape_interval;
                }

                let time_series = TimeSeries {
                    labels: time_series.labels.clone(),
                    timestamps, values,
                };

                let mut buf = Vec::new();
                serde_json::to_writer(&mut buf, &time_series).map_err(|e| format!(
                    "Failed to serialize time series: {e}"))?;
                buf.push(b'\n');

                yield buf;

                sent += 1;
                let sent_percent = sent * 100 / total;

                if sent_percent != logged_percent {
                    info!("Sent {sent} of {total} ({}%).", sent_percent);
                    logged_percent = sent_percent;
                }
            }
        }
    }
}

#[derive(Serialize)]
struct TimeSeries {
    #[serde(rename="metric")]
    labels: HashMap<String, String>,
    #[serde(rename="timestamps")]
    timestamps: Vec<i64>,
    #[serde(rename="values")]
    values: Vec<f64>,
}