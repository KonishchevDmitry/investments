use lazy_static::lazy_static;
use regex::Regex;

use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::broker_statement::taxes::TaxId;
use crate::broker_statement::dividends::{Dividend, DividendWithoutPaidTax, TaxIdExtractor};
use crate::util::DecimalRestrictions;

use super::StatementParser;
use super::common::{Record, RecordParser, parse_date};

pub struct DividendsParser {}

impl RecordParser for DividendsParser {
    fn parse(&self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let currency = record.get_value("Currency")?;
        if currency == "Total" {
            return Ok(());
        }

        let date = parse_date(record.get_value("Date")?)?;
        let description = record.get_value("Description")?;
        let amount = Cash::new(
            currency, record.parse_cash("Amount", DecimalRestrictions::StrictlyPositive)?);

        let (issuer, tax_description) = parse_dividend_description(description)?;

        if let Some(tax_description) = tax_description {
            parser.statement.dividends_without_paid_tax.push(DividendWithoutPaidTax {
                date: date,
                issuer: issuer,
                amount: amount,
                tax_extractor: TaxIdExtractor::new(TaxId::new(date, &tax_description))
            });
        } else {
            parser.statement.dividends.push(Dividend {
                date: date,
                issuer: issuer,
                amount: amount,
                paid_tax: Cash::new(amount.currency, dec!(0)),
            })
        }

        Ok(())
    }
}

fn parse_dividend_description(description: &str) -> GenericResult<(String, Option<String>)> {
    lazy_static! {
        static ref DESCRIPTION_REGEX: Regex = Regex::new(
            r"^(?P<description>(?P<issuer>[A-Z]+)\b.+) \((?P<type>[^)]+)\)$").unwrap();
    }

    let captures = DESCRIPTION_REGEX.captures(description).ok_or_else(|| format!(
        "Unexpected dividend description: {:?}", description))?;

    let issuer = captures.name("issuer").unwrap().as_str().to_owned();
    let income_type = captures.name("type").unwrap().as_str().to_owned();

    let tax_description = if income_type == "Return of Capital" {
        None
    } else {
        let description = captures.name("description").unwrap().as_str().to_owned();
        Some(description + " - US Tax")
    };

    Ok((issuer, tax_description))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dividend_description_parsing() {
        assert_eq!(parse_dividend_description(
            "VNQ (US9229085538) Cash Dividend USD 0.7318 (Ordinary Dividend)").unwrap(),
            (s!("VNQ"), Some(s!("VNQ (US9229085538) Cash Dividend USD 0.7318 - US Tax"))),
        );

        assert_eq!(parse_dividend_description(
            "IEMG(US46434G1031) Cash Dividend 0.44190500 USD per Share (Ordinary Dividend)").unwrap(),
            (s!("IEMG"), Some(s!("IEMG(US46434G1031) Cash Dividend 0.44190500 USD per Share - US Tax"))),
        );

        assert_eq!(parse_dividend_description(
            "BND(US9219378356) Cash Dividend 0.18685800 USD per Share (Mixed Income)").unwrap(),
            (s!("BND"), Some(s!("BND(US9219378356) Cash Dividend 0.18685800 USD per Share - US Tax"))),
        );

        assert_eq!(parse_dividend_description(
            "VNQ(US9229085538) Cash Dividend 0.82740000 USD per Share (Return of Capital)").unwrap(),
            (s!("VNQ"), None),
        );
    }
}