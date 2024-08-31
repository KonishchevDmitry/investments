use scraper::ElementRef;

use crate::broker_statement::partial::{PartialBrokerStatement, PartialBrokerStatementRc};
use crate::core::EmptyResult;
use crate::formats::html::{self, HtmlTableRow, SectionParser, SkipCell};
use crate::types::Decimal;
use crate::util::{self, DecimalRestrictions};

use super::common::{parse_decimal_cell, skip_row, trim_column_title};

pub struct AssetsParser {
    statement: PartialBrokerStatementRc,
}

impl AssetsParser {
    pub fn new(statement: PartialBrokerStatementRc) -> Box<dyn SectionParser> {
        Box::new(AssetsParser {statement})
    }
}

impl SectionParser for AssetsParser {
    fn parse(&mut self, table: ElementRef) -> EmptyResult {
        let mut statement = self.statement.borrow_mut();

        for asset in html::read_table::<AssetsRow>(table)? {
            asset.parse(&mut statement)?;
        }

        Ok(())
    }
}

#[derive(HtmlTableRow)]
#[table(trim_column_title="trim_column_title", skip_row="skip_row")]
struct AssetsRow {
    // Основной рынок

    #[column(name="Наименование")]
    name: String,
    #[column(name="ISIN ценной бумаги")]
    _1: SkipCell,
    #[column(name="Валюта рыночной цены")]
    _2: SkipCell,

    // Начало периода

    #[column(name="Количество, шт", parse_with="parse_decimal_cell")]
    starting: Decimal,
    #[column(name="Номинал")]
    _4: SkipCell,
    #[column(name="Рыночная цена")]
    _5: SkipCell,
    #[column(name="Рыночная стоимость, без НКД")]
    _6: SkipCell,
    #[column(name="НКД")]
    _7: SkipCell,

    // Конец периода

    #[column(name="Количество, шт")]
    _8: SkipCell,
    #[column(name="Номинал")]
    _9: SkipCell,
    #[column(name="Рыночная цена")]
    _10: SkipCell,
    #[column(name="Рыночная стоимость, без НКД")]
    _11: SkipCell,
    #[column(name="НКД")]
    _12: SkipCell,

    // Изменение за период

    #[column(name="Количество, шт")]
    _13: SkipCell,
    #[column(name="Рыночная стоимость")]
    _14: SkipCell,

    // Плановые показатели

    #[column(name="Плановые зачисления по сделкам, шт")]
    _15: SkipCell,
    #[column(name="Плановые списания по сделкам, шт")]
    _16: SkipCell,
    #[column(name="Плановый исходящий остаток, шт", parse_with="parse_decimal_cell")]
    planned: Decimal,
}

impl AssetsRow {
    fn parse(&self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        if !self.starting.is_zero() {
            statement.has_starting_assets.replace(true);
        }

        let quantity = util::validate_named_decimal("quantity", self.planned, DecimalRestrictions::PositiveOrZero)?;
        if !quantity.is_zero() {
            statement.add_open_position(&self.name, quantity)?;
        }

        Ok(())
    }
}