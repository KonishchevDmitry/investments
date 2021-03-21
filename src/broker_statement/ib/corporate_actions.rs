#[cfg(test)] use csv::StringRecord;
use lazy_static::lazy_static;
use regex::Regex;

use crate::broker_statement::corporate_actions::{CorporateAction, CorporateActionType};
use crate::core::{EmptyResult, GenericResult};
#[cfg(test)] use crate::types::Decimal;
use crate::util::{self, DecimalRestrictions};

use super::StatementParser;
use super::common::{Record, RecordParser, parse_date_time};
#[cfg(test)] use super::common::RecordSpec;

pub struct CorporateActionsParser {}

impl RecordParser for CorporateActionsParser {
    fn skip_totals(&self) -> bool {
        true
    }

    fn parse(&self, parser: &mut StatementParser, record: &Record) -> EmptyResult {
        let corporate_action = parse(record)?;
        parser.statement.corporate_actions.push(corporate_action);
        Ok(())
    }
}

fn parse(record: &Record) -> GenericResult<CorporateAction> {
    let asset_category = record.get_value("Asset Category")?;
    if asset_category != "Stocks" {
        return Err!("Unsupported asset category of corporate action: {:?}", asset_category);
    }

    let execution_date = record.parse_date("Report Date")?;

    let description = util::fold_spaces(record.get_value("Description")?);
    let description = description.as_ref();

    lazy_static! {
        static ref REGEX: Regex = Regex::new(concat!(
            r"^(?P<symbol>[A-Z]+) ?\([A-Z0-9]+\) (?P<action>Split|Spinoff) ",
            r"(?P<to>[1-9]\d*) for (?P<from>[1-9]\d*) ",
            r"\((?P<child_symbol>[A-Z]+), [^,)]+, [A-Z0-9]+\)$",
        )).unwrap();
    }

    Ok(if let Some(captures) = REGEX.captures(description) {
        let symbol = captures.name("symbol").unwrap().as_str().to_owned();

        match captures.name("action").unwrap().as_str() {
            "Split" => {
                let from: u32 = captures.name("from").unwrap().as_str().parse()?;
                let to: u32 = captures.name("to").unwrap().as_str().parse()?;

                let change = record.parse_amount("Quantity", DecimalRestrictions::NonZero)?;
                let (from_change, to_change) = if change.is_sign_positive() {
                    (None, Some(change))
                } else {
                    (Some(-change), None)
                };

                CorporateAction {
                    date: execution_date, symbol,
                    action: CorporateActionType::StockSplit{from, from_change, to, to_change},
                }
            },
            "Spinoff" => {
                let conclusion_date = parse_date_time(record.get_value("Date/Time")?)?.date();
                let quantity = record.parse_amount("Quantity", DecimalRestrictions::StrictlyPositive)?;
                let currency = record.get_value("Currency")?.to_owned();

                CorporateAction {
                    date: execution_date, symbol,
                    action: CorporateActionType::Spinoff {
                        date: conclusion_date,
                        symbol: captures.name("child_symbol").unwrap().as_str().to_owned(),
                        quantity, currency,
                    },
                }
            },
            _ => unreachable!(),
        }
    } else {
        return Err!("Unsupported corporate action: {:?}", description);
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    lazy_static! {
        static ref CORPORATE_ACTION_FIELDS: Vec<&'static str> =
            "Asset Category,Currency,Report Date,Date/Time,Description,Quantity,Proceeds,Value,Realized P/L,Code"
            .split(',').collect();
    }

    #[test]
    fn stock_split_parsing() {
        let spec = RecordSpec::new("test", CORPORATE_ACTION_FIELDS.clone(), 0);
        let record = StringRecord::from(vec![
            "Stocks", "USD", "2020-08-31", "2020-08-28, 20:25:00",
            "AAPL(US0378331005) Split 4 for 1 (AAPL, APPLE INC, US0378331005)",
            "111", "0", "0", "0", "",
        ]);
        let record = Record::new(&spec, &record);

        assert_eq!(parse(&record).unwrap(), CorporateAction {
            date: date!(31, 8, 2020),
            symbol: s!("AAPL"),
            action: CorporateActionType::StockSplit{
                from: 1, from_change: None,
                to: 4, to_change: Some(dec!(111)),
            },
        });
    }

    #[rstest(record, from_change, to_change,
        case(vec![
            "Stocks", "USD", "2020-08-03", "2020-07-31, 20:25:00",
            "VISL(US92836Y2019) Split 1 for 6 (VISL, VISLINK TECHNOLOGIES INC, US92836Y2019)",
            "-80", "0", "0", "0", "",
        ], Some(dec!(80)), None),
        case(vec![
            "Stocks", "USD", "2020-08-03", "2020-07-31, 20:25:00",
            "VISL(US92836Y2019) Split 1 for 6 (VISL, VISLINK TECHNOLOGIES INC, US92836Y3009)",
            "13.3333", "0", "0", "0", "",
        ], None, Some(dec!(13.3333))),
    )]
    fn reverse_stock_split_parsing(record: Vec<&str>, from_change: Option<Decimal>, to_change: Option<Decimal>) {
        let spec = RecordSpec::new("test", CORPORATE_ACTION_FIELDS.clone(), 0);
        let record = StringRecord::from(record);
        let record = Record::new(&spec, &record);

        assert_eq!(parse(&record).unwrap(), CorporateAction {
            date: date!(3, 8, 2020),
            symbol: s!("VISL"),
            action: CorporateActionType::StockSplit{
                from: 6, from_change,
                to: 1, to_change,
            },
        });
    }

    #[test]
    fn spinoff_parsing() {
        let spec = RecordSpec::new("test", CORPORATE_ACTION_FIELDS.clone(), 0);
        let record = StringRecord::from(vec![
            "Stocks", "USD", "2020-11-17", "2020-11-16, 20:25:00",
            "PFE(US7170811035) Spinoff  124079 for 1000000 (VTRS, VIATRIS INC-W/I, US92556V1061)",
            "9.3059", "0", "0", "0", "",
        ]);
        let record = Record::new(&spec, &record);

        assert_eq!(parse(&record).unwrap(), CorporateAction {
            date: date!(17, 11, 2020),
            symbol: s!("PFE"),
            action: CorporateActionType::Spinoff {
                date: date!(16, 11, 2020),
                symbol: s!("VTRS"),
                quantity: dec!(9.3059),
                currency: s!("USD"),
            },
        });
    }
}