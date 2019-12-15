use chrono::Duration;
use lazy_static::lazy_static;
use regex::Regex;

use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, CashAssets};
use crate::formatting;
use crate::types::{Date, Decimal};
use crate::util::{self, DecimalRestrictions};
use crate::xls::{self, TableReader, Cell, SkipCell};

use xls_table_derive::XlsTableRow;

use super::{Parser, SectionParser};
use super::common::{parse_date, parse_short_date, parse_currency};

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
    fn consume_title(&self) -> bool {
        false
    }

    fn parse(&self, parser: &mut Parser) -> EmptyResult {
        let title_row = xls::strip_row_expecting_columns(parser.sheet.next_row_checked()?, 1)?;
        let currency = parse_currency(xls::get_string_cell(&title_row[0])?)?;

        for cash_flow in &xls::read_table::<CashFlowRow>(&mut parser.sheet)? {
            self.process_cash_flow(parser, currency, cash_flow)?;
        }

        Ok(())
    }
}

impl CashFlowParser {
    fn process_cash_flow(&self, parser: &mut Parser, currency: &str, cash_flow: &CashFlowRow) -> EmptyResult {
        let date = parse_short_date(&cash_flow.date)?;
        let operation = cash_flow.operation.as_str();

        let mut deposit_restrictions = DecimalRestrictions::Zero;
        let mut withdrawal_restrictions = DecimalRestrictions::Zero;

        match operation {
            "Приход ДС" => {
                deposit_restrictions = DecimalRestrictions::StrictlyPositive;
                parser.statement.cash_flows.push(CashAssets::new(date, currency, cash_flow.deposit));
            },
            "Покупка/Продажа" => {
                deposit_restrictions = DecimalRestrictions::PositiveOrZero;
                withdrawal_restrictions = DecimalRestrictions::PositiveOrZero;
            },
            "Урегулирование сделок" |
            "Вознаграждение компании" |
            "Вознаграждение за обслуживание счета депо" => {
                withdrawal_restrictions = DecimalRestrictions::StrictlyPositive;
            },
            _ => return Err!("Unsupported cash flow operation: {:?}", cash_flow.operation),
        };

        for &(name, value, restrictions) in &[
            ("deposit", cash_flow.deposit, deposit_restrictions),
            ("withdrawal", cash_flow.withdrawal, withdrawal_restrictions),
            ("tax", cash_flow.tax, DecimalRestrictions::Zero),
            ("warranty", cash_flow.warranty, DecimalRestrictions::Zero),
            ("margin", cash_flow.margin, DecimalRestrictions::Zero),
        ] {
            util::validate_decimal(value, restrictions).map_err(|_| format!(
                "Unexpected {} amount for {:?} operation: {}", name, operation, value))?;
        }

        Ok(())
    }
}

#[derive(XlsTableRow, Debug)]
struct CashFlowRow {
    #[column(name="Дата")]
    date: String,
    #[column(name="Операция")]
    operation: String,
    #[column(name="Сумма зачисления")]
    deposit: Decimal,
    #[column(name="Сумма списания")]
    withdrawal: Decimal,
    #[column(name="В т.ч.НДС (руб.)")]
    tax: Decimal,
    #[column(name="Остаток (+/-)")]
    _5: SkipCell,
    #[column(name="в т.ч. гарант. обеспечение")]
    warranty: Decimal,
    #[column(name="в т.ч. депозитная маржа")]
    margin: Decimal,
    #[column(name="Площадка")]
    _8: SkipCell,
    #[column(name="Примечание")]
    _9: SkipCell,
    #[column(name="Промежуточный клиринг (FORTS)")]
    _10: SkipCell,
}

impl TableReader for CashFlowRow {
    fn skip_row(row: &[&Cell]) -> GenericResult<bool> {
        Ok(
            xls::get_string_cell(row[0])?.starts_with("Итого по валюте ") ||
            xls::get_string_cell(row[1])? == "Итого:"
        )
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
    _11: SkipCell,
    #[column(name="Место хранения")]
    _12: SkipCell,
    #[column(name="Эмитент")]
    _13: SkipCell,
}

impl TableReader for AssetsRow {
    fn skip_row(row: &[&Cell]) -> GenericResult<bool> {
        Ok(xls::get_string_cell(row[0])? == "Итого:")
    }
}