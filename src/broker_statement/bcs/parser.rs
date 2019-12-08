use std::ops::Index;

use calamine::{Xls, Reader, Range, DataType, open_workbook};
use log::trace;

use crate::core::{EmptyResult, GenericResult};
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::brokers::BrokerInfo;

use super::parsers::{PeriodParser, AssetsParser};

pub struct Parser {
    pub statement: PartialBrokerStatement,
    sheet: Range<DataType>,
    next_row_id: usize,
}

impl Parser {
    pub fn read(broker_info: BrokerInfo, path: &str) -> GenericResult<PartialBrokerStatement> {
        let mut workbook: Xls<_> = open_workbook(path)?;

        let sheet_name = "TDSheet";
        let sheet = workbook.worksheet_range(sheet_name).ok_or_else(|| format!(
            "The statement doesn't contain sheet with {:?} name", sheet_name))??;

        Parser {
            statement: PartialBrokerStatement::new(broker_info),
            sheet,
            next_row_id: 0,
        }.parse()
    }

    fn parse(mut self) -> GenericResult<PartialBrokerStatement> {
        let mut sections = SectionState::new(vec![
            Section::new_required("Период:", Box::new(PeriodParser{})),

            // FIXME
            Section::new_anchor_required("1. Движение денежных средств"),
            Section::new_anchor_required("1.1. Движение денежных средств по совершенным сделкам:"),
            Section::new_anchor_required("1.1.1. Движение денежных средств по совершенным сделкам (иным операциям) с ценными бумагами, по срочным сделкам, а также сделкам с иностранной валютой:"),
            Section::new_anchor_required("Остаток денежных средств на начало периода (Рубль):"),
            Section::new_anchor_required("Остаток денежных средств на конец периода (Рубль):"),

            Section::new_required("3. Активы:", Box::new(AssetsParser{})),
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

            if let Some(parser) = section.parser.as_ref() {
                if !parser.consume_title() {
                    self.next_row_id -= 1;
                }

                // FIXME: Wrap errors
                parser.parse(&mut self)?;
            }
        }

        sections.validate()?;
        self.statement.validate()
    }

    // FIXME: Check for error cells?
    fn next_row(&mut self) -> Option<&[DataType]> {
        if self.next_row_id >= self.sheet.height() {
            return None;
        }

        let row = self.sheet.index(self.next_row_id);
        self.next_row_id += 1;

        Some(row)
    }

    pub fn next_row_checked(&mut self) -> GenericResult<&[DataType]> {
        Ok(self.next_row().ok_or_else(|| "Got an unexpected end of sheet")?)
    }

    pub fn skip_empty_rows(&mut self) {
        loop {
            let row = match self.next_row() {
                Some(row) => row,
                None => break,
            };

            let empty = row.iter().all(|cell| {
                if let DataType::Empty = cell {
                    true
                } else {
                    false
                }
            });

            if !empty {
                self.next_row_id -= 1;
                break;
            }
        }
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
    parser: Option<Box<dyn SectionParser>>,
    required: bool,
    seen: bool,
}

impl Section {
    #[allow(dead_code)] // FIXME
    fn new(title: &'static str, parser: Box<dyn SectionParser>) -> Section {
        Section::new_full(title, Some(parser), false)
    }

    fn new_required(title: &'static str, parser: Box<dyn SectionParser>) -> Section {
        Section::new_full(title, Some(parser), true)
    }

    fn new_anchor_required(title: &'static str) -> Section {
        Section::new_full(title, None, true)
    }

    fn new_full(title: &'static str, parser: Option<Box<dyn SectionParser>>, required: bool) -> Section {
        Section { title, parser, required, seen: false }
    }
}

pub trait SectionParser {
    fn consume_title(&self) -> bool { true }
    fn parse(&self, parser: &mut Parser) -> EmptyResult;
}