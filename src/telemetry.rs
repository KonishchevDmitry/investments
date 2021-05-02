use std::collections::HashSet;

use serde::Serialize;
use diesel::{self, prelude::*};
// FIXME(konishchev): Implement
// #[cfg(test)] use mockito::{self, Mock, mock};

use crate::brokers::Broker;
use crate::core::EmptyResult;
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

pub struct Telemetry {
    db: db::Connection,
}

impl Telemetry {
    pub fn new(connection: db::Connection) -> Telemetry {
        Telemetry {db: connection}
    }

    // FIXME(konishchev): Implement
    pub fn add(&self, record: TelemetryRecord) -> EmptyResult {
        let payload = serde_json::to_string(&record)?;

        diesel::insert_into(telemetry::table)
            .values(models::NewTelemetryRecord {payload: &payload})
            .execute(&*self.db)?;

        Ok(())
    }

    // FIXME(konishchev): Implement
    /*
    fn send(&self) -> EmptyResult {
        let mut records = telemetry::table
            .select((telemetry::id, telemetry::payload))
            .order_by(telemetry::id.asc())
            .load::<(i64, String)>(&*self.db)?;

        const MAX_RECORDS: usize = 10;
        let count = records.len();

        let to_send = if count > MAX_RECORDS {
            let drop_index = count - MAX_RECORDS;
            let drop_below = records[drop_index].0;
            diesel::delete(telemetry::table.filter(telemetry::id.lt(drop_below)))
                .execute(&*database)?;
            &records[drop_index..]
        } else {
            &records
        };

        #[derive(Serialize)]
        pub struct TelemetryRecords {
            records: Vec<serde_json::Value>,
        }

        let mut payloads = Vec::with_capacity(to_send.len());
        for payload in payloads {
            let payload = serde_json::from_str(payload)?;
            payloads.push(payload);
        }

        #[cfg(not(test))] let base_url = "http://www.cbr.ru";
        #[cfg(test)] let base_url = mockito::server_url();

        Ok(())
    }
     */
}

#[cfg(test)]
mod tests {
    use super::*;

    // FIXME(konishchev): Implement
    #[test]
    fn telemetry() {
        let (_database, connection) = db::new_temporary();

        let expected = vec![
            TelemetryRecord::mock(0),
            TelemetryRecord::mock(1),
        ];
        let telemetry = Telemetry::new(connection.clone());
        for record in &expected {
            telemetry.add(record.clone()).unwrap();
        }

        let records = telemetry::table
            .select((telemetry::id, telemetry::payload))
            .order_by(telemetry::id.asc())
            .load::<(i64, String)>(&*connection).unwrap();
        compare(&records, &expected);
    }

    fn compare(actual: &[(i64, String)], expected: &[TelemetryRecord]) {
        let actual: Vec<String> = actual.iter().map(|record| record.1.clone()).collect();
        let expected: Vec<String> = expected.iter().map(|record| serde_json::to_string(record).unwrap()).collect();
        assert_eq!(actual, expected);
    }
}