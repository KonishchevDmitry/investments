/// Implements telemetry sending functionality.
///
/// Sends only basic anonymous usage statistics like program version, used commands and brokers.
/// No personal information will ever be sent.

use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex, Condvar};
use std::thread::{self, JoinHandle};
use std::time::{Instant, SystemTime, Duration};

use diesel::{self, prelude::*};
use log::{trace, error};
use platforms::target::OS;
use reqwest::blocking::Client;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use uuid::Uuid;

use crate::brokers::Broker;
use crate::core::{EmptyResult, GenericResult, GenericError};
use crate::db::{self, schema::{settings, telemetry}, models};
use crate::util;

#[derive(Serialize, Clone)]
pub struct TelemetryRecord {
    id: String,
    time: u64,

    os: &'static str,
    version: &'static str,
    #[serde(skip_serializing_if = "util::is_default")]
    precompiled: bool,
    #[serde(skip_serializing_if = "util::is_default")]
    container: bool,

    command: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    brokers: Vec<String>,
}

impl TelemetryRecord {
    #[cfg(test)]
    fn mock(id: usize) -> TelemetryRecord {
        let mut record = TelemetryRecordBuilder::new().build("command-mock");
        record.id = format!("{}", id);
        record
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

        let id = Uuid::new_v4().to_string();
        let os = std::env::consts::OS;
        let time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default()
            .as_secs();

        TelemetryRecord {
            id, time,

            os,
            version: env!("CARGO_PKG_VERSION"),
            precompiled: option_env!("INVESTMENTS_PRECOMPILED_BINARY").is_some(),
            container: os == OS::Linux.as_str() && std::process::id() == 1,

            command: command.to_owned(),
            brokers,
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct TelemetryConfig {
    #[serde(default)]
    pub disable: bool,
}

#[derive(Serialize)]
struct TelemetryRequest {
    user_id: String,
    records: Vec<Value>,
}

pub struct Telemetry {
    db: db::Connection,
    sender: Option<TelemetrySender>,
}

impl Telemetry {
    pub fn new(
        connection: db::Connection, flush_thresholds: BTreeMap<usize, Duration>, max_records: usize,
    ) -> GenericResult<Telemetry> {
        let mut telemetry = Telemetry {
            db: connection,
            sender: None,
        };

        if let Some((records, last_record_id)) = telemetry.load(max_records)? {
            let user_id = telemetry.user_id()?;
            let request = TelemetryRequest {user_id, records};

            // By default we don't give any extra time to sender to complete its work. But if we
            // accumulated some records - we do.
            let mut deadline = Instant::now();
            for (&threshold, &timeout) in flush_thresholds.iter().rev() {
                if request.records.len() % threshold == 0 {
                    deadline += timeout;
                    break;
                }
            }

            telemetry.sender.replace(TelemetrySender::new(request, last_record_id, deadline));
        }

        Ok(telemetry)
    }

    pub fn add(&self, record: TelemetryRecord) -> EmptyResult {
        let payload = serde_json::to_string(&record)?;

        diesel::insert_into(telemetry::table)
            .values(models::NewTelemetryRecord {payload})
            .execute(&*self.db)?;

        Ok(())
    }

    fn load(&self, max_records: usize) -> GenericResult<Option<(Vec<Value>, i64)>> {
        let records = telemetry::table
            .select((telemetry::id, telemetry::payload))
            .order_by(telemetry::id.asc())
            .load::<(i64, String)>(&*self.db)?;

        let mut records: &[_] = &records;
        if records.len() > max_records {
            let count = records.len() - max_records;
            trace!("Dropping {} telemetry records.", count);
            self.delete(records[count - 1].0)?;
            records = &records[count..];
        }

        let mut payloads = Vec::with_capacity(records.len());
        for record in records {
            let payload = serde_json::from_str(&record.1).map_err(|e| format!(
                "Failed to parse telemetry record: {}", e))?;
            payloads.push(payload);
        }

        Ok(records.last().map(|record| (payloads, record.0)))
    }

    fn delete(&self, last_record_id: i64) -> EmptyResult {
        diesel::delete(telemetry::table.filter(telemetry::id.le(last_record_id)))
            .execute(&*self.db)?;
        Ok(())
    }

    fn user_id(&self) -> GenericResult<String> {
        self.db.transaction::<_, GenericError, _>(|| {
            let name = models::SETTING_USER_ID;
            let user_id = settings::table
                .select(settings::value)
                .filter(settings::name.eq(name))
                .get_result::<String>(&*self.db).optional()?;

            Ok(match user_id {
                Some(user_id) => user_id,
                None => {
                    let user_id = Uuid::new_v4().to_string();

                    diesel::insert_into(settings::table)
                        .values(&models::NewSetting {name, value: &user_id})
                        .execute(&*self.db)?;

                    user_id
                },
            })
        })
    }

    #[cfg(test)]
    fn close(mut self) -> EmptyResult {
        self.close_impl()
    }

    fn close_impl(&mut self) -> EmptyResult {
        if let Some(sender) = self.sender.take() {
            if let Some(last_record_id) = sender.wait() {
                self.delete(last_record_id).map_err(|e| format!(
                    "Failed to delete telemetry records: {}", e))?;
            }
        }
        Ok(())
    }
}

impl Drop for Telemetry {
    fn drop(&mut self) {
        if let Err(err) = self.close_impl() {
            error!("{}.", err)
        }
    }
}

struct TelemetrySender {
    thread: JoinHandle<()>,
    result: Arc<(Mutex<Option<Option<i64>>>, Condvar)>,
    deadline: Instant,
}

impl TelemetrySender {
    fn new(request: TelemetryRequest, last_record_id: i64, deadline: Instant) -> TelemetrySender {
        let result = Arc::new((Mutex::new(None), Condvar::new()));

        let thread = {
            let result = result.clone();
            thread::spawn(move || {
                let ok = TelemetrySender::send(request);

                let (lock, cond) = result.as_ref();
                let mut result = lock.lock().unwrap();

                result.replace(if ok {
                    Some(last_record_id)
                } else {
                    None
                });
                cond.notify_one();
            })
        };

        TelemetrySender {thread, result, deadline}
    }

    fn wait(self) -> Option<i64> {
        let result = {
            let (lock, cond) = self.result.as_ref();

            let guard = lock.lock().unwrap();
            let timeout = self.deadline.checked_duration_since(Instant::now()).unwrap_or_default();

            let (mut result, _) = cond.wait_timeout_while(
                guard, timeout, |result| result.is_none(),
            ).unwrap();

            result.take().unwrap_or_default()
        };

        if cfg!(test) {
            // Join the thread in test mode to not introduce any side effects, but do it after
            // result acquiring to not change the behaviour.
            self.thread.join().unwrap();
        } else {
            // We mustn't delay program execution or shutdown because of telemetry server or network
            // unavailability, so just forget about the thread - it will die on program exit.
        }

        result
    }

    fn send(request: TelemetryRequest) -> bool {
        #[cfg(not(test))] let base_url = "https://investments.konishchev.ru";
        #[cfg(test)] let base_url = mockito::server_url();
        let url = format!("{}/telemetry", base_url);

        trace!("Sending telemetry ({} records)...", request.records.len());
        match Client::new().post(url).json(&request).send() {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    // Consume body in test mode to block on unreachable server emulation
                    if cfg!(test) {
                        let _ = response.bytes();
                    }
                    trace!("Telemetry has been successfully sent.");
                    true
                } else {
                    trace!("Telemetry server returned an error: {}.", status);
                    false
                }
            },
            Err(e) => {
                trace!("Failed to send telemetry: {}.", e);
                false
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{self, Mock, mock};

    #[test]
    fn telemetry() {
        let (_database, connection) = db::new_temporary();
        let new_telemetry = || {
            Telemetry::new(connection.clone(), btreemap!{
                1 => Duration::from_millis(10),
            }, 5).unwrap()
        };

        let mut expected = vec![];
        let mut server = broken_server().expect(0);

        // Broken server, nothing to drop, nothing to send
        let user_id = {
            let telemetry = new_telemetry();
            let user_id = telemetry.user_id().unwrap();

            for id in 0..4 {
                let record = TelemetryRecord::mock(id);
                telemetry.add(record.clone()).unwrap();
                expected.push(record);
            }

            telemetry.close().unwrap();
            user_id
        };
        server.assert();
        compare(connection.clone(), &expected); // 4 records

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
        server = server.expect(1);
        server.assert();
        compare(connection.clone(), &expected); // 8 records

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
        server = server.expect(2);
        server.assert();
        compare(connection.clone(), &expected); // 9 records

        // Healthy server, dropping records, sending remaining
        expected.drain(..4);
        server = healthy_server(&user_id, &expected); // 5 records
        {
            let telemetry = new_telemetry();

            for id in 12..16 {
                let record = TelemetryRecord::mock(id);
                telemetry.add(record.clone()).unwrap();
                expected.push(record);
            }

            telemetry.close().unwrap();
        }
        server.assert();
        expected.drain(..5);
        compare(connection.clone(), &expected); // 4 records

        // Unreachable server, nothing to drop, trying to send
        server = unreachable_server();
        {
            let telemetry = new_telemetry();

            let record = TelemetryRecord::mock(16);
            telemetry.add(record.clone()).unwrap();
            expected.push(record);

            telemetry.close().unwrap();
        }
        server.assert();
        compare(connection.clone(), &expected); // 5 records

        // Healthy server, nothing to drop, sending all records
        server = healthy_server(&user_id, &expected);
        {
            let telemetry = new_telemetry();

            let record = TelemetryRecord::mock(17);
            telemetry.add(record.clone()).unwrap();
            expected.push(record);

            telemetry.close().unwrap();
        }
        server.assert();
        expected.drain(..5);
        compare(connection.clone(), &expected); // 1 record
    }

    fn broken_server() -> Mock {
        mock("POST", "/telemetry")
            .with_status(500)
            .create()
    }

    fn healthy_server(user_id: &str, expected: &[TelemetryRecord]) -> Mock {
        let expected_request = TelemetryRequest {
            user_id: user_id.to_owned(),
            records: expected.iter().map(|record| {
                serde_json::to_value(record).unwrap()
            }).collect(),
        };
        let expected_body = serde_json::to_string(&expected_request).unwrap();

        mock("POST", "/telemetry")
            .match_header("content-type", "application/json")
            .match_body(expected_body.as_str())
            .with_status(200)
            .create()
    }

    fn unreachable_server() -> Mock {
        mock("POST", "/telemetry")
            .with_status(200)
            .with_body_from_fn(|_| {
                thread::sleep(Duration::from_millis(100));
                Ok(())
            })
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