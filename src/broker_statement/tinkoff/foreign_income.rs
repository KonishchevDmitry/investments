use matches::matches;

use xls_table_derive::XlsTableRow;

use crate::core::{GenericResult, EmptyResult};
use crate::xls::{self, XlsStatementParser, SheetParser, SheetReader, Section, SectionParser,
                 TableReader, Cell, SkipCell};

const SHEET_NAME: &str = "Отчет";
const TITLE_PREFIX: &str = "Отчет о выплате доходов по ценным бумагам иностранных эмитентов";

pub struct ForeignIncomeStatementReader {
}

impl ForeignIncomeStatementReader {
    #[allow(dead_code)] // FIXME(konishchev): Remove
    pub fn is_statement(path: &str) -> GenericResult<bool> {
        if !path.ends_with(".xlsx") {
            return Ok(false);
        }

        let sheet = match xls::open_sheet(path, SHEET_NAME)? {
            Some(sheet) => sheet,
            None => return Ok(false),
        };

        for mut row in sheet.rows() {
            row = xls::trim_row_right(row);
            if row.len() == 1 && matches!(&row[0], Cell::String(value) if value.starts_with(TITLE_PREFIX)) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    #[allow(dead_code)] // FIXME(konishchev): Remove
    fn read(path: &str) -> EmptyResult {
        let parser = Box::new(ForeignIncomeSheetParser {});

        XlsStatementParser::read(path, parser, vec![
            Section::new(TITLE_PREFIX).by_prefix().required()
                .parser(Box::new(ForeignIncomeParser {})),
        ])?;

        Ok(())
    }
}

struct ForeignIncomeSheetParser {
}

impl SheetParser for ForeignIncomeSheetParser {
    fn sheet_name(&self) -> &str {
        SHEET_NAME
    }

    fn repeatable_table_column_titles(&self) -> bool {
        true
    }
}

pub struct ForeignIncomeParser {
}

impl SectionParser for ForeignIncomeParser {
    fn parse(&mut self, parser: &mut XlsStatementParser) -> EmptyResult {
        parser.sheet.next_row_checked()?;

        for _ in xls::read_table::<ForeignIncomeRow>(&mut parser.sheet)? {
            // FIXME(konishchev): Implement
        }
        Ok(())
    }
}

#[derive(XlsTableRow)]
struct ForeignIncomeRow {
    #[column(name="Дата фиксации реестра")]
    _0: SkipCell,
    #[column(name="Дата выплаты")]
    _1: String,
    #[column(name="Тип выплаты*")]
    _2: SkipCell,
    #[column(name="Наименование ценной бумаги")]
    _3: SkipCell,
    #[column(name="ISIN")]
    _4: SkipCell,
    #[column(name="Страна эмитента")]
    _5: SkipCell,
    #[column(name="Количество ценных бумаг")]
    _6: SkipCell,
    #[column(name="Выплата на одну бумагу")]
    _7: SkipCell,
    #[column(name="Комиссия внешних платежных агентов**")]
    _8: SkipCell,
    #[column(name="Сумма налога, удержанного эмитентом")]
    _9: SkipCell,
    #[column(name="Итоговая сумма выплаты")]
    _10: SkipCell,
    #[column(name="Валюта")]
    _11: SkipCell,
}

impl TableReader for ForeignIncomeRow {
    fn next_row(sheet: &mut SheetReader) -> Option<&[Cell]> {
        loop {
            let row = match sheet.next_row() {
                Some(row) => row,
                None => return None,
            };

            if let Some(Cell::String(value)) = row.iter().next() {
                if value.starts_with(TITLE_PREFIX) || value.starts_with("Депонент: ") {
                    continue;
                } else if value.starts_with("*Типы выплат: ") {
                    return None;
                }
            }

            return Some(unsafe {
                // Loop confuses the borrow checker
                std::slice::from_raw_parts(row.as_ptr(), row.len())
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(name => ["dividends-report-example.xlsx"])]
    fn parse_real(name: &str) {
        let path = format!("testdata/tinkoff/{}", name);
        assert!(ForeignIncomeStatementReader::is_statement(&path).unwrap());
        ForeignIncomeStatementReader::read(&path).unwrap();
    }
}