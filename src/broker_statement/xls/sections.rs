use std::ops::Range;

use crate::core::{EmptyResult, GenericResult};
use crate::xls::Cell;

use super::XlsStatementParser;

pub struct Section {
    title: &'static str,
    pub parser: Option<Box<dyn SectionParser>>,
    by_prefix: bool,
    required: bool,
}

impl Section {
    pub fn new(title: &'static str) -> Section {
        Section {
            title,
            by_prefix: false,
            parser: None,
            required: false,
        }
    }

    pub fn by_prefix(mut self) -> Section {
        self.by_prefix = true;
        self
    }

    pub fn required(mut self) -> Section {
        self.required = true;
        self
    }

    pub fn parser(mut self, parser: Box<dyn SectionParser>) -> Section {
        self.parser = Some(parser);
        self
    }
}

pub trait SectionParser {
    fn consume_title(&self) -> bool { true }
    fn parse(&self, parser: &mut XlsStatementParser) -> EmptyResult;
}

pub struct SectionState {
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

    pub fn match_section(&mut self, row: &[Cell]) -> GenericResult<Option<&Section>> {
        if row.is_empty() {
            return Ok(None);
        }

        let cell_value = match row[0] {
            Cell::String(ref value) => value,
            _ => return Ok(None),
        };

        let start_from = self.start_from();
        let current_id = match self.sections[start_from..].iter().position(|section| {
            if section.by_prefix {
                cell_value.starts_with(section.title)
            } else {
                section.title == cell_value
            }
        }) {
            Some(index) => start_from + index,
            None => return Ok(None),
        };

        self.validate_missing_sections(start_from..current_id)?;
        self.last_id.replace(current_id);

        Ok(Some(&self.sections[current_id]))
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