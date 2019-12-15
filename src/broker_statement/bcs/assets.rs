use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::types::Decimal;
use crate::xls::{self, TableReader, Cell, SkipCell};

use xls_table_derive::XlsTableRow;

use super::{Parser, SectionParser};

pub struct AssetsParser {
}

impl SectionParser for AssetsParser {
    // FIXME: It's a prototype - rewrite
    fn parse(&self, parser: &mut Parser) -> EmptyResult {
        parser.sheet.skip_empty_rows();
        parser.sheet.skip_non_empty_rows();
        parser.sheet.skip_empty_rows();
        parser.sheet.next_row_checked()?;

        let assets: Vec<AssetsRow> = xls::read_table(&mut parser.sheet)?;

        for asset in &assets {
            if asset.asset == "Рубль" {
                if let Some(amount) = asset.end_value {
                    parser.statement.cash_assets.deposit(Cash::new("RUB", amount))
                }
            } else {
                continue;
            }
        }

        parser.statement.set_starting_assets(false)?;

        Ok(())
    }
}

#[derive(XlsTableRow)]
struct AssetsRow {
    #[column(name="Вид актива")]
    asset: String,
    #[column(name="Номер гос. регистрации ЦБ/ ISIN")]
    _1: SkipCell,
    #[column(name="Тип ЦБ (№ вып.)")]
    _2: SkipCell,
    #[column(name="Кол-во ценных бумаг")]
    _3: SkipCell,
    #[column(name="Цена закрытия/котировка вторич.(5*)")]
    _4: SkipCell,
    #[column(name="Сумма НКД")]
    _5: SkipCell,
    #[column(name="Сумма, в т.ч. НКД")]
    _6: SkipCell,
    #[column(name="Кол-во ценных бумаг")]
    _7: SkipCell,
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

impl TableReader for AssetsRow {
    fn skip_row(row: &[&Cell]) -> GenericResult<bool> {
        Ok(xls::get_string_cell(row[0])? == "Итого:")
    }
}