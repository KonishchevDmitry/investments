use crate::broker_statement::fees::Fee;
use crate::broker_statement::taxes::TaxWithholding;
use crate::broker_statement::xls::{XlsStatementParser, SectionParser};
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, CashAssets};
use crate::formatting;
use crate::types::Decimal;
use crate::util::{self, DecimalRestrictions};
use crate::xls::{self, TableReader, Cell, SkipCell};

use xls_table_derive::XlsTableRow;

use super::common::{parse_short_date, parse_currency};

pub struct CashFlowParser {
}

impl SectionParser for CashFlowParser {
    fn consume_title(&self) -> bool {
        false
    }

    fn parse(&mut self, parser: &mut XlsStatementParser) -> EmptyResult {
        let title_row = xls::strip_row_expecting_columns(parser.sheet.next_row_checked()?, 1)?;
        let currency = parse_currency(xls::get_string_cell(&title_row[0])?)?;

        for cash_flow in &xls::read_table::<CashFlowRow>(&mut parser.sheet)? {
            self.process_cash_flow(parser, currency, cash_flow)?;
        }

        Ok(())
    }
}

impl CashFlowParser {
    fn process_cash_flow(&self, parser: &mut XlsStatementParser, currency: &str, cash_flow: &CashFlowRow) -> EmptyResult {
        let date = parse_short_date(&cash_flow.date)?;
        let operation = cash_flow.operation.as_str();

        let mut deposit_restrictions = DecimalRestrictions::Zero;
        let mut withdrawal_restrictions = DecimalRestrictions::Zero;

        match operation {
            "Приход ДС" => {
                deposit_restrictions = DecimalRestrictions::StrictlyPositive;
                parser.statement.deposits_and_withdrawals.push(CashAssets::new(
                    date, currency, cash_flow.deposit));
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
                let amount = Cash::new(currency, cash_flow.withdrawal);
                withdrawal_restrictions = DecimalRestrictions::StrictlyPositive;

                let description = operation.strip_prefix("Комиссия ").unwrap_or(operation);
                let description = format!("Комиссия брокера: {}", formatting::untitle(description));

                parser.statement.fees.push(Fee::new(date, amount, Some(description)));
            },
            "НДФЛ" => {
                let withheld_tax = Cash::new(currency, cash_flow.withdrawal);
                withdrawal_restrictions = DecimalRestrictions::StrictlyPositive;

                let comment = cash_flow.comment.as_deref().unwrap_or_default();
                let year = comment.parse::<u16>().map_err(|_| format!(
                    "Got an unexpected comment for {:?} operation: {:?}",
                    operation, comment,
                ))? as i32;

                let tax_withholding = TaxWithholding::new(date, year, withheld_tax)?;
                parser.statement.tax_agent_withholdings.push(tax_withholding);
            },
            _ => return Err!("Unsupported cash flow operation: {:?}", cash_flow.operation),
        };

        for &(name, value, restrictions) in &[
            ("deposit", cash_flow.deposit, deposit_restrictions),
            ("withdrawal", cash_flow.withdrawal, withdrawal_restrictions),
            ("tax", cash_flow.tax, DecimalRestrictions::Zero),
            ("warranty", cash_flow.warranty, DecimalRestrictions::Zero),
            ("margin", cash_flow.margin, DecimalRestrictions::Zero),
        ] {
            util::validate_decimal(value, restrictions).map_err(|_| format!(
                "Unexpected {} amount for {:?} operation: {}", name, operation, value))?;
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