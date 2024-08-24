use crate::broker_statement::partial::{PartialBrokerStatement, PartialBrokerStatementRc};
use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::formats::xls::{self, XlsTableRow, XlsStatementParser, SectionParser, TableReader, Cell, SkipCell};
use crate::instruments;
use crate::types::Decimal;

use super::common::{parse_currency, parse_symbol, trim_column_title};

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
#[table(trim_column_title="trim_column_title")]
struct AssetRow {
    #[column(name="Вид актива")]
    name: String,
    #[column(name="Номер гос. регистрации ЦБ/ ISIN")]
    id: Option<String>,
    #[column(name="Тип актива (для ЦБ - № вып.)", alias="Тип ЦБ (№ вып.)")]
    security_type: Option<String>,
    #[column(name="Кол-во ЦБ / Масса ДМ (шт/г)", alias="Кол-во ценных бумаг")]
    _3: SkipCell,
    #[column(name="Цена закрытия/котировка вторич.")]
    _4: SkipCell,
    #[column(name="Сумма НКД")]
    _5: SkipCell,
    #[column(name="Сумма, в т.ч. НКД")]
    start_value: Option<Decimal>,
    #[column(name="Кол-во ЦБ / Масса ДМ (шт/г)", alias="Кол-во ценных бумаг")]
    end_quantity: Option<i32>,
    #[column(name="Цена закрытия/ котировка вторич.")]
    _8: SkipCell,
    #[column(name="Сумма НКД")]
    _9: SkipCell,
    #[column(name="Сумма, в т.ч. НКД")]
    end_value: Option<Decimal>,
    #[column(name="Организатор торгов")]
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
            self.parse_currency(statement)?;
        } else {
            self.parse_stock(statement)?;
        }

        Ok(())
    }

    fn parse_currency(&self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        if let Some(amount) = self.end_value {
            let currency = &parse_currency(&self.name)?;
            statement.assets.cash.as_mut().unwrap().deposit(Cash::new(currency, amount))
        }
        Ok(())
    }

    fn parse_stock(&self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        let symbol = parse_symbol(&self.name)?;

        let quantity = self.end_quantity.unwrap_or(0);
        if quantity < 0 {
            return Err!("Got a negative open position for {:?}", self.name);
        } else if quantity != 0 {
            statement.add_open_position(&symbol, quantity.into())?;
        }

        let isin = self.id.as_ref().and_then(|id| instruments::parse_isin(id).ok())
            .or_else(|| instruments::parse_isin(&self.name).ok())
            .ok_or_else(|| format!("There is no ISIN info for {:?}", self.name))?;
        statement.instrument_info.get_or_add(&symbol).add_isin(isin);

        Ok(())
    }
}