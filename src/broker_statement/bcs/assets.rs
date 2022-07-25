use crate::broker_statement::partial::{PartialBrokerStatement, PartialBrokerStatementRc};
use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::types::Decimal;
use crate::xls::{self, XlsStatementParser, SectionParser, TableReader, Cell, SkipCell};

use xls_table_derive::XlsTableRow;

use super::common::{parse_currency, parse_symbol};

pub struct AssetsParser {
    statement: PartialBrokerStatementRc,
}

impl AssetsParser {
    pub fn new(statement: PartialBrokerStatementRc) -> Box<dyn SectionParser> {
        Box::new(AssetsParser {statement})
    }
}

impl SectionParser for AssetsParser {
    fn consume_title(&self) -> bool {
        false
    }

    fn parse(&mut self, parser: &mut XlsStatementParser) -> EmptyResult {
        let mut has_starting_assets = false;
        let mut statement = self.statement.borrow_mut();

        for asset in &xls::read_table::<AssetRow>(&mut parser.sheet)? {
            has_starting_assets |= asset.start_value.is_some();

            if !asset.name.ends_with(" (в пути)") {
                asset.parse(&mut statement)?;
            }
        }

        statement.set_has_starting_assets(has_starting_assets)
    }
}

#[derive(XlsTableRow)]
struct AssetRow {
    #[column(name="Вид актива")]
    name: String,
    #[column(name="Номер гос. регистрации ЦБ/ ISIN")]
    _1: SkipCell,
    #[column(name="Тип актива (для ЦБ - № вып.)", alias="Тип ЦБ (№ вып.)")]
    security_type: Option<String>,
    #[column(name="Кол-во ЦБ / Масса ДМ (шт/г)", alias="Кол-во ценных бумаг")]
    _3: SkipCell,
    #[column(name="Цена закрытия/котировка вторич.(5*)")]
    _4: SkipCell,
    #[column(name="Сумма НКД")]
    _5: SkipCell,
    #[column(name="Сумма, в т.ч. НКД")]
    start_value: Option<Decimal>,
    #[column(name="Кол-во ЦБ / Масса ДМ (шт/г)", alias="Кол-во ценных бумаг")]
    end_quantity: Option<i32>,
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
    fn skip_row(row: &[Option<&Cell>]) -> GenericResult<bool> {
        Ok(xls::get_string_cell(row[0].unwrap())? == "Итого:")
    }
}

impl AssetRow {
    fn parse(&self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        let is_currency = self.security_type.as_ref()
            .map(|value| value.trim().len()).unwrap_or(0) == 0;

        if is_currency {
            if let Some(amount) = self.end_value {
                let currency = &parse_currency(&self.name)?;
                statement.assets.cash.as_mut().unwrap().deposit(Cash::new(currency, amount))
            }
        } else {
            let quantity = self.end_quantity.unwrap_or(0);
            if quantity < 0 {
                return Err!("Got a negative open position for {:?}", self.name);
            } else if quantity != 0 {
                let symbol = parse_symbol(&self.name)?;
                statement.add_open_position(&symbol, quantity.into())?;
            }
        }

        Ok(())
    }
}