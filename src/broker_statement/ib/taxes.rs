use lazy_static::lazy_static;
use regex::Regex;

use crate::core::{GenericResult, EmptyResult};
use crate::instruments::InstrumentId;
use crate::util::DecimalRestrictions;

use super::StatementParser;
use super::cash_flows::CashFlowId;
use super::common::{self, Record, RecordParser, SecurityID, parse_symbol};

// Every year IB has to adjust the 1042 withholding (i.e. withholding on US dividends paid to non-US
// accounts) to reflect dividend reclassifications. This is typically done in February the following
// year. As such, the majority of these adjustments are refunds to customers. The typical case is
// when IB's best information at the time of paying a dividend indicates that the distribution is an
// ordinary dividend (and therefore subject to withholding), then later at year end, the dividend is
// reclassified as Return of Capital, proceeds, or capital gains (all of which are not subject to
// 1042 withholding).
//
// So withholding in previous year's statements should be reviewed against February statement's
// withholding adjustments. As it turns out, dates may not match.
//
// At this time we match dividends on taxes using (date, symbol) pair. Matching by description
// turned out to be too fragile.
pub struct WithholdingTaxParser {}

impl RecordParser for WithholdingTaxParser {
    fn skip_totals(&self) -> bool {
        true
    }

    fn parse(&mut self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let currency = record.get_value("Currency")?;
        let description = record.get_value("Description")?;
        let statement_date = record.parse_date("Date")?;

        let issuer = parse_tax_description(description)?;
        let actual_date = parser.tax_remapping.map(statement_date, description);

        // Tax amount is represented as a negative number.
        //
        // Positive number is used to cancel a previous tax payment and usually followed by another
        // negative number.
        let tax = record.parse_cash("Amount", currency, DecimalRestrictions::NonZero)?;

        let cash_flow_id = CashFlowId::new(statement_date, description, tax);
        let cash_flow_date = parser.cash_flows.map(&parser.statement, cash_flow_id, actual_date)?;

        let accruals = parser.statement.tax_accruals(
            actual_date, InstrumentId::Symbol(issuer), true);

        if tax.is_positive() {
            accruals.reverse(cash_flow_date, tax);
        } else {
            accruals.add(cash_flow_date, -tax);
        }

        Ok(())
    }
}

fn parse_tax_description(description: &str) -> GenericResult<String> {
    lazy_static! {
        static ref DESCRIPTION_REGEX: Regex = Regex::new(&format!(
            r"^(?P<issuer>{symbol}) ?\({id}\) .+ - [A-Z]{{2}} Tax$",
            symbol=common::STOCK_SYMBOL_REGEX, id=SecurityID::REGEX)).unwrap();
    }

    let captures = DESCRIPTION_REGEX.captures(description).ok_or_else(|| format!(
        "Unexpected tax description: {:?}", description))?;

    parse_symbol(captures.name("issuer").unwrap().as_str())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(description, symbol,
        case("BND (US9219378356) Cash Dividend USD 0.181007 - US Tax", "BND"),
        case("BND(US9219378356) Cash Dividend USD 0.193413 per Share - US Tax", "BND"),
        case("BND(US9219378356) Cash Dividend 0.18366600 USD per Share - US Tax", "BND"),
        case("BND(43645828) Cash Dividend 0.19446400 USD per Share - US Tax", "BND"),
        case("UNIT(US91325V1089) Payment in Lieu of Dividend - US Tax", "UNIT"),
        case("ETN(IE00B8KQN827) Cash Dividend USD 0.73 per Share - IE Tax", "ETN"),
    )]
    fn tax_parsing(description: &str, symbol: &str) {
        assert_eq!(parse_tax_description(description).unwrap(), symbol.to_owned());
    }
}
