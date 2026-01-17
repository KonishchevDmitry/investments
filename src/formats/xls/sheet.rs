use std::ops::Index;
use std::path::Path;

use calamine::{Range, Reader, open_workbook_auto};

use crate::core::GenericResult;

use super::{Cell, is_empty_row};

pub struct SheetReader {
    sheet: Range<Cell>,
    parser: Box<dyn SheetParser>,

    prev_row_id: Option<usize>,
    next_row_id: usize,
    eof_reached: bool,
}

impl SheetReader {
    pub fn new(sheet: Range<Cell>, parser: Box<dyn SheetParser>) -> SheetReader {
        SheetReader {
            sheet, parser,
            prev_row_id: None,
            next_row_id: 0,
            eof_reached: false,
        }
    }

    pub fn open(path: &Path, parser: Box<dyn SheetParser>) -> GenericResult<SheetReader> {
        Ok(SheetReader::new(open_sheet(path)?, parser))
    }

    pub fn parse_empty_tables(&self) -> bool {
        self.parser.parse_empty_tables()
    }

    pub fn repeatable_table_column_titles(&self) -> bool {
        self.parser.repeatable_table_column_titles()
    }

    pub fn current_human_row_id(&self) -> usize {
        self.next_row_id
    }

    pub fn next_human_row_id(&self) -> usize {
        self.next_row_id + 1
    }

    pub fn next_row(&mut self) -> Option<&[Cell]> {
        while self.next_row_id < self.sheet.height() {
            let row = self.sheet.index(self.next_row_id);
            if self.parser.skip_row(row) {
                self.next_row_id += 1;
                continue;
            }

            self.prev_row_id.replace(self.next_row_id);
            self.next_row_id += 1;
            return Some(row);
        }

        self.eof_reached = true;
        None
    }

    pub fn next_row_checked(&mut self) -> GenericResult<&[Cell]> {
        Ok(self.next_row().ok_or("Got an unexpected end of sheet")?)
    }

    pub fn step_back(&mut self) {
        self.next_row_id = self.prev_row_id.take().unwrap();
        self.eof_reached = false;
    }

    #[allow(dead_code)]
    pub fn skip_empty_rows(&mut self) {
        while let Some(row) = self.next_row() {
            if !is_empty_row(row) {
                self.step_back();
                break;
            }
        }
    }

    pub fn detalize_error(&self, error: &str) -> String {
        if self.next_row_id == 0 || self.eof_reached {
            error.to_owned()
        } else {
            format!("Starting from #{} row: {:?}: {}",
                    self.current_human_row_id(), self.sheet.index(self.next_row_id - 1), error)
        }
    }
}

pub trait SheetParser {
    // In the beginning of 2024 year T-Bank statements became broken: empty tables started to lose random columns.
    // This property can help to workaround such temporary problems.
    fn parse_empty_tables(&self) -> bool {
        true
    }

    fn repeatable_table_column_titles(&self) -> bool {
        false
    }

    fn skip_row(&self, _row: &[Cell]) -> bool {
        false
    }
}

pub fn open_sheet(path: &Path) -> GenericResult<Range<Cell>> {
    let mut workbook = open_workbook_auto(path)?;

    let mut sheets = workbook.worksheets();
    if sheets.len() > 1 {
        return Err!("The workbook has more than one sheet");
    }

    Ok(sheets.pop().ok_or("The workbook is empty")?.1)
}