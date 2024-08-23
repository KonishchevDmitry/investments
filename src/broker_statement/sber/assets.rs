use scraper::ElementRef;

use xls_table_derive::XlsTableRow;

use crate::broker_statement::partial::{PartialBrokerStatement, PartialBrokerStatementRc};
use crate::core::EmptyResult;
use crate::formats::html::{self, SectionParser};
use crate::formats::xls::SkipCell;

use super::common::parse_quantity_cell;

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

#[derive(XlsTableRow, Debug)]
struct AssetsRow {
    // Основной рынок

    #[column(name="Наименование")]
    name: String,
    #[column(name="ISIN ценной бумаги")]
    _1: SkipCell,
    #[column(name="Валюта рыночной цены")]
    _2: SkipCell,

    // Начало периода

    #[column(name="Количество, шт", parse_with="parse_quantity_cell")]
    starting: u32,
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
    #[column(name="Плановый исходящий остаток, шт", parse_with="parse_quantity_cell")]
    planned: u32,
}

impl AssetsRow {
    fn parse(&self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        if self.starting != 0 {
            statement.has_starting_assets.replace(true);
        }

        if self.planned != 0 {
            statement.add_open_position(&self.name, self.planned.into())?;
        }

        Ok(())
    }
}