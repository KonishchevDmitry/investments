use num_traits::Zero;

use xls_table_derive::XlsTableRow;

use crate::broker_statement::xls::{XlsStatementParser, SectionParser};
use crate::core::EmptyResult;
use crate::util::DecimalRestrictions;
use crate::xls::{self, SheetReader, Cell, SkipCell, TableReader};

use super::common::{parse_decimal, parse_cash};

pub struct CashFlowParser {
}

impl SectionParser for CashFlowParser {
    fn parse(&mut self, parser: &mut XlsStatementParser) -> EmptyResult {
        let mut has_starting_assets = false;

        for assets in &xls::read_table::<AssetsRow>(&mut parser.sheet)? {
            let starting = parse_decimal(&assets.starting, DecimalRestrictions::No)?;
            has_starting_assets |= !starting.is_zero();

            let ending = parse_cash(&assets.currency, &assets.ending, DecimalRestrictions::No)?;
            parser.statement.cash_assets.deposit(ending);

            // FIXME(konishchev): Support or skip?
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

        // FIXME(konishchev): Take stocks into account
        parser.statement.set_starting_assets(has_starting_assets)?;

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
        sheet.next_row().and_then(|row| {
            let non_empty_cells = row.iter().filter(|cell| !matches!(cell, Cell::Empty)).count();
            if non_empty_cells > 1 {
                Some(row)
            } else {
                None
            }
        })
    }
}