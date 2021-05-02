use std::collections::HashSet;
use std::thread::{self, JoinHandle};

use diesel::{self, prelude::*};
use log::{trace, debug, error};
#[cfg(test)] use mockito::{self, Mock, mock};
use reqwest::blocking::Client;
use serde::Serialize;
use serde_json::Value;

use crate::brokers::Broker;
use crate::core::{EmptyResult, GenericResult};
use crate::db::{self, schema::telemetry, models};

// FIXME(konishchev): Add more fields
#[derive(Serialize, Clone)]
pub struct TelemetryRecord {
    command: String,
    brokers: Vec<String>,
}

impl TelemetryRecord {
    // FIXME(konishchev): Rewrite
    #[cfg(test)]
    fn mock(id: usize) -> TelemetryRecord {
        TelemetryRecord {
            command: format!("{}", id),
            brokers: Vec::new(),
        }
    }
}

pub struct TelemetryRecordBuilder {
    brokers: HashSet<Broker>,
}

impl TelemetryRecordBuilder {
    pub fn new() -> TelemetryRecordBuilder {
        TelemetryRecordBuilder {
            brokers: HashSet::new(),
        }
    }

    pub fn new_with_broker(broker: Broker) -> TelemetryRecordBuilder {
        let mut record = TelemetryRecordBuilder::new();
        record.add_broker(broker);
        record
    }

    pub fn add_broker(&mut self, broker: Broker) {
        self.brokers.insert(broker);
    }

    pub fn build(self, command: &str) -> TelemetryRecord {
        let mut brokers: Vec<String> = self.brokers.iter()
            .map(|broker| broker.id().to_owned()).collect();
        brokers.sort();

        TelemetryRecord {
            command: command.to_owned(),
            brokers,
        }
    }
}

// FIXME(konishchev): Configuration option
pub struct Telemetry {
    db: db::Connection,
    processor: Option<JoinHandle<Option<i64>>>,
}

impl Telemetry {
    pub fn new(connection: db::Connection, max_records: usize) -> GenericResult<Telemetry> {
        let to_send = Telemetry::load(connection.clone(), max_records)?;
        Ok(Telemetry {
            db: connection,
            processor: to_send.map(|(last_record_id, payloads)| {
                thread::spawn(move || process(last_record_id, payloads))
            }),
        })
    }

    pub fn add(&self, record: TelemetryRecord) -> EmptyResult {
        let payload = serde_json::to_string(&record)?;

        diesel::insert_into(telemetry::table)
            .values(models::NewTelemetryRecord {payload})
            .execute(&*self.db)?;

        Ok(())
    }

    fn load(connection: db::Connection, max_records: usize) -> GenericResult<Option<(i64, Vec<Value>)>> {
        let records = telemetry::table
            .select((telemetry::id, telemetry::payload))
            .order_by(telemetry::id.asc())
            .load::<(i64, String)>(&*connection)?;

        let mut records: &[_] = &records;
        if records.len() > max_records {
            let count = records.len() - max_records;
            debug!("Dropping {} telemetry records.", count);

            diesel::delete(telemetry::table.filter(telemetry::id.le(records[count - 1].0)))
                .execute(&*connection)?;

            records = &records[count..];
        }

        let mut payloads = Vec::with_capacity(records.len());
        for record in records {
            let payload = serde_json::from_str(&record.1).map_err(|e| format!(
                "Failed to parse telemetry record: {}", e))?;
            payloads.push(payload);
        }

        Ok(records.last().map(|record| (record.0, payloads)))
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn close(mut self) -> EmptyResult {
        self.close_impl()
    }

    fn close_impl(&mut self) -> EmptyResult {
        // FIXME(konishchev): Timeout
        if let Some(processor) = self.processor.take() {
            processor.join().unwrap();
        }
        Ok(())
    }
}

impl Drop for Telemetry {
    fn drop(&mut self) {
        if let Err(e) = self.close_impl() {
            // FIXME(konishchev): HERE
            error!("Telemetry processing error: {}.", e)
        }
    }
}

#[derive(Serialize)]
pub struct TelemetryRecords {
    records: Vec<Value>,
}

fn process(last_record_id: i64, records: Vec<Value>) -> Option<i64> {
    #[cfg(not(test))] let base_url = "https://investments.konishchev.ru";
    #[cfg(test)] let base_url = mockito::server_url();
    let url = format!("{}/telemetry", base_url);

    trace!("Sending telemetry ({} records)...", records.len());
    match Client::new().post(url).json(&TelemetryRecords {records}).send() {
        Ok(response) => {
            if response.status().is_success() {
                trace!("Telemetry has been successfully sent.");
                Some(last_record_id)
            } else {
                trace!("Telemetry server returned an error: {}.", response.status());
                None
            }
        },
        Err(e) => {
            trace!("Failed to send telemetry: {}.", e);
            None
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry() {
        let (_database, connection) = db::new_temporary();

        let max_records = 5;
        let new_telemetry = || {
            Telemetry::new(connection.clone(), max_records).unwrap()
        };

        let mut expected = vec![];
        let mut server = broken_server();

        // Broken server, nothing to drop, nothing to send
        {
            let telemetry = new_telemetry();

            for id in 0..4 {
                let record = TelemetryRecord::mock(id);
                telemetry.add(record.clone()).unwrap();
                expected.push(record);
            }

            telemetry.close().unwrap();
        }
        compare(connection.clone(), &expected);
        server = server.expect(0);
        server.assert();

        // Broken server, nothing to drop, trying to send
        {
            let telemetry = new_telemetry();

            for id in 4..8 {
                let record = TelemetryRecord::mock(id);
                telemetry.add(record.clone()).unwrap();
                expected.push(record);
            }

            telemetry.close().unwrap();
        }
        compare(connection.clone(), &expected);
        server = server.expect(1);
        server.assert();

        // Broken server, dropping records, trying to send
        {
            let telemetry = new_telemetry();
            expected.drain(..3);

            for id in 8..12 {
                let record = TelemetryRecord::mock(id);
                telemetry.add(record.clone()).unwrap();
                expected.push(record);
            }

            telemetry.close().unwrap();
        }
        compare(connection.clone(), &expected);
        server = server.expect(2);
        server.assert();

        // Healthy server, dropping records, sending remaining
        expected.drain(..4);
        server = healthy_server(&expected);
        {
            let telemetry = new_telemetry();

            for id in 12..16 {
                let record = TelemetryRecord::mock(id);
                telemetry.add(record.clone()).unwrap();
                expected.push(record);
            }

            telemetry.close().unwrap();
        }
        compare(connection.clone(), &expected);
        server = server.expect(1);
        server.assert();
    }

    fn broken_server() -> Mock {
        mock("POST", "/telemetry")
            .with_status(500)
            .create()
    }

    fn healthy_server(expected: &[TelemetryRecord]) -> Mock {
        let expected_body = serde_json::to_string(&TelemetryRecords {
            records: expected.iter().map(|record| {
                serde_json::to_value(record).unwrap()
            }).collect(),
        }).unwrap();

        mock("POST", "/telemetry")
            .match_header("content-type", "application/json")
            .match_body(expected_body.as_str())
            .with_status(200)
            .create()
    }

    fn compare(connection: db::Connection, expected: &[TelemetryRecord]) {
        let actual = telemetry::table
            .select(telemetry::payload)
            .order_by(telemetry::id.asc())
            .load::<String>(&*connection).unwrap();

        let expected: Vec<String> = expected.iter()
            .map(|record| serde_json::to_string(record).unwrap())
            .collect();

        assert_eq!(actual, expected);
    }
}