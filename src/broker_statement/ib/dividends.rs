use lazy_static::lazy_static;
use regex::Regex;

use crate::core::{EmptyResult, GenericResult};
use crate::instruments::InstrumentId;
use crate::util::DecimalRestrictions;

use super::StatementParser;
use super::cash_flows::CashFlowId;
use super::common::{self, Record, RecordParser, parse_symbol};

pub struct DividendsParser {}

impl RecordParser for DividendsParser {
    fn skip_totals(&self) -> bool {
        true
    }

    fn parse(&mut self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let currency = record.get_value("Currency")?;
        let statement_date = record.parse_date("Date")?;
        let description = record.get_value("Description")?;
        let amount = record.parse_cash("Amount", currency, DecimalRestrictions::NonZero)?;

        let issuer = parse_dividend_description(description)?;
        let cash_flow_id = CashFlowId::new(statement_date, description, amount);
        let cash_flow_date = parser.cash_flows.map(&parser.statement, cash_flow_id, statement_date)?;

        let accruals = parser.statement.dividend_accruals(
            statement_date, InstrumentId::Symbol(issuer), true);

        if amount.is_negative() {
            accruals.reverse(cash_flow_date, -amount);
        } else {
            accruals.add(cash_flow_date, amount);
        }

        Ok(())
    }
}

fn parse_dividend_description(description: &str) -> GenericResult<String> {
    lazy_static! {
        static ref DESCRIPTION_REGEX: Regex = Regex::new(&format!(
            r"^(?P<issuer>{symbol}) ?\({id}\) ",
            symbol=common::STOCK_SYMBOL_REGEX, id=common::STOCK_ID_REGEX)).unwrap();
    }

    let captures = DESCRIPTION_REGEX.captures(description).ok_or_else(|| format!(
        "Unexpected dividend description: {:?}", description))?;

    parse_symbol(captures.name("issuer").unwrap().as_str())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(description, symbol,
        case("VNQ (US9229085538) Cash Dividend USD 0.7318 (Ordinary Dividend)", "VNQ"),
        case("IEMG(US46434G1031) Cash Dividend 0.44190500 USD per Share (Ordinary Dividend)", "IEMG"),

        case("BND(US9219378356) Cash Dividend 0.18685800 USD per Share (Mixed Income)", "BND"),
        case("VNQ(US9229085538) Cash Dividend 0.82740000 USD per Share (Return of Capital)", "VNQ"),

        case("EXH4(DE000A0H08J9) Cash Dividend EUR 0.013046 per Share (Mixed Income)", "EXH4"),
        case("BND(US9219378356) Cash Dividend USD 0.193413 per Share (Ordinary Dividend)", "BND"),
        case("BND(US9219378356) Cash Dividend USD 0.193413 per Share - Reversal (Ordinary Dividend)", "BND"),
        case("RDS B(US7802591070) Cash Dividend USD 0.32 per Share (Ordinary Dividend)", "RDS-B"),

        case("UNIT(US91325V1089) Payment in Lieu of Dividend (Ordinary Dividend)", "UNIT"),

        case("TEF (US8793822086) Stock Dividend US8793822086 416666667 for 10000000000 (Ordinary Dividend)", "TEF"),
        case("TEF (US8793822086) Stock Dividend US8793822086 416666667 for 10000000000 - REVERSAL (Ordinary Dividend)", "TEF"),
    )]
    fn dividend_parsing(description: &str, symbol: &str) {
        assert_eq!(parse_dividend_description(description).unwrap(), symbol);
    }
}