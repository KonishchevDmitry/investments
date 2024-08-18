// XXX(konishchev): Rewrite
use std::cell::RefCell;
use std::fs::File;
use std::io::{self, Read, Write};
use std::ops::Range;
use std::rc::Rc;

use log::trace;
use scraper::{ElementRef, Html, Selector};

use crate::core::{EmptyResult, GenericResult};
use crate::formats::html::util;

pub struct HtmlStatementParser<'a> {
    body: ElementRef<'a>,
}

impl<'a> HtmlStatementParser<'a> {
    pub fn read(path: &str, sections: Vec<Section>) -> EmptyResult {
        let mut data = String::new();
        File::open(path)?.read_to_string(&mut data)?;

        let document = Html::parse_document(&data);
        let body = util::select_one(document.root_element(), "html body")?;


        let mut parser = HtmlStatementParser {body};

        parser.parse(sections)
        // if let Err(e) = parser.parse(sections) {
        //     return Err(parser.sheet.detalize_error(&e.to_string()).into());
        // }

        // Ok(())
    }

    fn parse(&mut self, sections: Vec<Section>) -> EmptyResult {
        let mut sections = SectionState::new(sections);
        let mut elements = self.body.child_elements();

        while let Some(mut element) = elements.next() {
            let section = match sections.match_section(element)? {
                Some(section) => section,
                None => continue,
            };

            trace!("Got {:?} section.", section.title);

            if let Some(parser) = section.parser.as_ref() {
                let mut parser = parser.as_ref().borrow_mut();

                // if !parser.consume_title() {
                //     self.sheet.step_back();
                // }

                match parser.section_type() {
                    SectionType::Simple => {},
                    SectionType::Table => {
                        element = elements.next().expect("BOOM");
                        if element.value().name() != "table" {
                            return Err!("Unexpected: {}", element.html());
                        }
                    },
                }

                parser.parse(element)?;
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

pub enum SectionType {
    Simple,
    Table,
}

pub trait SectionParser {
    fn section_type(&self) -> SectionType { SectionType::Table }
    fn parse(&mut self, element: ElementRef) -> EmptyResult;
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

    pub fn match_section(&mut self, row: ElementRef) -> GenericResult<Option<&mut Section>> {
        let cell_value = row.text().fold(String::new(), |acc, x| acc + x);
        let cell_value = cell_value.trim();
        // trace!(">>> {cell_value:?}");

        let start_from = self.start_from();
        let current_id = match self.sections[start_from..].iter().position(|section| {
            section.matches.iter().any(|title| {
                if section.by_prefix {
                    cell_value.starts_with(title)
                } else {
                    *title == cell_value
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