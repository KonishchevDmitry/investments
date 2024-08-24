use std::cmp::Ordering;

use crate::broker_statement::corporate_actions::{CorporateAction, CorporateActionType, StockSplitRatio};
use crate::broker_statement::partial::{PartialBrokerStatement, PartialBrokerStatementRc};
use crate::core::EmptyResult;
use crate::formats::xls::{self, XlsTableRow, XlsStatementParser, SheetReader, SectionParser, TableReader, Cell, SkipCell};
use crate::formatting;
use crate::time::Date;

use super::common::{parse_symbol, parse_short_date_cell, trim_column_title};

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
            let comment = asset.comment.as_deref().unwrap_or_default().trim();
            if comment.is_empty() {
                continue;
            }

            if comment.starts_with("Конвертация паи") {
                asset.parse_split(&mut statement)?;
            } else {
                return Err!("Unsupported corporate action: {:?}", asset.comment);
            }
        }

        Ok(())
    }
}

#[derive(XlsTableRow)]
#[table(trim_column_title="trim_column_title")]
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
    fn parse_split(&self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        let action = self.get_stock_split().ok_or_else(|| format!(
            "Unsupported stock split: {} at {}", self.symbol, formatting::format_date(self.date)))?;

        let symbol = parse_symbol(self.symbol.trim_end())?;
        statement.corporate_actions.push(CorporateAction {
            time: self.date.into(),
            report_date: None,
            symbol, action,
        });

        Ok(())
    }

    fn get_stock_split(&self) -> Option<CorporateActionType> {
        let (debit, credit) = match (self.start_quantity, self.debit, self.credit, self.end_quantity) {
            (Some(start), Some(debit), Some(credit), Some(end)) if debit == start && credit == end => (debit, credit),
            _ => return None,
        };

        let ratio = match debit.cmp(&credit) {
            Ordering::Equal => return None,

            Ordering::Less => {
                if credit % debit != 0 {
                    return None;
                }
                StockSplitRatio::new(1, credit / debit)
            },

            Ordering::Greater => {
                if debit % credit != 0 {
                    return None;
                }
                StockSplitRatio::new(debit / credit, 1)
            },
        };

        Some(CorporateActionType::StockSplit{
            ratio,
            from_change: Some(debit.into()),
            to_change: Some(credit.into()),
        })
    }
}