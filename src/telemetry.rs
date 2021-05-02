use std::collections::HashSet;

use crate::brokers::Broker;

pub struct TelemetryRecord {
    command: String,
    brokers: Vec<String>,
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