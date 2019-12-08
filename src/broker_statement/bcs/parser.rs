use std::ops::Index;

use calamine::{Xls, Reader, Range, DataType, open_workbook};
use log::trace;

use crate::core::{EmptyResult, GenericResult};

// FIXME
pub fn read_statement(path: &str) -> EmptyResult {
    Parser::read(path)
}

struct Parser {
    sheet: Range<DataType>,
    next_row: usize,
}

impl Parser {
    fn read(path: &str) -> EmptyResult {
        let mut workbook: Xls<_> = open_workbook(path)?;

        let sheet_name = "TDSheet";
        let sheet = workbook.worksheet_range(sheet_name).ok_or_else(|| format!(
            "The statement doesn't contain sheet with {:?} name", sheet_name))??;

        Parser {
            sheet,
            next_row: 0,
        }.parse()
    }

    fn parse(&mut self) -> EmptyResult {
        let mut sections = SectionState::new(vec![
            Section::new_required("Период:"),
        ]);

        loop {
            let row = match self.next_row() {
                Some(row) => row,
                None => break,
            };

            let section = match sections.match_section(row)? {
                Some(section) => section,
                None => continue,
            };

            trace!("Got {:?} section.", section.title);

//            println!("row={:?}, row[0]={:?}", row, row[0]);
        }

        sections.validate()?;

        // FIXME
        unimplemented!();
    }

    // FIXME: Check for error cells?
    fn next_row(&mut self) -> Option<&[DataType]> {
        if self.next_row >= self.sheet.height() {
            return None;
        }

        let row = self.sheet.index(self.next_row);
        self.next_row += 1;

        Some(row)
    }
}

struct SectionState {
    sections: Vec<Section>,
    last_id: Option<usize>,
}

impl SectionState {
    fn new(sections: Vec<Section>) -> SectionState {
        SectionState {
            sections,
            last_id: None,
        }
    }

    fn match_section(&mut self, row: &[DataType]) -> GenericResult<Option<&Section>> {
        if row.is_empty() {
            return Ok(None);
        }

        let cell_value = match row[0] {
            DataType::String(ref value) => value,
            _ => return Ok(None),
        };

        for (section_id, section) in self.sections.iter_mut().enumerate() {
            if section.title != cell_value {
                continue;
            }

            if section.seen {
                return Err!("Got a duplicated {:?} section", section.title);
            }
            section.seen = true;

            match self.last_id {
                Some(last_id) if section_id <= last_id => {
                    return Err!("Got an unexpected {:?} section", section.title);
                }
                _ => {},
            };

            self.last_id.replace(section_id);
            return Ok(Some(section));
        }

        Ok(None)
    }

    fn validate(&self) -> EmptyResult {
        for section in &self.sections {
            if section.required && !section.seen {
                return Err!("Unable to find {:?} section in the statement", section.title);
            }
        }

        Ok(())
    }
}

struct Section {
    title: &'static str,
    required: bool,
    seen: bool,
}

impl Section {
    fn new(title: &'static str) -> Section {
        Section { title, required: false, seen: false }
    }

    fn new_required(title: &'static str) -> Section {
        Section { title, required: true, seen: false }
    }
}