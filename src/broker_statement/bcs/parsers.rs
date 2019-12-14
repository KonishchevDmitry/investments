use chrono::Duration;
use lazy_static::lazy_static;
use regex::Regex;

use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::formatting;
use crate::types::{Date, Decimal};
use crate::xls::{self, TableReader, Cell, SkipCell};

use xls_table_derive::XlsTableRow;

use super::{Parser, SectionParser};
use super::common::parse_date;

pub struct PeriodParser {
}

impl SectionParser for PeriodParser {
    fn consume_title(&self) -> bool { false }

    fn parse(&self, parser: &mut Parser) -> EmptyResult {
        let row = xls::strip_row_expecting_columns(parser.sheet.next_row_checked()?, 2)?;
        let period = parse_period(xls::get_string_cell(row[1])?)?;
        parser.statement.set_period(period)?;
        Ok(())
    }
}

fn parse_period(value: &str) -> GenericResult<(Date, Date)> {
    lazy_static! {
        static ref PERIOD_REGEX: Regex = Regex::new(
            r"^с (?P<start>\d{2}\.\d{2}\.\d{4}) по (?P<end>\d{2}\.\d{2}\.\d{4})$").unwrap();
    }

    let captures = PERIOD_REGEX.captures(value).ok_or_else(|| format!(
        "Invalid period: {:?}", value))?;
    let start = parse_date(captures.name("start").unwrap().as_str())?;
    let end = parse_date(captures.name("end").unwrap().as_str())? + Duration::days(1);

    if start >= end {
        return Err!("Invalid period: {}", formatting::format_period(start, end));
    }

    Ok((start, end))
}

pub struct CashFlowParser {
}

impl SectionParser for CashFlowParser {
    // FIXME: It's a prototype - rewrite
    fn parse(&self, parser: &mut Parser) -> EmptyResult {
        let _: Vec<CashFlowRow> = xls::read_table(&mut parser.sheet)?;
        Ok(())
    }
}

#[derive(XlsTableRow, Debug)]
struct CashFlowRow {
    #[column(name="Дата")]
    _1: SkipCell,
    #[column(name="Операция")]
    _2: SkipCell,
    #[column(name="Сумма зачисления")]
    _3: SkipCell,
    #[column(name="Сумма списания")]
    _4: SkipCell,
    #[column(name="В т.ч.НДС (руб.)")]
    _5: SkipCell,
    #[column(name="Остаток (+/-)")]
    _6: SkipCell,
    #[column(name="в т.ч. гарант. обеспечение")]
    _7: SkipCell,
    #[column(name="в т.ч. депозитная маржа")]
    _8: SkipCell,
    #[column(name="Площадка")]
    _9: SkipCell,
    #[column(name="Примечание")]
    _10: SkipCell,
    #[column(name="Промежуточный клиринг (FORTS)")]
    _11: SkipCell,
}

impl TableReader for CashFlowRow {
    fn skip_row(row: &[&Cell]) -> GenericResult<bool> {
        Ok(xls::get_string_cell(row[0])? == "Итого:")
    }
}

pub struct AssetsParser {
}

impl SectionParser for AssetsParser {
    // FIXME: It's a prototype - rewrite
    fn parse(&self, parser: &mut Parser) -> EmptyResult {
        parser.sheet.skip_empty_rows();
        parser.sheet.skip_non_empty_rows();
        parser.sheet.skip_empty_rows();
        parser.sheet.next_row_checked()?;

        let assets: Vec<AssetsRow> = xls::read_table(&mut parser.sheet)?;

        for asset in &assets {
            if asset.asset == "Рубль" {
                if let Some(amount) = asset.end_value {
                    parser.statement.cash_assets.deposit(Cash::new("RUB", amount))
                }
            } else {
                continue;
            }
        }

        parser.statement.set_starting_assets(false)?;

        Ok(())
    }
}

#[derive(XlsTableRow, Debug)]
struct AssetsRow {
    #[column(name="Вид актива")]
    asset: String,
    #[column(name="Номер гос. регистрации ЦБ/ ISIN")]
    _1: SkipCell,
    #[column(name="Тип ЦБ (№ вып.)")]
    _2: SkipCell,
    #[column(name="Кол-во ценных бумаг")]
    _3: SkipCell,
    #[column(name="Цена закрытия/котировка вторич.(5*)")]
    _4: SkipCell,
    #[column(name="Сумма НКД")]
    _5: SkipCell,
    #[column(name="Сумма, в т.ч. НКД")]
    _6: SkipCell,
    #[column(name="Кол-во ценных бумаг")]
    _7: SkipCell,
    #[column(name="Цена закрытия/ котировка вторич.(5*)")]
    _8: SkipCell,
    #[column(name="Сумма НКД")]
    _9: SkipCell,
    #[column(name="Сумма, в т.ч. НКД")]
    end_value: Option<Decimal>,
    #[column(name="Организатор торгов (2*)")]
    _10: SkipCell,
    #[column(name="Место хранения")]
    _11: SkipCell,
    #[column(name="Эмитент")]
    _12: SkipCell,
}

impl TableReader for AssetsRow {
    fn skip_row(row: &[&Cell]) -> GenericResult<bool> {
        Ok(xls::get_string_cell(row[0])? == "Итого:")
    }
}