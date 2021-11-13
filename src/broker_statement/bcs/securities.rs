use crate::broker_statement::partial::{PartialBrokerStatement, PartialBrokerStatementRc};
use crate::core::EmptyResult;
use crate::formatting;
use crate::time::Date;
use crate::xls::{self, XlsStatementParser, SheetReader, SectionParser, TableReader, Cell, SkipCell};

use xls_table_derive::XlsTableRow;

use super::common::{parse_symbol, parse_short_date_cell};

pub struct SecuritiesParser {
    statement: PartialBrokerStatementRc,
}

impl SecuritiesParser {
    pub fn new(statement: PartialBrokerStatementRc) -> Box<dyn SectionParser> {
        Box::new(SecuritiesParser {statement})
    }
}

impl SectionParser for SecuritiesParser {
    fn parse(&mut self, parser: &mut XlsStatementParser) -> EmptyResult {
        let mut statement = self.statement.borrow_mut();
        parser.sheet.skip_empty_rows();

        for asset in &xls::read_table::<SecurityRow>(&mut parser.sheet)? {
            if matches!(&asset.comment, Some(d) if d.trim_end() == "Конвертация паи") {
                asset.parse_split(&mut statement)?;
            }
        }

        Ok(())
    }
}

#[derive(XlsTableRow)]
struct SecurityRow {
    #[column(name="ЦБ")]
    symbol: String,
    #[column(name="Дата", parse_with="parse_short_date_cell")]
    date: Date,

    #[column(name="Остаток на начало дня / начало операции", optional=true)]
    start_quantity: Option<u32>,
    #[column(name="Приход", optional=true)]
    credit: Option<u32>,
    #[column(name="Расход", optional=true)]
    debit: Option<u32>,
    #[column(name="Остаток на конец дня / конец операции", optional=true)]
    end_quantity: Option<u32>,

    #[column(name="Место хранения")]
    _6: SkipCell,
    #[column(name="Примечание", optional=true)]
    comment: Option<String>,
}

impl TableReader for SecurityRow {
    fn next_row<'a>(sheet: &'a mut SheetReader) -> Option<&[Cell]> {
        let (first_sheet, second_sheet) = unsafe {
            // Fighting with borrow checker
            let sheet = sheet as *mut SheetReader;
            (&mut *sheet as &'a mut SheetReader, &mut *sheet as &'a mut SheetReader)
        };

        let first_row = match first_sheet.next_row() {
            Some(row) => row,
            None => return None,
        };

        if !xls::is_empty_row(first_row) {
            return Some(first_row);
        }

        let second_row = match second_sheet.next_row() {
            Some(row) => row,
            None => return None,
        };

        if xls::is_empty_row(second_row) {
            return None;
        }

        Some(second_row)
    }
}

impl SecurityRow {
    fn parse_split(&self, _statement: &mut PartialBrokerStatement) -> EmptyResult {
        parse_symbol(self.symbol.trim_end())?;

        match (self.start_quantity, self.debit, self.credit, self.end_quantity) {
            (Some(start), Some(debit), Some(credit), Some(end)) if debit == start && credit == end => {},
            _ => return Err!("Unsupported stock split: {} at {}", self.symbol, formatting::format_date(self.date)),
        }

        Ok(())
    }
}