use itertools::Itertools;
use isin::ISIN;
use xls_table_derive::XlsTableRow;

use crate::broker_statement::partial::PartialBrokerStatementRc;
use crate::core::{EmptyResult, GenericResult};
use crate::formats::xls::{self, XlsStatementParser, SectionParser, SheetReader, Cell, SkipCell, TableReader};
use crate::instruments::parse_isin;

use super::common::{
    SecuritiesRegistry, SecuritiesRegistryRc, read_next_table_row, save_instrument_exchange_info, trim_column_title};

pub struct SecuritiesInfoParser {
    statement: PartialBrokerStatementRc,
    securities: SecuritiesRegistryRc,
}

impl SecuritiesInfoParser {
    pub fn new(statement: PartialBrokerStatementRc, securities: SecuritiesRegistryRc) -> Box<dyn SectionParser> {
        Box::new(SecuritiesInfoParser {statement, securities})
    }
}

impl SectionParser for SecuritiesInfoParser {
    fn parse(&mut self, parser: &mut XlsStatementParser) -> EmptyResult {
        let mut statement = self.statement.borrow_mut();
        let securities = self.securities.borrow();

        for security in xls::read_table::<SecuritiesInfoRow>(&mut parser.sheet)? {
            let (symbol, isin) = security.parse(&securities)?;
            let misnamed_instrument = statement.instrument_info.remove(isin.as_ref());

            if let Some(exchange) = &security.exchange {
                // New statements don't have exchange info for some reason

                // When assets are moved between depositaries we might get OTC instrument info in the statement, because
                // they are moved through it. It has ISIN in symbol column and is not usable for us, so just skip it.
                if exchange == "ВНБ" {
                    continue;
                }

                save_instrument_exchange_info(&mut statement.instrument_info, symbol, exchange)?;
            }

            let instrument = statement.instrument_info.get_or_add(symbol);
            instrument.set_name(&security.name);
            instrument.add_isin(isin);

            if let Some(misnamed_instrument) = misnamed_instrument {
                instrument.merge(misnamed_instrument, false);
            }
        }

        Ok(())
    }
}

#[derive(XlsTableRow)]
#[table(trim_column_title_with="trim_column_title", case_insensitive_match=true, space_insensitive_match=true)]
struct SecuritiesInfoRow {
    #[column(name="Сокращенное наименование актива")]
    name: String,
    #[column(name="Торговая площадка", optional=true)]
    exchange: Option<String>,
    #[column(name="Код актива")]
    code: String,
    #[column(name="ISIN", optional=true)]
    isin: Option<String>,
    #[column(name="Код государственной регистрации", alias="Номер гос.регистрации")]
    _4: SkipCell,
    #[column(name="Наименование эмитента")]
    _5: SkipCell,
    #[column(name="Тип")]
    _6: SkipCell,
    #[column(name="Номинал", optional=true)]
    _7: Option<SkipCell>,
    #[column(name="Валюта номинала", optional=true)]
    _8: Option<SkipCell>,
}

impl TableReader for SecuritiesInfoRow {
    fn next_row(sheet: &mut SheetReader) -> Option<&[Cell]> {
        read_next_table_row(sheet)
    }
}

impl SecuritiesInfoRow {
    fn parse<'a>(&'a self, securities: &'a SecuritiesRegistry) -> GenericResult<(&'a str, ISIN)> {
        // Old statements contain both symbol and ISIN columns, but later the symbol column has been removed for some
        // reason, so now we have to look for it in other sections of the statement.

        if let Some(ref isin) = self.isin {
            return Ok((&self.code, parse_isin(isin)?))
        }

        let isin = parse_isin(&self.code)?;
        let symbol = match securities.get(&self.name) {
            Some(info) if !info.symbols.is_empty() => {
                if info.symbols.len() > 1 {
                    return Err!("{:?} resolves to multiple symbols: {}",
                        self.name, info.symbols.iter().join(", "));
                }
                info.symbols.iter().next().unwrap()
            },

            // OTC instruments don't have symbol, so return ISIN instead of it and rely on symbol remapping
            Some(info) if !info.isin.is_empty() => {
                if info.isin.len() > 1 {
                    return Err!("{:?} resolves to multiple ISIN: {}",
                        self.name, info.isin.iter().join(", "));
                }

                let known_isin = info.isin.iter().next().unwrap();
                if isin != *known_isin {
                    return Err!("{:?} resolves to multiple ISIN: {} and {}",
                        self.name, *known_isin, isin);
                }

                known_isin.as_ref()
            },

            _ => return Err!("Unable to find symbol of {:?}", self.name),
        };

        Ok((symbol, isin))
    }
}