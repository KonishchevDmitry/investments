use std::collections::HashMap;

use lazy_static::lazy_static;
use regex::Regex;

use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::broker_statement::Dividend;
use crate::formatting;
use crate::localities;
use crate::types::Date;
use crate::util::DecimalRestrictions;

use super::StatementParser;
use super::common::{Record, RecordParser, parse_date};
use super::taxes::TaxId;

pub struct DividendInfo {
    date: Date,
    description: String,
    amount: Cash,
}

pub struct DividendsParser {}

impl RecordParser for DividendsParser {
    fn parse(&self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let currency = record.get_value("Currency")?;
        if currency == "Total" {
            return Ok(());
        }

        let date = parse_date(record.get_value("Date")?)?;
        let description = record.get_value("Description")?.to_owned();
        let amount = Cash::new(
            currency, record.parse_cash("Amount", DecimalRestrictions::StrictlyPositive)?);

        parser.dividends.push(DividendInfo {
            date: date,
            description: description,
            amount: amount,
        });

        Ok(())
    }
}

pub fn parse_dividends(mut dividends_info: Vec<DividendInfo>, taxes: &mut HashMap<TaxId, Cash>) -> GenericResult<Vec<Dividend>> {
    let country = localities::us();
    let mut dividends = Vec::with_capacity(dividends_info.len());

    for dividend in dividends_info.drain(..) {
        let (issuer, tax_description) = parse_dividend_description(&dividend.description)?;
        let tax_id = (dividend.date, tax_description);
        let paid_tax = taxes.remove(&tax_id).ok_or_else(|| format!(
            "Unable to match the following dividend to paid taxes: {} / {:?} ({:?})",
            formatting::format_date(dividend.date), dividend.description, tax_id.1))?;

        if dividend.amount.currency != country.currency {
            return Err!(
                "Got {} / {:?} dividend in {} currency when {} is expected",
                formatting::format_date(dividend.date), dividend.description,
                dividend.amount.currency, country.currency);
        }

        let expected_tax = country.tax_to_pay(dividend.amount.amount, None);
        if paid_tax != Cash::new(dividend.amount.currency, expected_tax) {
            return Err!(
                "Paid tax for {} / {:?} dividend is not equal to expected one: {} vs {}",
                formatting::format_date(dividend.date), dividend.description, paid_tax,
                expected_tax);
        }

        dividends.push(Dividend {
            date: dividend.date,
            issuer: issuer,
            amount: dividend.amount,
            paid_tax: paid_tax,
        })
    }

    Ok(dividends)
}

fn parse_dividend_description(description: &str) -> GenericResult<(String, String)> {
    lazy_static! {
        static ref description_regex: Regex = Regex::new(
            r"^(?P<description>(?P<issuer>[A-Z]+)\b.+) \([^)]+\)$").unwrap();
    }

    let captures = description_regex.captures(description).ok_or_else(|| format!(
        "Unexpected dividend description: {:?}", description))?;

    let issuer = captures.name("issuer").unwrap().as_str().to_owned();
    let short_description = captures.name("description").unwrap().as_str().to_owned();
    let tax_description = short_description + " - US Tax";

    Ok((issuer, tax_description))
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