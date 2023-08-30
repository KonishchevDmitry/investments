use xls_table_derive::XlsTableRow;

use crate::broker_statement::partial::PartialBrokerStatementRc;
use crate::core::EmptyResult;
use crate::formats::xls::{self, XlsStatementParser, SectionParser, SheetReader, Cell, SkipCell, TableReader};
use crate::instruments::parse_isin;

use super::common::{SecuritiesRegistryRc, read_next_table_row, parse_quantity_cell};

pub struct AssetsParser {
    statement: PartialBrokerStatementRc,
    securities: SecuritiesRegistryRc,
}

impl AssetsParser {
    pub fn new(statement: PartialBrokerStatementRc, securities: SecuritiesRegistryRc) -> Box<dyn SectionParser> {
        Box::new(AssetsParser {statement, securities})
    }
}

impl SectionParser for AssetsParser {
    fn parse(&mut self, parser: &mut XlsStatementParser) -> EmptyResult {
        let mut statement = self.statement.borrow_mut();
        let mut securities = self.securities.borrow_mut();

        for asset in xls::read_table::<AssetsRow>(&mut parser.sheet)? {
            let info = securities.entry(asset.name).or_default();

            // We can have multiple rows per one instrument: a technical one with ISIN and a real one with stock symbol
            if let Ok(isin) = parse_isin(&asset.code) {
                info.isin.insert(isin);
            } else {
                info.symbols.insert(asset.code.clone());
            }

            if asset.starting != 0 {
                statement.has_starting_assets.replace(true);
            }

            if asset.planned != 0 {
                statement.add_open_position(&asset.code, asset.planned.into())?;
            }
        }

        Ok(())
    }
}

#[derive(XlsTableRow)]
struct AssetsRow {
    #[column(name="Сокращенное наименование актива")]
    name: String,
    #[column(name="Код актива")]
    code: String,
    #[column(name="Место хранения")]
    _2: SkipCell,
    #[column(name="Входящий остаток", parse_with="parse_quantity_cell")]
    starting: u32,
    #[column(name="Зачисление")]
    _4: SkipCell,
    #[column(name="Списание")]
    _5: SkipCell,
    #[column(name="Исходящий остаток")]
    _6: SkipCell,
    #[column(name="Плановый исходящий остаток", parse_with="parse_quantity_cell")]
    planned: u32,

    // Deprecated columns
    #[column(name="Рыночная цена", optional=true)]
    _8: Option<SkipCell>,
    #[column(name="Валюта цены", optional=true)]
    _9: Option<SkipCell>,
    #[column(name="НКД", optional=true)]
    _10: Option<SkipCell>,
    #[column(name="Рыночная стоимость", optional=true)]
    _11: Option<SkipCell>,
}

impl TableReader for AssetsRow {
    fn next_row(sheet: &mut SheetReader) -> Option<&[Cell]> {
        read_next_table_row(sheet)
    }
}