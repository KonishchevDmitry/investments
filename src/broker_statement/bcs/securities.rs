use std::cmp::Ordering;
use std::collections::HashSet;

use crate::broker_statement::corporate_actions::{CorporateAction, CorporateActionType, StockSplitRatio};
use crate::broker_statement::partial::{PartialBrokerStatement, PartialBrokerStatementRc};
use crate::core::{EmptyResult, GenericResult};
use crate::formats::xls::{self, XlsTableRow, XlsStatementParser, SheetReader, SectionParser, TableReader, Cell};
use crate::formatting;
use crate::time::Date;
use crate::types::Decimal;
use crate::util::{self, DecimalRestrictions};

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

        let securities = xls::read_table::<SecurityRow>(&mut parser.sheet)?;
        let blocked: HashSet<String> = securities.iter()
            .filter(|security| security.depositary.contains("Блокированный раздел"))
            .map(|security| security.symbol.clone())
            .collect();

        for security in &securities {
            let comment = security.comment.as_deref().unwrap_or_default().trim();
            if comment.is_empty() {
                continue;
            }

            if comment.starts_with("Конвертация паи") {
                security.parse_split(&mut statement)?;
            } else if comment == "Прочее" && blocked.contains(&security.symbol) {
                // Assume operations on blocked securities at OTC market
            } else {
                return Err!("Unsupported corporate action: {:?}", security.comment);
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
    start_quantity: Option<Decimal>,
    #[column(name="Приход", optional=true)]
    deposit: Option<Decimal>,
    #[column(name="Расход", optional=true)]
    withdrawal: Option<Decimal>,
    #[column(name="Остаток на конец дня / конец операции", optional=true)]
    end_quantity: Option<Decimal>,

    #[column(name="Место хранения")]
    depositary: String,
    #[column(name="Примечание", optional=true)]
    comment: Option<String>,
}

impl TableReader for SecurityRow {
    fn next_row<'a>(sheet: &'a mut SheetReader) -> Option<&'a [Cell]> {
        let (first_sheet, second_sheet) = unsafe {
            // Fighting with borrow checker
            let sheet = sheet as *mut SheetReader;
            (&mut *sheet as &'a mut SheetReader, &mut *sheet as &'a mut SheetReader)
        };

        let first_row = first_sheet.next_row()?;
        if !xls::is_empty_row(first_row) {
            return Some(first_row);
        }

        let second_row = second_sheet.next_row()?;
        if xls::is_empty_row(second_row) {
            return None;
        }

        Some(second_row)
    }
}

impl SecurityRow {
    fn parse_split(&self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        let action = self.get_stock_split()?.ok_or_else(|| format!(
            "Unsupported stock split: {} at {}", self.symbol, formatting::format_date(self.date)))?;

        let symbol = parse_symbol(self.symbol.trim_end())?;
        statement.corporate_actions.push(CorporateAction {
            time: self.date.into(),
            report_date: None,
            symbol, action,
        });

        Ok(())
    }

    fn get_stock_split(&self) -> GenericResult<Option<CorporateActionType>> {
        let (withdrawal, deposit) = match (self.start_quantity, self.withdrawal, self.deposit, self.end_quantity) {
            (Some(start), Some(withdrawal), Some(deposit), Some(end)) if withdrawal == start && deposit == end => (
                util::validate_named_decimal("withdrawal value", withdrawal, DecimalRestrictions::StrictlyPositive)?,
                util::validate_named_decimal("deposit value", deposit, DecimalRestrictions::StrictlyPositive)?,
            ),
            _ => return Ok(None),
        };

        let calc_ratio = |bigger: Decimal, smaller: Decimal| -> Option<u32> {
            if bigger % smaller != dec!(0) {
                return None;
            }
            u32::try_from(bigger / smaller).ok()
        };

        let ratio = match withdrawal.cmp(&deposit) {
            Ordering::Equal => return Ok(None),

            Ordering::Less => {
                let Some(to) = calc_ratio(deposit, withdrawal) else {
                    return Ok(None);
                };
                StockSplitRatio::new(1, to)
            },

            Ordering::Greater => {
                let Some(from) = calc_ratio(withdrawal, deposit) else {
                    return Ok(None);
                };
                StockSplitRatio::new(from, 1)
            },
        };

        Ok(Some(CorporateActionType::StockSplit{
            ratio,
            withdrawal: Some(withdrawal),
            deposit: Some(deposit),
        }))
    }
}