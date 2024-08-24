use scraper::ElementRef;

use crate::broker_statement::partial::{PartialBrokerStatement, PartialBrokerStatementRc};
use crate::core::EmptyResult;
use crate::formats::html::{self, HtmlTableRow, SectionParser, SkipCell};
use crate::instruments::parse_isin;

use super::common::trim_column_title;

pub struct SecuritiesInfoParser {
    statement: PartialBrokerStatementRc,
}

impl SecuritiesInfoParser {
    pub fn new(statement: PartialBrokerStatementRc) -> Box<dyn SectionParser> {
        Box::new(SecuritiesInfoParser {statement})
    }
}

impl SectionParser for SecuritiesInfoParser {
    fn parse(&mut self, table: ElementRef) -> EmptyResult {
        let mut statement = self.statement.borrow_mut();

        for row in html::read_table::<SecuritiesInfoRow>(table)? {
            row.parse(&mut statement)?;
        }

        Ok(())
    }
}

#[derive(HtmlTableRow)]
#[table(trim_column_title_with="trim_column_title")]
struct SecuritiesInfoRow {
    #[column(name="Наименование")]
    name: String,
    #[column(name="Код")]
    symbol: String,
    #[column(name="ISIN ценной бумаги")]
    isin: String,
    #[column(name="Эмитент")]
    _3: SkipCell,
    #[column(name="Вид, Категория, Тип, иная информация")]
    _4: SkipCell,
    #[column(name="Выпуск, Транш, Серия")]
    _5: SkipCell,
}

impl SecuritiesInfoRow {
    fn parse<'a>(&'a self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        let instrument = statement.instrument_info.add(&self.symbol)?;
        instrument.set_name(&self.name);
        instrument.add_isin(parse_isin(&self.isin)?);
        Ok(())
    }
}