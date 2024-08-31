use crate::broker_statement::fees::Fee;
use crate::broker_statement::partial::{PartialBrokerStatement, PartialBrokerStatementRc};
use crate::broker_statement::payments::Withholding;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, CashAssets};
use crate::formats::xls::{self, XlsTableRow, XlsStatementParser, SectionParser, TableReader, Cell, SkipCell};
use crate::formatting;
use crate::time::Date;
use crate::types::Decimal;
use crate::util::{self, DecimalRestrictions};

use super::common::{parse_currency, parse_short_date_cell, trim_column_title};

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
#[table(trim_column_title="trim_column_title")]
struct CashFlowRow {
    #[column(name="Дата", parse_with="parse_short_date_cell")]
    date: Date,
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
        let operation = self.operation.as_str();

        let mut validator = CashFlowValidator {
            row: self,
            deposit: DecimalRestrictions::Zero,
            withdrawal: DecimalRestrictions::Zero
        };

        match operation {
            "Приход ДС" => {
                validator.deposit = DecimalRestrictions::StrictlyPositive;
                validator.validate()?;

                statement.deposits_and_withdrawals.push(CashAssets::new(
                    self.date, currency, self.deposit));
            },

            "Покупка/Продажа" | "Покупка/Продажа (репо)" | "Внебиржевая сделка ОТС" => {
                validator.deposit = DecimalRestrictions::PositiveOrZero;
                validator.withdrawal = DecimalRestrictions::PositiveOrZero;
                validator.validate()?;
            },

            "Урегулирование сделок" |
            "Вознаграждение компании" |
            "Комиссия за перенос позиции" |
            "Фиксированное вознаграждение по тарифу" |
            "Вознаграждение за обслуживание счета депо" => {
                validator.withdrawal = DecimalRestrictions::StrictlyPositive;
                validator.validate()?;

                let amount = Cash::new(currency, self.withdrawal);
                let description = operation.strip_prefix("Комиссия ").unwrap_or(operation);
                let description = format!("Комиссия брокера: {}", formatting::untitle(description));

                statement.fees.push(Fee::new(self.date, Withholding::new(amount), Some(description)));
            },

            "НДФЛ" => {
                validator.withdrawal = DecimalRestrictions::StrictlyPositive;
                validator.validate()?;

                let withheld_tax = Cash::new(currency, self.withdrawal);

                let comment = self.comment.as_deref().unwrap_or_default();
                let year = comment.parse::<u16>().map_err(|_| format!(
                    "Got an unexpected comment for {:?} operation: {:?}",
                    operation, comment,
                ))? as i32;

                statement.tax_agent_withholdings.add(self.date, year, Withholding::new(withheld_tax))?;
            },

            _ => return Err!("Unsupported cash flow operation: {:?}", self.operation),
        };

        Ok(())
    }
}

struct CashFlowValidator<'a> {
    row: &'a CashFlowRow,
    deposit: DecimalRestrictions,
    withdrawal: DecimalRestrictions,
}

impl<'a> CashFlowValidator<'a> {
    fn validate(&self) -> EmptyResult {
        for (name, value, restrictions) in [
            ("deposit", self.row.deposit, self.deposit),
            ("withdrawal", self.row.withdrawal, self.withdrawal),
            ("tax", self.row.tax, DecimalRestrictions::Zero),
            ("warranty", self.row.warranty, DecimalRestrictions::Zero),
            ("margin", self.row.margin, DecimalRestrictions::Zero),
        ] {
            util::validate_decimal(value, restrictions).map_err(|_| format!(
                "Unexpected {} amount for {:?} operation: {}", name, self.row.operation, value))?;
        }

        Ok(())
    }
}