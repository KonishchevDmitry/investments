use xls_table_derive::XlsTableRow;

use crate::broker_statement::xls::{XlsStatementParser, SectionParser};
use crate::core::EmptyResult;
use crate::xls::{self, SheetReader, Cell, SkipCell, TableReader};

use super::common::read_next_table_row;

pub struct SecuritiesInfoParser {
}

impl SectionParser for SecuritiesInfoParser {
    fn parse(&mut self, parser: &mut XlsStatementParser) -> EmptyResult {
        for security in xls::read_table::<SecuritiesInfoRow>(&mut parser.sheet)? {
            parser.statement.instrument_names.insert(security.symbol, security.name);
        }

        Ok(())
    }
}

#[derive(XlsTableRow)]
struct SecuritiesInfoRow {
    #[column(name="Сокращенное наименование актива")]
    name: String,
    #[column(name="Код актива")]
    symbol: String,
    #[column(name="ISIN")]
    _2: SkipCell,
    #[column(name="Номер гос.регистрации")]
    _3: SkipCell,
    #[column(name="Наименование эмитента")]
    _4: SkipCell,
    #[column(name="Тип")]
    _5: SkipCell,
    #[column(name="Номинал")]
    _6: SkipCell,
    #[column(name="Валюта номинала")]
    _7: SkipCell,
}

impl TableReader for SecuritiesInfoRow {
    fn next_row(sheet: &mut SheetReader) -> Option<&[Cell]> {
        read_next_table_row(sheet)
    }
}