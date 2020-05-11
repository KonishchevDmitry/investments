use crate::broker_statement::partial::PartialBrokerStatement;
use crate::broker_statement::xls::{XlsStatementParser, SectionParser};
use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::types::Decimal;
use crate::xls::{self, TableReader, Cell, SkipCell};

use xls_table_derive::XlsTableRow;

use super::common::{parse_currency, parse_symbol};

pub struct AssetsParser {
}

impl SectionParser for AssetsParser {
    fn consume_title(&self) -> bool {
        false
    }

    fn parse(&mut self, parser: &mut XlsStatementParser) -> EmptyResult {
        let mut has_starting_assets = false;

        for asset in &xls::read_table::<AssetRow>(&mut parser.sheet)? {
            has_starting_assets |= asset.start_value.is_some();

            if !asset.name.ends_with(" (в пути)") {
                self.process_asset(&mut parser.statement, asset)?;
            }
        }

        parser.statement.set_starting_assets(has_starting_assets)
    }
}

impl AssetsParser {
    fn process_asset(&self, statement: &mut PartialBrokerStatement, asset: &AssetRow) -> EmptyResult {
        let is_currency = asset.security_type.as_ref()
            .map(|value| value.trim().len()).unwrap_or(0) == 0;

        if is_currency {
            if let Some(amount) = asset.end_value {
                let currency = &parse_currency(&asset.name)?;
                statement.cash_assets.deposit(Cash::new(currency, amount))
            }
        } else {
            let quantity = asset.end_quantity.unwrap_or(0);
            if quantity != 0 {
                let symbol = parse_symbol(&asset.name)?;
                if statement.open_positions.insert(symbol.clone(), quantity).is_some() {
                    return Err!("Got duplicated position for {}", symbol);
                }
            }
        }

        Ok(())
    }
}

#[derive(XlsTableRow)]
struct AssetRow {
    #[column(name="Вид актива")]
    name: String,
    #[column(name="Номер гос. регистрации ЦБ/ ISIN")]
    _1: SkipCell,
    #[column(name="Тип ЦБ (№ вып.)")]
    security_type: Option<String>,
    #[column(name="Кол-во ценных бумаг")]
    _3: SkipCell,
    #[column(name="Цена закрытия/котировка вторич.(5*)")]
    _4: SkipCell,
    #[column(name="Сумма НКД")]
    _5: SkipCell,
    #[column(name="Сумма, в т.ч. НКД")]
    start_value: Option<Decimal>,
    #[column(name="Кол-во ценных бумаг")]
    end_quantity: Option<u32>,
    #[column(name="Цена закрытия/ котировка вторич.(5*)")]
    _8: SkipCell,
    #[column(name="Сумма НКД")]
    _9: SkipCell,
    #[column(name="Сумма, в т.ч. НКД")]
    end_value: Option<Decimal>,
    #[column(name="Организатор торгов (2*)")]
    _11: SkipCell,
    #[column(name="Место хранения")]
    _12: SkipCell,
    #[column(name="Эмитент")]
    _13: SkipCell,
}

impl TableReader for AssetRow {
    fn skip_row(row: &[&Cell]) -> GenericResult<bool> {
        Ok(xls::get_string_cell(row[0])? == "Итого:")
    }
}