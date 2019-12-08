use calamine::{Xls, Reader, Range, DataType, open_workbook};
use log::trace;

use crate::core::EmptyResult;
use std::ops::Index;

pub fn read_statement(path: &str) -> EmptyResult {
    Parser::read(path)
}

struct Parser {
//    sheet_reader: SheetReader,
    sheet: Range<DataType>,
    current_row: usize,
}

struct Section {
    title: &'static str,
}

impl Parser {
    fn read(path: &str) -> EmptyResult {
        let mut workbook: Xls<_> = open_workbook(path)?;

        let sheet_name = "TDSheet";
        let sheet = workbook.worksheet_range(sheet_name).ok_or_else(|| format!(
            "The statement doesn't contain {:?} sheet", sheet_name))??;

        Parser {
            sheet,
            current_row: 0,
//            sheet_reader: SheetReader {
//                workbook,
//                rows: sheet.rows(),
//            },
        }.parse()
    }

    fn parse(&mut self) -> EmptyResult {
        let sections = &[
            Section{title: "Период:"},
        ];
        let mut last_section_id = None;

        loop {
            let row = match self.next_row() {
                Some(row) => row,
                None => break,
            };

            if row.is_empty() {
                continue;
            }

            match row[0] {
                DataType::String(ref value) => {
                    for (current_id, section) in sections.iter().enumerate() {
                        if value == section.title {
                            if let Some(last_id) = last_section_id {
                                if current_id <= last_id {
                                    return Err!("Got a duplicated {:?} section.", section.title);
                                }
                            }

                            last_section_id.replace(current_id);
                            trace!("Got {:?} section.", section.title);
                        }
                    }
                },
                _ => continue,
            }

//            println!("row={:?}, row[0]={:?}", row, row[0]);
        }

        unimplemented!();
    }

    fn next_row(&mut self) -> Option<&[DataType]> {
        if self.current_row < self.sheet.height() {
            let row = self.sheet.index(self.current_row);
            self.current_row += 1;
            Some(row)
        } else {
            None
        }
    }
}

//struct SheetReader {
//    workbook: Xls<BufReader<File>>,
//    rows: Rows<DataType>,
//}