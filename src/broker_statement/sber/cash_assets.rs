use scraper::ElementRef;

use crate::broker_statement::partial::{PartialBrokerStatement, PartialBrokerStatementRc};
use crate::core::EmptyResult;
use crate::currency::Cash;
use crate::formats::html::{self, HtmlTableRow, SectionParser, SkipCell};
use crate::types::Decimal;

use super::common::{parse_decimal_cell, skip_row, trim_column_title};

pub struct CashAssetsParser {
    statement: PartialBrokerStatementRc,
}

impl CashAssetsParser {
    pub fn new(statement: PartialBrokerStatementRc) -> Box<dyn SectionParser> {
        Box::new(CashAssetsParser {statement})
    }
}

impl SectionParser for CashAssetsParser {
    fn parse(&mut self, table: ElementRef) -> EmptyResult {
        let mut statement = self.statement.borrow_mut();
        statement.has_starting_assets.get_or_insert(false);

        for row in html::read_table::<CashAssetsRow>(table)? {
            row.parse(&mut statement)?;
        }

        Ok(())
    }
}

#[derive(HtmlTableRow)]
#[table(trim_column_title="trim_column_title", skip_row="skip_row")]
struct CashAssetsRow {
    #[column(name="Торговая площадка")]
    _0: SkipCell,
    #[column(name="Валюта")]
    currency: String,
    #[column(name="Курс на конец периода")]
    _2: SkipCell,
    #[column(name="Начало периода", parse_with="parse_decimal_cell")]
    start_amount: Decimal,
    #[column(name="Изменение за период")]
    _4: SkipCell,
    #[column(name="Конец периода")]
    _5: SkipCell,
    #[column(name="Плановые зачисления по операциям", alias="Плановые зачисления по сделкам")]
    _6: SkipCell,
    #[column(name="Плановые списания по операциям", alias="Плановые списания по сделкам")]
    _7: SkipCell,
    #[column(name="Плановый исходящий остаток", parse_with="parse_decimal_cell")]
    end_amount: Decimal,
}

impl CashAssetsRow {
    fn parse(&self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        if !self.start_amount.is_zero() {
            statement.has_starting_assets.replace(true);
        }

        let amount = Cash::new(&self.currency, self.end_amount);
        if amount.is_zero() {
            return Ok(());
        }

        let assets = statement.assets.cash.as_mut().unwrap();
        if assets.has_assets(&self.currency) {
            return Err!("Got a duplicated cash assets for {} currency", self.currency);
        }

        assets.deposit(amount);
        Ok(())
    }
}