use xls_table_derive::XlsTableRow;

use crate::broker_statement::xls::{XlsStatementParser, SectionParser};
use crate::core::EmptyResult;
use crate::xls::{self, SheetReader, Cell, SkipCell, TableReader};

use super::common::{read_next_table_row, parse_quantity_cell};

pub struct AssetsParser {
}

impl SectionParser for AssetsParser {
    fn parse(&mut self, parser: &mut XlsStatementParser) -> EmptyResult {
        for asset in &xls::read_table::<AssetsRow>(&mut parser.sheet)? {
            let symbol = &asset.symbol;

            if asset.starting != 0 {
                parser.statement.starting_assets.replace(true);
            }

            if asset.planned != 0 {
                parser.statement.add_open_position(symbol, asset.planned.into())?;
            }
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
    #[column(name="Входящий остаток", parse_with="parse_quantity_cell")]
    starting: u32,
    #[column(name="Зачисление")]
    _4: SkipCell,
    #[column(name="Списание")]
    _5: SkipCell,
    #[column(name="Исходящий остаток")]
    _6: SkipCell,
    #[column(name="Плановый исходящий остаток", parse_with="parse_quantity_cell")]
    planned: u32,
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