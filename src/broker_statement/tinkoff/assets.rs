use xls_table_derive::XlsTableRow;

use crate::broker_statement::xls::{XlsStatementParser, SectionParser};
use crate::core::EmptyResult;
use crate::xls::{self, SheetReader, Cell, SkipCell, TableReader};

use super::common::read_next_table_row;

pub struct AssetsParser {
}

impl SectionParser for AssetsParser {
    fn parse(&mut self, parser: &mut XlsStatementParser) -> EmptyResult {
        for asset in &xls::read_table::<AssetsRow>(&mut parser.sheet)? {
            let symbol = &asset.symbol;

            let starting: u32 = asset.starting.parse().map_err(|_| format!(
                "Invalid {} starting quantity: {}", symbol, asset.starting))?;

            let planned: u32 = asset.planned.parse().map_err(|_| format!(
                "Invalid {} planned quantity: {}", symbol, asset.planned))?;

            if starting != 0 {
                parser.statement.starting_assets.replace(true);
            }

            parser.statement.add_open_position(symbol, planned.into())?;
        }

        Ok(())
    }
}

#[derive(XlsTableRow)]
struct AssetsRow {
    #[column(name="Сокращенное наименование актива")]
    _0: SkipCell,
    #[column(name="Код актива")]
    symbol: String,
    #[column(name="Место хранения")]
    _2: SkipCell,
    #[column(name="Входящий остаток")]
    starting: String,
    #[column(name="Зачисление")]
    _4: SkipCell,
    #[column(name="Списание")]
    _5: SkipCell,
    #[column(name="Исходящий остаток")]
    _6: SkipCell,
    #[column(name="Плановый исходящий остаток")]
    planned: String,
    #[column(name="Рыночная цена")]
    _8: SkipCell,
    #[column(name="Валюта цены")]
    _9: SkipCell,
    #[column(name="НКД")]
    _10: SkipCell,
    #[column(name="Рыночная стоимость")]
    _11: SkipCell,
}

impl TableReader for AssetsRow {
    fn next_row(sheet: &mut SheetReader) -> Option<&[Cell]> {
        read_next_table_row(sheet)
    }
}