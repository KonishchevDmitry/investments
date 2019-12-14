use log::trace;

use crate::core::{EmptyResult, GenericResult};
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::brokers::BrokerInfo;
use crate::xls::{SheetReader, Cell};

use super::parsers::{PeriodParser, AssetsParser};

pub struct Parser {
    pub statement: PartialBrokerStatement,
    pub sheet: SheetReader,
}

impl Parser {
    pub fn read(broker_info: BrokerInfo, path: &str) -> GenericResult<PartialBrokerStatement> {
        Parser {
            statement: PartialBrokerStatement::new(broker_info),
            sheet: SheetReader::new(path, "TDSheet")?,
        }.parse()
    }

    fn parse(mut self) -> GenericResult<PartialBrokerStatement> {
        // FIXME: We need a better way here
        let mut sections = SectionState::new(vec![
            Section::new_required("Период:", Box::new(PeriodParser{})),

            Section::new_anchor_required("1. Движение денежных средств"),
            Section::new_anchor_required("1.1. Движение денежных средств по совершенным сделкам:"),
            Section::new_anchor_required("1.1.1. Движение денежных средств по совершенным сделкам (иным операциям) с ценными бумагами, по срочным сделкам, а также сделкам с иностранной валютой:"),
            Section::new_anchor_required("Остаток денежных средств на начало периода (Рубль):"),
            Section::new_anchor_required("Остаток денежных средств на конец периода (Рубль):"),

            Section::new_required("3. Активы:", Box::new(AssetsParser{})),
        ]);

        while let Some(row) = self.sheet.next_row() {
            let section = match sections.match_section(row)? {
                Some(section) => section,
                None => continue,
            };

            trace!("Got {:?} section.", section.title);

            if let Some(parser) = section.parser.as_ref() {
                if !parser.consume_title() {
                    self.sheet.step_back();
                }

                // FIXME: Wrap errors
                parser.parse(&mut self)?;
            }
        }

        sections.validate()?;
        self.statement.validate()
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

    fn match_section(&mut self, row: &[Cell]) -> GenericResult<Option<&Section>> {
        if row.is_empty() {
            return Ok(None);
        }

        let cell_value = match row[0] {
            // FIXME: Check for error cell?
            Cell::String(ref value) => value,
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