use std::collections::HashMap;

use serde::Serialize;

use crate::time::Date;

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

#[allow(dead_code)] // FIXME(konishchev): Drop it
#[derive(Clone, Serialize)]
#[serde(deny_unknown_fields)]
struct PreparedTimeSeries {
    #[serde(rename="metric")]
    labels: HashMap<String, String>,
    #[serde(rename="values")]
    values: Vec<f64>,
    #[serde(rename="timestamps")]
    timestamps: Vec<i64>,
}