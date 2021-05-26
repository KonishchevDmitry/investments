#[cfg(test)] use csv::StringRecord;
use lazy_static::lazy_static;
use regex::Regex;

use crate::broker_statement::corporate_actions::{CorporateAction, CorporateActionType, StockSplitRatio};
use crate::core::{EmptyResult, GenericResult};
use crate::formatting::format_date;
#[cfg(test)] use crate::types::{Date, DateTime, Decimal};
use crate::util::{self, DecimalRestrictions};

use super::StatementParser;
use super::common::{self, Record, RecordParser, parse_symbol};
#[cfg(test)] use super::common::RecordSpec;

pub struct CorporateActionsParser {
    corporate_actions: Vec<CorporateAction>,
}

impl RecordParser for CorporateActionsParser {
    fn skip_totals(&self) -> bool {
        true
    }

    fn parse(&mut self, _parser: &mut StatementParser, record: &Record) -> EmptyResult {
        self.corporate_actions.push(parse(record)?);
        Ok(())
    }
}

impl CorporateActionsParser {
    pub fn new() -> CorporateActionsParser {
        CorporateActionsParser {
            corporate_actions: Vec::new(),
        }
    }

    pub fn commit(self, parser: &mut StatementParser) -> EmptyResult {
        // Here we postprocess parsed corporate actions:
        // * Complex stock splits are represented by two records, so we join them here

        let mut stock_splits = Vec::<CorporateAction>::new();

        for action in self.corporate_actions {
            match action.action {
                CorporateActionType::StockSplit {..} => {
                    if let Some(last) = stock_splits.last() {
                        if action.time == last.time && action.symbol == last.symbol {
                            stock_splits.push(action);
                        } else {
                            parser.statement.corporate_actions.push(join_stock_splits(stock_splits)?);
                            stock_splits = vec![action];
                        }
                    } else {
                        stock_splits.push(action);
                    }
                },
                _ => parser.statement.corporate_actions.push(action),
            }
        }

        if !stock_splits.is_empty() {
            parser.statement.corporate_actions.push(join_stock_splits(stock_splits)?);
        }

        Ok(())
    }
}

fn parse(record: &Record) -> GenericResult<CorporateAction> {
    let asset_category = record.get_value("Asset Category")?;
    if asset_category != "Stocks" {
        return Err!("Unsupported asset category of corporate action: {:?}", asset_category);
    }

    let time = record.parse_date_time("Date/Time")?;
    let report_date = Some(record.parse_date("Report Date")?);

    let description = util::fold_spaces(record.get_value("Description")?);
    let description = description.as_ref();

    lazy_static! {
        static ref REGEX: Regex = Regex::new(&format!(concat!(
            r"^(?P<symbol>{symbol}) ?\({id}\) (?P<action>Split|Spinoff) ",
            r"(?P<to>[1-9]\d*) for (?P<from>[1-9]\d*) ",
            r"\((?P<child_symbol>{symbol})(?:\.OLD)?, [^,)]+, {id}\)$",
        ), symbol=common::STOCK_SYMBOL_REGEX, id=common::STOCK_ID_REGEX)).unwrap();
    }

    Ok(if let Some(captures) = REGEX.captures(description) {
        let symbol = parse_symbol(captures.name("symbol").unwrap().as_str())?;

        match captures.name("action").unwrap().as_str() {
            "Split" => {
                let from: u32 = captures.name("from").unwrap().as_str().parse()?;
                let to: u32 = captures.name("to").unwrap().as_str().parse()?;
                let ratio = StockSplitRatio::new(from, to);

                let change = record.parse_quantity("Quantity", DecimalRestrictions::NonZero)?;
                let (from_change, to_change) = if change.is_sign_positive() {
                    (None, Some(change))
                } else {
                    (Some(-change), None)
                };

                CorporateAction {
                    time: time.into(), report_date, symbol,
                    action: CorporateActionType::StockSplit{ratio, from_change, to_change},
                }
            },
            "Spinoff" => {
                let quantity = record.parse_quantity("Quantity", DecimalRestrictions::StrictlyPositive)?;
                let currency = record.get_value("Currency")?.to_owned();

                CorporateAction {
                    time: time.into(), report_date, symbol,
                    action: CorporateActionType::Spinoff {
                        symbol: parse_symbol(captures.name("child_symbol").unwrap().as_str())?,
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

fn join_stock_splits(mut actions: Vec<CorporateAction>) -> GenericResult<CorporateAction> {
    match actions.len() {
        0 => unreachable!(),
        1 => {
            // Simple stock split
            return Ok(actions.pop().unwrap())
        },
        2 => {
            // Complex stock splits are represented by two records
        },
        _ => {
            let action = actions.first().unwrap();
            return Err!(
                "Unsupported stock split: {} at {}",
                action.symbol, format_date(action.time.date));
        },
    };

    let supplementary_action = actions.pop().unwrap();
    let mut action = actions.pop().unwrap();

    let (ratio, from_change, to_change) = match (action.action, supplementary_action.action) {
        // It looks like the records may have an arbitrary order
        (
            CorporateActionType::StockSplit {ratio: first_ratio, from_change: Some(from_change), to_change: None},
            CorporateActionType::StockSplit {ratio: second_ratio, from_change: None, to_change: Some(to_change)},
        ) if first_ratio == second_ratio => {
            (first_ratio, from_change, to_change)
        },
        (
            CorporateActionType::StockSplit {ratio: first_ratio, from_change: None, to_change: Some(to_change)},
            CorporateActionType::StockSplit {ratio: second_ratio, from_change: Some(from_change), to_change: None},
        ) if first_ratio == second_ratio => {
            (first_ratio, from_change, to_change)
        },
        _ => {
            return Err!(
                "Unsupported stock split: {} at {}",
                action.symbol, format_date(action.time.date));
        },
    };

    action.action = CorporateActionType::StockSplit {
        ratio,
        from_change: Some(from_change),
        to_change: Some(to_change),
    };
    Ok(action)
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

    #[rstest(record, symbol, time, report_date, to, from, from_change, to_change,
        case(vec![
            "Stocks", "USD", "2020-08-31", "2020-08-28, 20:25:00",
            "AAPL(US0378331005) Split 4 for 1 (AAPL, APPLE INC, US0378331005)",
            "111", "0", "0", "0", "",
        ], "AAPL", date_time!(2020, 8, 28, 20, 25, 00), date!(2020, 8, 31), 4, 1, None, Some(dec!(111))),

        case(vec![
            "Stocks", "USD", "2021-01-21", "2021-01-20, 20:25:00",
            "SLG(US78440X1019) Split 100000 for 102918 (SLG.OLD, SL GREEN REALTY CORP, US78440X1019)",
            "-7", "0", "0", "0", "",
        ], "SLG", date_time!(2021, 1, 20, 20, 25, 00), date!(2021, 1, 21), 100000, 102918, Some(dec!(7)), None),

        case(vec![
            "Stocks", "USD", "2020-08-03", "2020-07-31, 20:25:00",
            "VISL(US92836Y2019) Split 1 for 6 (VISL, VISLINK TECHNOLOGIES INC, US92836Y2019)",
            "-80", "0", "0", "0", "",
        ], "VISL", date_time!(2020, 7, 31, 20, 25, 00), date!(2020, 8, 3), 1, 6, Some(dec!(80)), None),
        case(vec![
            "Stocks", "USD", "2020-08-03", "2020-07-31, 20:25:00",
            "VISL(US92836Y2019) Split 1 for 6 (VISL, VISLINK TECHNOLOGIES INC, US92836Y3009)",
            "13.3333", "0", "0", "0", "",
        ], "VISL", date_time!(2020, 7, 31, 20, 25, 00), date!(2020, 8, 3), 1, 6, None, Some(dec!(13.3333))),
    )]
    fn stock_split_parsing(
        record: Vec<&str>, symbol: &str, time: DateTime, report_date: Date, to: u32, from: u32,
        from_change: Option<Decimal>, to_change: Option<Decimal>,
    ) {
        let spec = RecordSpec::new("test", CORPORATE_ACTION_FIELDS.clone(), 0);
        let record = StringRecord::from(record);
        let record = Record::new(&spec, &record);

        assert_eq!(parse(&record).unwrap(), CorporateAction {
            time: time.into(),
            report_date: Some(report_date),

            symbol: symbol.to_owned(),
            action: CorporateActionType::StockSplit{
                ratio: StockSplitRatio::new(from, to),
                from_change, to_change,
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
            time: date_time!(2020, 11, 16, 20, 25, 00).into(),
            report_date: Some(date!(2020, 11, 17)),

            symbol: s!("PFE"),
            action: CorporateActionType::Spinoff {
                symbol: s!("VTRS"),
                quantity: dec!(9.3059),
                currency: s!("USD"),
            },
        });
    }
}