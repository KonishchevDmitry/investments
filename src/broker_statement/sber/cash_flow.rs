use scraper::ElementRef;

use crate::broker_statement::partial::{PartialBrokerStatement, PartialBrokerStatementRc};
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, CashAssets};
use crate::formats::html::{self, HtmlTableRow, SectionParser, SkipCell};
use crate::time::Date;
use crate::types::Decimal;
use crate::util::{self, DecimalRestrictions};

use super::common::{parse_date_cell, parse_decimal_cell};

pub struct CashFlowParser {
    statement: PartialBrokerStatementRc,
}

impl CashFlowParser {
    pub fn new(statement: PartialBrokerStatementRc) -> Box<dyn SectionParser> {
        Box::new(CashFlowParser {statement})
    }
}

impl SectionParser for CashFlowParser {
    fn parse(&mut self, table: ElementRef) -> EmptyResult {
        let mut statement = self.statement.borrow_mut();

        for row in html::read_table::<CashFlowRow>(table)? {
            row.parse(&mut statement)?;
        }

        Ok(())
    }
}

#[derive(HtmlTableRow)]
struct CashFlowRow {
    #[column(name="Дата", parse_with="parse_date_cell")]
    date: Date,
    #[column(name="Торговая площадка")]
    _1: SkipCell,
    #[column(name="Описание операции")]
    operation: String,
    #[column(name="Валюта")]
    currency: String,
    #[column(name="Сумма зачисления", parse_with="parse_decimal_cell")]
    deposit: Decimal,
    #[column(name="Сумма списания", parse_with="parse_decimal_cell")]
    withdrawal: Decimal,
}

impl CashFlowRow {
    fn parse(&self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        let operation = &self.operation;

        let deposit = util::validate_named_cash(
            "deposit amount", &self.currency, self.deposit, DecimalRestrictions::PositiveOrZero)?;

        let withdrawal = util::validate_named_cash(
            "withdrawal amount", &self.currency, self.withdrawal, DecimalRestrictions::PositiveOrZero)?;

        let check_amount = |amount: Cash| -> GenericResult<Cash> {
            if amount.is_zero() || !matches!((deposit.is_zero(), withdrawal.is_zero()), (true, false) | (false, true)) {
                return Err!(
                    "Got an unexpected deposit and withdrawal amounts for {:?} operation: {} and {}",
                    operation, deposit, withdrawal);
            }

            Ok(amount)
        };

        for trading_operation in ["Сделка от ", "Комиссия Биржи от ", "Комиссия Брокера оборотная от "] {
            if operation.starts_with(trading_operation) {
                return Ok(());
            }
        }

        match operation.as_str() {
            "Зачисление д/с" => {
                statement.deposits_and_withdrawals.push(CashAssets::new_from_cash(
                    self.date, check_amount(deposit)?));
            },

            _ => return Err!("Unsupported cash flow operation: {:?}", operation),
        };

        Ok(())
    }
}