use broker_statement::ib::IbStatementParser;
use broker_statement::ib::common::{Record, RecordParser};
use core::EmptyResult;

pub struct OpenPositionsParser {}

impl RecordParser for OpenPositionsParser {
    fn skip_data_types(&self) -> Option<&'static [&'static str]> {
        Some(&["Total"])
    }

    fn parse(&self, parser: &mut IbStatementParser, record: &Record) -> EmptyResult {
        for (field, value) in [
            ("DataDiscriminator", "Summary"),
            ("Asset Category", "Stocks"),
            ("Mult", "1"),
        ].iter() {
            if record.get_value(*field)? != *value {
                return Err!("Got an unexpected {:?} field value: {:?}", *field, *value);
            }
        }

        let symbol = record.get_value("Symbol")?;
        let quantity = record.parse_value("Quantity")?;

        if parser.statement.open_positions.insert(symbol.to_owned(), quantity).is_some() {
            return Err!("Got a duplicated {:?} symbol", symbol);
        }

        return Ok(());
    }
}
