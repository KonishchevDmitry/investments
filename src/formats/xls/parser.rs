use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;

use log::trace;

use crate::core::{EmptyResult, GenericResult};
use crate::formats::xls::{self, Cell, SheetReader, SheetParser};

pub struct XlsStatementParser {
    pub sheet: SheetReader,
}

impl XlsStatementParser {
    pub fn read(path: &str, parser: Box<dyn SheetParser>, sections: Vec<Section>) -> EmptyResult {
        let mut parser = XlsStatementParser {
            sheet: SheetReader::open(path, parser)?,
        };

        if let Err(e) = parser.parse(sections) {
            return Err(parser.sheet.detalize_error(&e.to_string()).into());
        }

        Ok(())
    }

    fn parse(&mut self, sections: Vec<Section>) -> EmptyResult {
        let mut sections = SectionState::new(sections);

        while let Some(row) = self.sheet.next_row() {
            let section = match sections.match_section(row)? {
                Some(section) => section,
                None => continue,
            };

            trace!("Got {:?} section at #{} row.", section.title, self.sheet.current_human_row_id());

            if let Some(parser) = section.parser.as_ref() {
                let mut parser = parser.as_ref().borrow_mut();

                if !parser.consume_title() {
                    self.sheet.step_back();
                }

                parser.parse(self)?;
            }
        }

        sections.validate()
    }
}

pub struct Section {
    title: &'static str,
    pub parser: Option<SectionParserRc>,
    matches: Vec<&'static str>,
    by_prefix: bool,
    required: bool,
}

impl Section {
    pub fn new(title: &'static str) -> Section {
        Section {
            title,
            parser: None,
            by_prefix: false,
            matches: vec![title],
            required: false,
        }
    }

    pub fn by_prefix(mut self) -> Section {
        self.by_prefix = true;
        self
    }

    pub fn alias(mut self, title: &'static str) -> Section {
        self.matches.push(title);
        self
    }

    pub fn required(mut self) -> Section {
        self.required = true;
        self
    }

    pub fn parser(mut self, parser: Box<dyn SectionParser>) -> Section {
        self.parser = Some(Rc::new(RefCell::new(parser)));
        self
    }

    pub fn parser_rc(mut self, parser: SectionParserRc) -> Section {
        self.parser = Some(parser);
        self
    }
}

pub trait SectionParser {
    fn consume_title(&self) -> bool { true }
    fn parse(&mut self, parser: &mut XlsStatementParser) -> EmptyResult;
}

pub type SectionParserRc = Rc<RefCell<Box<dyn SectionParser>>>;

struct SectionState {
    sections: Vec<Section>,
    last_id: Option<usize>,
}

impl SectionState {
    pub fn new(sections: Vec<Section>) -> SectionState {
        SectionState {
            sections,
            last_id: None,
        }
    }

    pub fn match_section(&mut self, row: &[Cell]) -> GenericResult<Option<&mut Section>> {
        let row = xls::util::trim_row_left(row);

        if row.is_empty() {
            return Ok(None);
        }

        let cell_value = match row[0] {
            Cell::String(ref value) => value,
            _ => return Ok(None),
        };

        let start_from = self.start_from();
        let current_id = match self.sections[start_from..].iter().position(|section| {
            section.matches.iter().any(|title| {
                if section.by_prefix {
                    cell_value.starts_with(title)
                } else {
                    title == cell_value
                }
            })
        }) {
            Some(index) => start_from + index,
            None => return Ok(None),
        };

        self.validate_missing_sections(start_from..current_id)?;
        self.last_id.replace(current_id);

        Ok(Some(&mut self.sections[current_id]))
    }

    fn start_from(&self) -> usize {
        match self.last_id {
            Some(last_id) => last_id + 1,
            None => 0,
        }
    }

    pub fn validate(&self) -> EmptyResult {
        self.validate_missing_sections(self.start_from()..self.sections.len())
    }

    fn validate_missing_sections(&self, range: Range<usize>) -> EmptyResult {
        match self.sections[range].iter().find(|section| {
            section.required
        }) {
            Some(section) => Err!("Unable to find {:?} section", section.title),
            None => Ok(())
        }
    }
}