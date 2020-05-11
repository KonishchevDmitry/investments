mod sections;

use crate::core::GenericResult;
use crate::broker_statement::partial::PartialBrokerStatement;
use crate::brokers::BrokerInfo;
use crate::xls::SheetReader;

use self::sections::SectionState;
pub use self::sections::{Section, SectionParser};

pub struct XlsStatementParser {
    pub statement: PartialBrokerStatement,
    pub sheet: SheetReader,
}

impl XlsStatementParser {
    pub fn read(
        broker_info: BrokerInfo, path: &str, sheet_name: &str, sections: Vec<Section>,
    ) -> GenericResult<PartialBrokerStatement> {
        XlsStatementParser {
            statement: PartialBrokerStatement::new(broker_info),
            sheet: SheetReader::new(path, sheet_name)?,
        }.parse(sections)
    }

    fn parse(mut self, sections: Vec<Section>) -> GenericResult<PartialBrokerStatement> {
        let mut sections = SectionState::new(sections);

        while let Some(row) = self.sheet.next_row() {
            let section = match sections.match_section(row)? {
                Some(section) => section,
                None => continue,
            };

            if let Some(parser) = section.parser.as_mut() {
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