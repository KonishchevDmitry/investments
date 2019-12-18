use std::ops::Range;

use crate::core::{EmptyResult, GenericResult};
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::brokers::BrokerInfo;
use crate::xls::{SheetReader, Cell};

use super::assets::AssetsParser;
use super::cash_flow::CashFlowParser;
use super::period::PeriodParser;
use super::trades::TradesParser;

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
        let mut sections = SectionState::new(vec![
            Section::new("Период:").parser(Box::new(PeriodParser{})).required(),

            Section::new("1. Движение денежных средств").required(),
            Section::new("1.1. Движение денежных средств по совершенным сделкам:").required(),
            Section::new(concat!(
                "1.1.1. Движение денежных средств по совершенным сделкам (иным операциям) с ",
                "ценными бумагами, по срочным сделкам, а также сделкам с иностранной валютой:",
            )).required(),
            Section::new("Остаток денежных средств на начало периода (Рубль):").required(),
            Section::new("Остаток денежных средств на конец периода (Рубль):").required(),
            Section::new("Рубль").parser(Box::new(CashFlowParser{})).required(),

            Section::new("2.1. Сделки:"),
            Section::new("Пай").parser(Box::new(TradesParser{})),
            Section::new("2.3. Незавершенные сделки"),

            Section::new("3. Активы:").required(),
            Section::new("Вид актива").parser(Box::new(AssetsParser{})).required(),
        ]);

        while let Some(row) = self.sheet.next_row() {
            let section = match sections.match_section(row)? {
                Some(section) => section,
                None => continue,
            };

            if let Some(parser) = section.parser.as_ref() {
                if !parser.consume_title() {
                    self.sheet.step_back();
                }

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
            Cell::String(ref value) => value,
            _ => return Ok(None),
        };

        let start_from = self.start_from();
        let current_id = match self.sections[start_from..].iter().position(|section| {
            section.title == cell_value
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

    fn validate(&self) -> EmptyResult {
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

struct Section {
    title: &'static str,
    parser: Option<Box<dyn SectionParser>>,
    required: bool,
}

impl Section {
    fn new(title: &'static str) -> Section {
        Section {
            title,
            parser: None,
            required: false,
        }
    }

    fn required(mut self) -> Section {
        self.required = true;
        self
    }

    fn parser(mut self, parser: Box<dyn SectionParser>) -> Section {
        self.parser = Some(parser);
        self
    }
}

pub trait SectionParser {
    fn consume_title(&self) -> bool { true }
    fn parse(&self, parser: &mut Parser) -> EmptyResult;
}