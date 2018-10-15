use std::collections::HashMap;
use std::iter::Iterator;

use chrono::Duration;
use csv::{self, StringRecord};
use num_traits::identities::Zero;
use regex::Regex;

use core::{EmptyResult, GenericResult};
use currency::{Cash, CashAssets};
use broker_statement::{BrokerStatement, BrokerStatementBuilder};
use types::Date;
use util;

pub struct IbStatementParser {
    statement: BrokerStatementBuilder,
    tickers: HashMap<String, String>,
    taxes: HashMap<(Date, String), Cash>,
    dividends: Vec<DividendInfo>,
}

impl IbStatementParser {
    pub fn new() -> IbStatementParser {
        IbStatementParser {
            statement: BrokerStatementBuilder::new(),
            tickers: HashMap::new(),
            taxes: HashMap::new(),
            dividends: Vec::new(),
        }
    }

    pub fn parse(mut self, path: &str) -> GenericResult<BrokerStatement> {
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

                    let parser: Box<RecordParser> = match name {
                        "Statement" => Box::new(StatementInfoParser {}),
                        "Net Asset Value" => Box::new(NetAssetValueParser {}),
                        "Deposits & Withdrawals" => Box::new(DepositsParser {}),
                        "Dividends" => Box::new(DividendsParser {}),
                        "Withholding Tax" => Box::new(WithholdingTaxParser {}),
                        "Financial Instrument Information" => Box::new(FinancialInstrumentInformationParser {}),
                        _ => Box::new(UnknownRecordParser {}),
                    };

                    let data_types = parser.data_types();

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

        self.parse_dividend_info()?;

        Ok(self.statement.get().map_err(|e| format!("Invalid statement: {}", e))?)
    }

    // FIXME: HERE
    fn parse_dividend_info(&mut self) -> EmptyResult {
        let tax_rate = decs!("0.1");

        for dividend in self.dividends.drain(..) {
            let (ticker, tax_description) = parse_dividend_description(&dividend.description)?;
            let tax_id = (dividend.date, tax_description);

            let paid_taxes = match self.taxes.remove(&tax_id) {
                Some(paid_taxes) => paid_taxes,
                None => {
                    return Err!(
                        "Unable to match the following dividend to paid taxes: {} / {:?} ({:?})",
                        dividend.date, dividend.description, tax_id.1);
                },
            };

            let mut expected_paid_taxes = dividend.amount;
            expected_paid_taxes.amount *= tax_rate;
            expected_paid_taxes = expected_paid_taxes.round();

            if paid_taxes != expected_paid_taxes {
                return Err!(
                    "Paid taxes for {} / {:?} don't equal to expected ones: {} vs {}",
                    dividend.date, dividend.description, paid_taxes, expected_paid_taxes);
            }
        }

        Ok(())
    }
}

enum State {
    None,
    Record(StringRecord),
    Header(StringRecord),
}

struct Record<'a> {
    name: &'a str,
    fields: &'a Vec<&'a str>,
    values: &'a StringRecord,
}

impl<'a> Record<'a> {
    fn get_value(&self, field: &str) -> GenericResult<&str> {
        if let Some(index) = self.fields.iter().position(|other: &&str| *other == field) {
            if let Some(value) = self.values.get(index + 2) {
                return Ok(value);
            }
        }

        Err!("{:?} record doesn't have {:?} field", self.name, field)
    }
}

fn parse_header(record: &StringRecord) -> GenericResult<(&str, Vec<&str>)> {
    let name = record.get(0).unwrap();
    let fields = record.iter().skip(2).collect::<Vec<_>>();
    trace!("Header: {}: {}.", name, format_record(fields.iter().map(|field: &&str| *field)));
    Ok((name, fields))
}

trait RecordParser {
    fn data_types(&self) -> Option<&'static [&'static str]> { Some(&["Data"]) }
    fn parse(&self, parser: &mut IbStatementParser, record: &Record) -> EmptyResult;
}

struct StatementInfoParser {}

impl RecordParser for StatementInfoParser {
    fn parse(&self, parser: &mut IbStatementParser, record: &Record) -> EmptyResult {
        if record.get_value("Field Name")? == "Period" {
            let period = record.get_value("Field Value")?;
            let period = parse_period(period)?;
            parser.statement.set_period(period)?;
        }

        Ok(())
    }
}

struct NetAssetValueParser {}

impl RecordParser for NetAssetValueParser {
    fn parse(&self, parser: &mut IbStatementParser, record: &Record) -> EmptyResult {
        let asset_class = match record.get_value("Asset Class") {
            // FIXME: We should be able to handle data with different headers somehow
            Err(_) => return Ok(()),
            Ok(asset_class) => asset_class,
        };

        if asset_class == "Cash" || asset_class == "Stock" {
            let currency = "USD"; // FIXME: Get from statement
            let amount = Cash::new_from_string(currency, record.get_value("Current Total")?)?;

            // FIXME: Accumulate in parser?
            // FIXME: Eliminate hacks with Cash type
            parser.statement.total_value = Some(Cash::new(currency, match parser.statement.total_value {
                Some(total_amount) => total_amount.amount + amount.amount,
                None => amount.amount,
            }));
        }

        Ok(())
    }
}

struct DepositsParser {}

impl RecordParser for DepositsParser {
    fn parse(&self, parser: &mut IbStatementParser, record: &Record) -> EmptyResult {
        let currency = record.get_value("Currency")?;
        if currency.starts_with("Total") {
            return Ok(());
        }

        // FIXME: Distinguish withdrawals from deposits
        let date = parse_date(record.get_value("Settle Date")?)?;
        let amount = Cash::new_from_string_positive(currency, record.get_value("Amount")?)?;

        parser.statement.deposits.push(CashAssets::new_from_cash(date, amount));

        Ok(())
    }
}

struct DividendInfo {
    date: Date,
    description: String,
    amount: Cash,
}

struct DividendsParser {}

impl RecordParser for DividendsParser {
    fn parse(&self, parser: &mut IbStatementParser, record: &Record) -> EmptyResult {
        let currency = record.get_value("Currency")?;
        if currency == "Total" {
            return Ok(());
        }

        let date = parse_date(record.get_value("Date")?)?;
        let description = record.get_value("Description")?.to_owned();
        let amount = Cash::new_from_string_positive(currency, record.get_value("Amount")?)?;

        parser.dividends.push(DividendInfo {
            date: date,
            description: description,
            amount: amount,
        });

        Ok(())
    }
}

struct WithholdingTaxParser {}

impl RecordParser for WithholdingTaxParser {
    fn parse(&self, parser: &mut IbStatementParser, record: &Record) -> EmptyResult {
        let currency = record.get_value("Currency")?;
        if currency == "Total" {
            return Ok(());
        }

        let date = parse_date(record.get_value("Date")?)?;
        let description = record.get_value("Description")?.to_owned();

        let tax_id = (date, description.clone());
        let mut tax = Cash::new_from_string(currency, record.get_value("Amount")?)?;

        // Tax amount is represented as a negative number.
        // Positive number is used to cancel a previous tax payment and usually followed by another
        // negative number.
        if tax.amount.is_zero() {
            return Err!("Invalid withholding tax: {}", tax.amount);
        } else if tax.amount.is_sign_positive() {
            return match parser.taxes.remove(&tax_id) {
                Some(cancelled_tax) if cancelled_tax == tax => Ok(()),
                _ => Err!("Invalid withholding tax: {}", tax.amount),
            }
        }

        tax.amount = -tax.amount;

        if let Some(_) = parser.taxes.insert(tax_id, tax) {
            return Err!("Got a duplicate withholding tax: {} / {:?}", date, description);
        }

        Ok(())
    }
}

struct FinancialInstrumentInformationParser {
}

impl RecordParser for FinancialInstrumentInformationParser {
    fn parse(&self, parser: &mut IbStatementParser, record: &Record) -> EmptyResult {
        parser.tickers.insert(
            record.get_value("Symbol")?.to_owned(),
            record.get_value("Description")?.to_owned(),
        );
        Ok(())
    }
}

struct UnknownRecordParser {}

impl RecordParser for UnknownRecordParser {
    fn data_types(&self) -> Option<&'static [&'static str]> {
        None
    }

    fn parse(&self, _parser: &mut IbStatementParser, record: &Record) -> EmptyResult {
        if false {
            trace!("Data: {}.", format_record(record.values.iter().skip(1)));
        }
        Ok(())
    }
}

fn format_record<'a, I>(iter: I) -> String
    where I: IntoIterator<Item = &'a str> {

    iter.into_iter()
        .map(|value| format!("{:?}", value))
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_date(date: &str) -> GenericResult<Date> {
    util::parse_date(date, "%Y-%m-%d")
}

fn parse_period(period: &str) -> GenericResult<(Date, Date)> {
    let dates = period.split(" - ").collect::<Vec<_>>();

    return Ok(match dates.len() {
        1 => {
            let date = parse_period_date(dates[0])?;
            (date, date + Duration::days(1))
        },
        2 => {
            let start = parse_period_date(dates[0])?;
            let end = parse_period_date(dates[1])?;

            if start > end {
                return Err!("Invalid period: {} - {}", start, end);
            }

            (start, end + Duration::days(1))
        },
        _ => return Err!("Invalid date: {:?}", period),
    });
}

fn parse_period_date(date: &str) -> GenericResult<Date> {
    util::parse_date(date, "%B %d, %Y")
}

fn parse_dividend_description(description: &str) -> GenericResult<(String, String)> {
    lazy_static! {
        static ref description_regex: Regex = Regex::new(
            r"^(?P<description>(?P<ticker>[A-Z]+)\b.+) \([^)]+\)$").unwrap();
    }

    let captures = description_regex.captures(description).ok_or_else(|| format!(
        "Unexpected dividend description: {:?}", description))?;

    let ticker = captures.name("ticker").unwrap().as_str().to_owned();
    let short_description = captures.name("description").unwrap().as_str().to_owned();
    let tax_description = short_description + " - US Tax";

    Ok((ticker, tax_description))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_parsing() {
        assert_eq!(parse_date("2018-06-22").unwrap(), date!(22, 6, 2018));
    }

    #[test]
    fn period_parsing() {
        assert_eq!(parse_period("October 1, 2018").unwrap(),
                   (date!(1, 10, 2018), date!(2, 10, 2018)));

        assert_eq!(parse_period("September 30, 2018").unwrap(),
                   (date!(30, 9, 2018), date!(1, 10, 2018)));

        assert_eq!(parse_period("May 21, 2018 - September 28, 2018").unwrap(),
                   (date!(21, 5, 2018), date!(29, 9, 2018)));
    }

    #[test]
    fn dividend_description_parsing() {
        assert_eq!(parse_dividend_description(
            "VNQ (US9229085538) Cash Dividend USD 0.7318 (Ordinary Dividend)").unwrap(),
                   (s!("VNQ"), s!("VNQ (US9229085538) Cash Dividend USD 0.7318 - US Tax")),
        );

        assert_eq!(parse_dividend_description(
            "IEMG(US46434G1031) Cash Dividend 0.44190500 USD per Share (Ordinary Dividend)").unwrap(),
                   (s!("IEMG"), s!("IEMG(US46434G1031) Cash Dividend 0.44190500 USD per Share - US Tax")),
        );

        assert_eq!(parse_dividend_description(
            "BND(US9219378356) Cash Dividend 0.18685800 USD per Share (Mixed Income)").unwrap(),
                   (s!("BND"), s!("BND(US9219378356) Cash Dividend 0.18685800 USD per Share - US Tax")),
        );
    }
}