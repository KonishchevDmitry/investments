use std::collections::HashMap;
use std::iter::Iterator;

use csv::{self, StringRecord};
use log::trace;

use crate::brokers::{self, BrokerInfo};
#[cfg(test)] use crate::config::Broker;
use crate::config::Config;
use crate::core::GenericResult;
use crate::currency::Cash;

use super::{BrokerStatement, BrokerStatementReader, BrokerStatementBuilder};

use self::common::{Record, RecordParser, format_record};

mod common;
mod dividends;
mod parsers;
mod taxes;
mod trades;

pub struct StatementReader {
    broker_info: BrokerInfo,
}

impl StatementReader {
    pub fn new(config: &Config) -> GenericResult<Box<BrokerStatementReader>> {
        Ok(Box::new(StatementReader {
            broker_info: brokers::interactive_brokers(config)?,
        }))
    }
}

impl BrokerStatementReader for StatementReader {
    fn is_statement(&self, file_name: &str) -> bool {
        file_name.ends_with(".csv")
    }

    fn read(&self, path: &str) -> GenericResult<BrokerStatement> {
        let parser = StatementParser {
            statement: BrokerStatementBuilder::new(self.broker_info.clone()),
            currency: None,
            taxes: HashMap::new(),
            dividends: Vec::new(),
        };

        parser.parse(path)
    }
}

enum State {
    None,
    Record(StringRecord),
    Header(StringRecord),
}

pub struct StatementParser {
    statement: BrokerStatementBuilder,
    currency: Option<String>,
    taxes: HashMap<taxes::TaxId, Cash>,
    dividends: Vec<dividends::DividendInfo>,
}

impl StatementParser {
    fn parse(mut self, path: &str) -> GenericResult<BrokerStatement> {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_path(path)?;

        let mut records = reader.records();
        let mut state = Some(State::None);

        'state: loop {
            match state.take().unwrap() {
                State::None => {
                    match records.next() {
                        Some(result) => state = Some(State::Record(result?)),
                        None => break,
                    };
                }
                State::Record(record) => {
                    if record.len() < 2 {
                        return Err!("Invalid record: {}", format_record(&record));
                    }

                    if record.get(1).unwrap() == "Header" {
                        state = Some(State::Header(record));
                    } else if record.get(1).unwrap() == "" {
                        trace!("Headerless record: {}.", format_record(&record));
                        state = Some(State::None);
                    } else {
                        return Err!("Invalid record: {}", format_record(&record));
                    }
                },
                State::Header(record) => {
                    let (name, fields) = parse_header(&record)?;

                    // FIXME: Remember seen records and check?
                    let parser: Box<RecordParser> = match name {
                        "Statement" => Box::new(parsers::StatementInfoParser {}),
                        "Account Information" => Box::new(parsers::AccountInformationParser {}),
                        "Change in NAV" => Box::new(parsers::ChangeInNavParser {}),
                        "Cash Report" => Box::new(parsers::CashReportParser {}),
                        "Open Positions" => Box::new(trades::OpenPositionsParser {}),
                        "Trades" => Box::new(trades::TradesParser {}),
                        "Deposits & Withdrawals" => Box::new(parsers::DepositsParser {}),
                        "Dividends" => Box::new(dividends::DividendsParser {}),
                        "Withholding Tax" => Box::new(taxes::WithholdingTaxParser {}),
                        "Financial Instrument Information" => Box::new(parsers::FinancialInstrumentInformationParser {}),
                        _ => Box::new(parsers::UnknownRecordParser {}),
                    };

                    let data_types = parser.data_types();
                    let skip_data_types = parser.skip_data_types();

                    while let Some(result) = records.next() {
                        let record = result?;

                        if record.len() < 2 {
                            return Err!("Invalid record: {}", format_record(&record));
                        }

                        if record.get(0).unwrap() != name {
                            state = Some(State::Record(record));
                            continue 'state;
                        } else if record.get(1).unwrap() == "Header" {
                            state = Some(State::Header(record));
                            continue 'state;
                        }

                        if let Some(skip_data_types) = skip_data_types {
                            if skip_data_types.contains(&record.get(1).unwrap()) {
                                continue;
                            }
                        }

                        if let Some(data_types) = data_types {
                            if !data_types.contains(&record.get(1).unwrap()) {
                                return Err!("Invalid data record type: {}", format_record(&record));
                            }
                        }

                        parser.parse(&mut self, &Record {
                            name: name,
                            fields: &fields,
                            values: &record,
                        }).map_err(|e| format!(
                            "Failed to parse ({}) record: {}", format_record(&record), e
                        ))?;
                    }

                    break;
                }
            }
        }

        // FIXME: Ensure that all taxes are handled
        self.statement.dividends = dividends::parse_dividends(self.dividends, &mut self.taxes)?;
        self.statement.get()
    }
}

fn parse_header(record: &StringRecord) -> GenericResult<(&str, Vec<&str>)> {
    let name = record.get(0).unwrap();
    let fields = record.iter().skip(2).collect::<Vec<_>>();
    trace!("Header: {}: {}.", name, format_record(fields.iter().map(|field: &&str| *field)));
    Ok((name, fields))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing() {
        let statement = BrokerStatement::read(
            &Config::mock(), Broker::InteractiveBrokers, "testdata/interactive-brokers").unwrap();

        assert!(!statement.starting_assets);
        assert!(!statement.cash_assets.is_empty());

        assert!(!statement.deposits.is_empty());
        assert!(!statement.stock_buys.is_empty());
        assert!(statement.stock_sells.is_empty());
        assert!(!statement.dividends.is_empty());

        assert!(!statement.open_positions.is_empty());
        assert!(!statement.instrument_names.is_empty());
    }
}