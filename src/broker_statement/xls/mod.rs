mod sections;

use crate::core::{GenericResult, EmptyResult};
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::xls::{SheetReader, SheetParser};

use self::sections::SectionState;
pub use self::sections::{Section, SectionParser, SectionParserRc};

pub struct XlsStatementParser {
    pub statement: PartialBrokerStatement,
    pub sheet: SheetReader,
}

impl XlsStatementParser {
    pub fn read(
        path: &str, parser: Box<dyn SheetParser>, sections: Vec<Section>,
    ) -> GenericResult<PartialBrokerStatement> {
        let mut parser = XlsStatementParser {
            statement: PartialBrokerStatement::new(true),
            sheet: SheetReader::new(path, parser)?,
        };

        if let Err(e) = parser.parse(sections) {
            return Err(parser.sheet.detalize_error(&e.to_string()).into());
        }

        parser.statement.validate()
    }

    fn parse(&mut self, sections: Vec<Section>) -> EmptyResult {
        let mut sections = SectionState::new(sections);

        while let Some(row) = self.sheet.next_row() {
            let section = match sections.match_section(row)? {
                Some(section) => section,
                None => continue,
            };

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