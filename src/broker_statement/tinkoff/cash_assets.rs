use std::cmp::Ordering;

use num_traits::Zero;

use xls_table_derive::XlsTableRow;

use crate::broker_statement::partial::PartialBrokerStatement;
use crate::broker_statement::xls::{XlsStatementParser, SectionParser};
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, CashAssets};
use crate::types::{Date, Time};
use crate::util::DecimalRestrictions;
use crate::xls::{self, SheetReader, Cell, SkipCell, TableReader};

use super::common::{parse_date, parse_decimal, parse_cash, read_next_table_row};
use crate::broker_statement::tinkoff::common::parse_time;

pub struct CashAssetsParser {
}

impl SectionParser for CashAssetsParser {
    fn parse(&mut self, parser: &mut XlsStatementParser) -> EmptyResult {
        parse_current_assets(parser)?;
        parse_cash_flows(parser)?;
        Ok(())
    }
}

#[derive(XlsTableRow)]
struct AssetsRow {
    #[column(name="Валюта")]
    currency: String,
    #[column(name="Входящий остаток на начало периода:")]
    starting: String,
    #[column(name="Исходящий остаток на конец периода:")]
    ending: String,
    #[column(name="Плановый исходящий остаток на конец периода (с учетом неисполненных на дату отчета сделок):")]
    planned: String,
    #[column(name="Задолженность клиента перед брокером:")]
    debt: String,
    #[column(name="Сумма непокрытого остатка:")]
    uncovered: String,
    #[column(name="Задолженность клиента перед Депозитарием (справочно)")]
    _6: SkipCell,
}

impl TableReader for AssetsRow {
    fn next_row(sheet: &mut SheetReader) -> Option<&[Cell]> {
        read_next_table_row(sheet)
    }
}

fn parse_current_assets(parser: &mut XlsStatementParser) -> EmptyResult {
    parser.statement.starting_assets.get_or_insert(false);

    for assets in &xls::read_table::<AssetsRow>(&mut parser.sheet)? {
        let starting = parse_decimal(&assets.starting, DecimalRestrictions::No)?;
        if !starting.is_zero() {
            parser.statement.starting_assets.replace(true);
        }

        let ending = parse_cash(&assets.currency, &assets.ending, DecimalRestrictions::No)?;
        parser.statement.cash_assets.deposit(ending);

        let planned = parse_cash(&assets.currency, &assets.planned, DecimalRestrictions::No)?;
        if planned != ending {
            return Err!("Planned ending cash is not supported yet")
        }

        let debt = parse_decimal(&assets.debt, DecimalRestrictions::No)?;
        if !debt.is_zero() {
            return Err!("Debt is not supported yet");
        }

        let uncovered = parse_decimal(&assets.uncovered, DecimalRestrictions::No)?;
        if !uncovered.is_zero() {
            return Err!("Leverage is not supported yet");
        }
    }

    Ok(())
}

#[derive(XlsTableRow)]
struct CashFlowRow {
    #[column(name="Дата")]
    date: Option<String>,
    #[column(name="Время совершения")]
    time: Option<String>,
    #[column(name="Дата исполнения")]
    execution_date: String,
    #[column(name="Операция")]
    operation: String,
    #[column(name="Сумма зачисления")]
    deposit: String,
    #[column(name="Сумма списания")]
    withdrawal: String,
    #[column(name="Примечание")]
    _6: SkipCell,
}

impl TableReader for CashFlowRow {
    fn next_row(sheet: &mut SheetReader) -> Option<&[Cell]> {
        read_next_table_row(sheet)
    }
}

fn parse_cash_flows(parser: &mut XlsStatementParser) -> EmptyResult {
    let mut cash_flows = Vec::new();

    struct CashFlow {
        date: Date,
        time: Option<Time>,
        currency: &'static str,
        info: CashFlowRow,
    }

    loop {
        let row = xls::strip_row_expecting_columns(&parser.sheet.next_row_checked()?, 1)?;
        let title = xls::get_string_cell(&row[0])?;

        let currency = match parser.statement.cash_assets.get(title) {
            Some(assets) => assets.currency,
            None => {
                parser.sheet.step_back();
                break;
            }
        };

        for cash_flow in xls::read_table::<CashFlowRow>(&mut parser.sheet)? {
            let (date, time) = match cash_flow.date.as_ref() {
                Some(date) => {
                    let date = parse_date(&date)?;
                    let time = cash_flow.time.as_ref().map(|time| parse_time(&time)).transpose()?;
                    (date, time)
                },
                None => (parse_date(&cash_flow.execution_date)?, None),
            };

            cash_flows.push(CashFlow {
                date, time,
                currency: currency,
                info: cash_flow,
            });
        }
    }

    cash_flows.sort_by(|a, b| {
        a.date.cmp(&b.date).then_with(|| {
            match (a.time, b.time) {
                (Some(a), Some(b)) => a.cmp(&b),
                _ => Ordering::Equal,
            }
        })
    });

    for CashFlow {date, currency, info: cash_flow, ..} in cash_flows {
        parse_cash_flow(&mut parser.statement, date, currency, &cash_flow)?;
    }

    Ok(())
}

fn parse_cash_flow(
    statement: &mut PartialBrokerStatement, date: Date, currency: &str, cash_flow: &CashFlowRow
) -> EmptyResult {
    let operation = &cash_flow.operation;
    let deposit = parse_cash(currency, &cash_flow.deposit, DecimalRestrictions::PositiveOrZero)?;
    let withdrawal = parse_cash(currency, &cash_flow.withdrawal, DecimalRestrictions::PositiveOrZero)?;

    let check_amount = |amount: Cash| -> GenericResult<Cash> {
        if amount.is_zero() || !matches!((deposit.is_zero(), withdrawal.is_zero()), (true, false) | (false, true)) {
            return Err!(
                "Got an unexpected deposit and withdrawal amounts for {} operation: {} and {}",
                operation, deposit, withdrawal);
        }

        Ok(amount)
    };

    match operation.as_str() {
        "Пополнение счета" => statement.cash_flows.push(
            CashAssets::new_from_cash(date, check_amount(deposit)?)),
        "Вывод средств" => statement.cash_flows.push(
            CashAssets::new_from_cash(date, -check_amount(withdrawal)?)),
        "Покупка/продажа" | "Комиссия за сделки" | "Комиссия по тарифу" => {},
        _ => {
            if cfg!(debug_assertions) {
                return Err!("Unsupported cash flow operation: {:?}", operation)
            }
        },
    };

    Ok(())
}