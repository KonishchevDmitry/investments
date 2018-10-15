use std::collections::HashMap;

use regex::Regex;

use core::{EmptyResult, GenericResult};
use currency::Cash;
use broker_statement::ib::IbStatementParser;
use broker_statement::ib::common::{Record, RecordParser, parse_date};
use types::Date;

pub struct DividendInfo {
    date: Date,
    description: String,
    amount: Cash,
}

pub struct DividendsParser {}

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

// FIXME: HERE
pub fn parse_dividends(mut dividends: Vec<DividendInfo>, taxes: &mut HashMap<(Date, String), Cash>) -> EmptyResult {
    let tax_rate = decs!("0.1");

    for dividend in dividends.drain(..) {
        let (ticker, tax_description) = parse_dividend_description(&dividend.description)?;
        let tax_id = (dividend.date, tax_description);

        let paid_taxes = match taxes.remove(&tax_id) {
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