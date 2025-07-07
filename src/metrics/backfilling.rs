use std::collections::{BTreeMap, HashMap};

use async_stream::try_stream;
use chrono::{Duration, TimeDelta, TimeZone, Utc};
use futures_core::stream::Stream;
use log::info;
use reqwest::{self, Body, ClientBuilder};
use serde::{Serialize, Deserialize};
use serde::de::{Deserializer, Error};
use url::Url;
use validator::Validate;

use crate::core::{EmptyResult, GenericResult};
use crate::time::{self, Date};
use crate::util;

#[derive(Deserialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct BackfillingConfig {
    pub url: Url,
    #[serde(default)]
    pub labels: HashMap<String, String>,
    #[serde(deserialize_with = "deserialize_scrape_interval")]
    pub scrape_interval: Duration,
    #[serde(default = "default_min_performance_period", deserialize_with = "deserialize_min_performance_period")]
    pub min_performance_period: Duration,
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
pub async fn backfill(config: &BackfillingConfig, mut metrics: Vec<DailyTimeSeries>) -> EmptyResult {
    if !config.labels.is_empty() {
        for metric in &mut metrics {
            for (label, value) in &config.labels {
                if metric.labels.insert(label.to_owned(), value.to_owned()).is_some() {
                    return Err!("Invalid metrics backfilling configuration: backfilled metrics already have {label:?} label");
                }
            }
        }
    }

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

struct DayMetric<'a> {
    labels: &'a HashMap<String, String>,
    value: f64,
}

async fn get_import_stream(metrics: Vec<DailyTimeSeries>, scrape_interval: Duration) -> impl Stream<Item = GenericResult<Vec<u8>>> {
    try_stream! {
        let scrape_interval = scrape_interval.num_seconds();

        let mut sent = 0;
        let mut logged_percent = 0;
        let total: usize = metrics.iter().map(|time_series| time_series.values.len()).sum();

        // Without this ordering backfilling process leads to very high memory usage by VictoriaMetrics, because it runs
        // LSM rebuilding processes in the background.
        let mut ordered_metrics: BTreeMap<Date, Vec<DayMetric>> = BTreeMap::new();

        for time_series in &metrics {
            for &(date, value) in &time_series.values {
                ordered_metrics.entry(date).or_default().push(DayMetric {
                    labels: &time_series.labels,
                    value,
                })
            }
        }

        let mut timestamps = Vec::new();
        let mut values = Vec::new();

        for (date, metrics) in ordered_metrics {
            for DayMetric { labels, value } in metrics {
                timestamps.clear();
                values.clear();

                let mut cur_time = Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap()).timestamp();
                let end_time = Utc.from_utc_datetime(&date.succ_opt().unwrap().and_hms_opt(0, 0, 0).unwrap()).timestamp();

                while cur_time < end_time {
                    timestamps.push(cur_time * 1000);
                    values.push(value);
                    cur_time += scrape_interval;
                }

                let time_series = TimeSeries {
                    labels,
                    timestamps: &timestamps,
                    values: &values,
                };

                let mut buf = Vec::new();
                serde_json::to_writer(&mut buf, &time_series).map_err(|e| format!(
                    "Failed to serialize time series: {e}"))?;
                buf.push(b'\n');

                yield buf;

                sent += 1;
                let sent_percent = sent * 100 / total;

                if sent_percent != logged_percent {
                    info!("Sent {sent} of {total} ({sent_percent}%).");
                    logged_percent = sent_percent;
                }
            }
        }
    }
}

#[derive(Serialize)]
struct TimeSeries<'a> {
    #[serde(rename="metric")]
    labels: &'a HashMap<String, String>,
    #[serde(rename="timestamps")]
    timestamps: &'a Vec<i64>,
    #[serde(rename="values")]
    values: &'a Vec<f64>,
}

fn deserialize_scrape_interval<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where D: Deserializer<'de>
{
    let value: String = Deserialize::deserialize(deserializer)?;
    time::parse_duration(&value).ok().filter(|&scrape_interval| {
        scrape_interval >= Duration::seconds(1) && scrape_interval <= Duration::days(1)
    }).ok_or_else(|| D::Error::custom(format!("Invalid scrape interval: {value:?}")))
}

fn default_min_performance_period() -> Duration {
    Duration::days(1)
}

fn deserialize_min_performance_period<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where D: Deserializer<'de>
{
    let value: String = Deserialize::deserialize(deserializer)?;
    let duration = time::parse_duration(&value).ok().ok_or_else(|| D::Error::custom(format!(
        "Invalid minimum performance period: {value:?}")))?;

    let days = duration.num_days();
    if days < 1 || duration != TimeDelta::days(days) {
        return Err(D::Error::custom("Invalid minimum performance period: it must have day granularity"));
    }

    Ok(duration)
}