use log::trace;

use crate::broker_statement::partial::PartialBrokerStatementRc;
use crate::core::EmptyResult;
use crate::formats::xls::{self, XlsTableRow, XlsStatementParser, SectionParser, SheetReader, Cell, SkipCell, TableReader};
use crate::instruments::parse_isin;

use super::common::{SecuritiesRegistryRc, read_next_table_row, trim_column_title};

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

        trace!("Assets:");

        for asset in xls::read_table::<AssetsRow>(&mut parser.sheet)? {
            let info = securities.entry(asset.name.clone()).or_default();

            // We can have multiple rows per one instrument: a technical one with ISIN and a real one with stock symbol
            if let Ok(isin) = parse_isin(&asset.code) {
                trace!("* ISIN: {}: {}", isin, asset.name);
                info.isin.insert(isin);
            } else {
                trace!("* Symbol: {}: {}", asset.code, asset.name);
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
#[table(trim_column_title="trim_column_title", case_insensitive_match=true, space_insensitive_match=true)]
struct AssetsRow {
    #[column(name="Наименование актива", alias="Сокращенное наименование актива")]
    name: String,
    #[column(name="Код актива")]
    code: String,
    // FIXME(konishchev): Support it
    #[column(name="ISIN", optional=true)] // Old statements don't have it
    _2: Option<SkipCell>,
    #[column(name="Место хранения")]
    _3: SkipCell,
    #[column(name="Входящий остаток", strict=false)] // Old statements stored it as string, new - as float
    starting: u32,
    #[column(name="Зачисление")]
    _5: SkipCell,
    #[column(name="Списание")]
    _6: SkipCell,
    #[column(name="Исходящий остаток")]
    _7: SkipCell,
    #[column(name="Плановый исходящий остаток", strict=false)] // Old statements stored it as string, new - as float
    planned: u32,

    // Deprecated columns
    #[column(name="Рыночная цена", optional=true)]
    _9: Option<SkipCell>,
    #[column(name="Валюта цены", optional=true)]
    _10: Option<SkipCell>,
    #[column(name="НКД", optional=true)]
    _11: Option<SkipCell>,
    #[column(name="Рыночная стоимость", optional=true)]
    _12: Option<SkipCell>,
}

impl TableReader for AssetsRow {
    fn next_row(sheet: &mut SheetReader) -> Option<&[Cell]> {
        read_next_table_row(sheet)
    }
}