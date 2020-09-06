use lazy_static::lazy_static;
use regex::Regex;

use crate::broker_statement::corporate_actions::{CorporateAction, CorporateActionType};
use crate::core::{EmptyResult, GenericResult};
use crate::types::Date;

use super::StatementParser;
use super::common::{Record, RecordParser};

pub struct CorporateActionsParser {}

impl RecordParser for CorporateActionsParser {
    fn skip_totals(&self) -> bool {
        true
    }

    fn parse(&self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let asset_category = record.get_value("Asset Category")?;
        let date = record.parse_date("Report Date")?;
        let description = record.get_value("Description")?;

        let corporate_action = parse(date, asset_category, description)?;
        parser.statement.corporate_actions.push(corporate_action);

        Ok(())
    }
}

fn parse(date: Date, asset_category: &str, description: &str) -> GenericResult<CorporateAction> {
    lazy_static! {
        static ref STOCK_SPLIT_REGEX: Regex = Regex::new(
            r"^(?P<symbol>[A-Z]+) ?\([A-Z0-9]+\) Split (?P<divisor>\d+) for (?P<dividend>\d+) \([^)]+\)$",
        ).unwrap();
    }

    Ok(match (asset_category, STOCK_SPLIT_REGEX.captures(description)) {
        ("Stocks", Some(captures)) => {
            let symbol = captures.name("symbol").unwrap().as_str();
            let divisor: u32 = captures.name("divisor").unwrap().as_str().parse()?;

            if divisor == 0 || captures.name("dividend").unwrap().as_str() != "1" {
                return Err!("Unsupported stock split: {:?}", description);
            }

            CorporateAction {
                date,
                symbol: symbol.to_owned(),
                action: CorporateActionType::StockSplit(divisor),
            }
        },
        _ => return Err!("Unsupported corporate action: {:?}", description),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing() {
        let date = date!(1, 1, 1);

        assert_eq!(
            parse(
                date, "Stocks", "AAPL(US0378331005) Split 4 for 1 (AAPL, APPLE INC, US0378331005)",
            ).unwrap(),

            CorporateAction {
                date,
                symbol: s!("AAPL"),
                action: CorporateActionType::StockSplit(4),
            }
        );
    }
}