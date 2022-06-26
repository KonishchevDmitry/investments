use crate::broker_statement::fees::Fee;
use crate::broker_statement::partial::{PartialBrokerStatement, PartialBrokerStatementRc};
use crate::broker_statement::payments::Withholding;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, CashAssets};
use crate::formatting;
use crate::types::Decimal;
use crate::util::{self, DecimalRestrictions};
use crate::xls::{self, XlsStatementParser, SectionParser, TableReader, Cell, SkipCell};

use xls_table_derive::XlsTableRow;

use super::common::{parse_short_date, parse_currency};

pub struct CashFlowParser {
    statement: PartialBrokerStatementRc,
}

impl CashFlowParser {
    pub fn new(statement: PartialBrokerStatementRc) -> Box<dyn SectionParser> {
        Box::new(CashFlowParser {statement})
    }
}

impl SectionParser for CashFlowParser {
    fn consume_title(&self) -> bool {
        false
    }

    fn parse(&mut self, parser: &mut XlsStatementParser) -> EmptyResult {
        let mut statement = self.statement.borrow_mut();

        let title_row = xls::strip_row_expecting_columns(parser.sheet.next_row_checked()?, 1)?;
        let currency = parse_currency(xls::get_string_cell(title_row[0])?)?;

        for cash_flow in &xls::read_table::<CashFlowRow>(&mut parser.sheet)? {
            cash_flow.parse(&mut statement, currency)?;
        }

        Ok(())
    }
}

#[derive(XlsTableRow)]
struct CashFlowRow {
    #[column(name="Дата")]
    date: String,
    #[column(name="Операция")]
    operation: String,
    #[column(name="Сумма зачисления")]
    deposit: Decimal,
    #[column(name="Сумма списания")]
    withdrawal: Decimal,
    #[column(name="В т.ч.НДС (руб.)")]
    tax: Decimal,
    #[column(name="Остаток (+/-)")]
    _5: SkipCell,
    #[column(name="в т.ч. гарант. обеспечение")]
    warranty: Decimal,
    #[column(name="в т.ч. депозитная маржа")]
    margin: Decimal,
    #[column(name="Площадка")]
    _8: SkipCell,
    #[column(name="Примечание")]
    comment: Option<String>,
    #[column(name="Промежуточный клиринг (FORTS)")]
    _10: SkipCell,
}

impl TableReader for CashFlowRow {
    fn skip_row(row: &[Option<&Cell>]) -> GenericResult<bool> {
        Ok(
            xls::get_string_cell(row[0].unwrap())?.starts_with("Итого по валюте ") ||
                xls::get_string_cell(row[1].unwrap())? == "Итого:"
        )
    }
}

impl CashFlowRow {
    fn parse(&self, statement: &mut PartialBrokerStatement, currency: &str) -> EmptyResult {
        let date = parse_short_date(&self.date)?;
        let operation = self.operation.as_str();

        let mut deposit_restrictions = DecimalRestrictions::Zero;
        let mut withdrawal_restrictions = DecimalRestrictions::Zero;

        match operation {
            "Приход ДС" => {
                deposit_restrictions = DecimalRestrictions::StrictlyPositive;
                statement.deposits_and_withdrawals.push(CashAssets::new(
                    date, currency, self.deposit));
            },
            "Покупка/Продажа" | "Покупка/Продажа (репо)" => {
                deposit_restrictions = DecimalRestrictions::PositiveOrZero;
                withdrawal_restrictions = DecimalRestrictions::PositiveOrZero;
            },
            "Урегулирование сделок" |
            "Вознаграждение компании" |
            "Комиссия за перенос позиции" |
            "Фиксированное вознаграждение по тарифу" |
            "Вознаграждение за обслуживание счета депо" => {
                let amount = Cash::new(currency, self.withdrawal);
                withdrawal_restrictions = DecimalRestrictions::StrictlyPositive;

                let description = operation.strip_prefix("Комиссия ").unwrap_or(operation);
                let description = format!("Комиссия брокера: {}", formatting::untitle(description));

                statement.fees.push(Fee::new(date, amount, Some(description)));
            },
            "НДФЛ" => {
                let withheld_tax = Cash::new(currency, self.withdrawal);
                withdrawal_restrictions = DecimalRestrictions::StrictlyPositive;

                let comment = self.comment.as_deref().unwrap_or_default();
                let year = comment.parse::<u16>().map_err(|_| format!(
                    "Got an unexpected comment for {:?} operation: {:?}",
                    operation, comment,
                ))? as i32;

                statement.tax_agent_withholdings.add(
                    date, year, Withholding::Withholding(withheld_tax))?;
            },
            _ => return Err!("Unsupported cash flow operation: {:?}", self.operation),
        };

        // FIXME(konishchev): Rewrite
        for &(name, value, restrictions) in &[
            ("deposit", self.deposit, deposit_restrictions),
            ("withdrawal", self.withdrawal, withdrawal_restrictions),
            ("tax", self.tax, DecimalRestrictions::Zero),
            ("warranty", self.warranty, DecimalRestrictions::Zero),
            ("margin", self.margin, DecimalRestrictions::Zero),
        ] {
            util::validate_decimal(value, restrictions).map_err(|_| format!(
                "Unexpected {} amount for {:?} operation: {}", name, operation, value))?;
        }

        Ok(())
    }
}