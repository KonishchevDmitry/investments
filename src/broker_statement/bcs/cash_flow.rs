use crate::broker_statement::fees::Fee;
use crate::broker_statement::xls::{XlsStatementParser, SectionParser};
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, CashAssets};
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
                parser.statement.cash_flows.push(CashAssets::new(date, currency, cash_flow.deposit));
            },
            "Покупка/Продажа" => {
                deposit_restrictions = DecimalRestrictions::PositiveOrZero;
                withdrawal_restrictions = DecimalRestrictions::PositiveOrZero;
            },
            "Урегулирование сделок" |
            "Вознаграждение компании" |
            "Вознаграждение за обслуживание счета депо" => {
                withdrawal_restrictions = DecimalRestrictions::StrictlyPositive;

                let mut description = String::with_capacity(operation.len());
                {
                    let mut chars = operation.chars();
                    description.extend(chars.next().unwrap().to_lowercase());
                    description.extend(chars);
                }

                parser.statement.fees.push(Fee {
                    date,
                    amount: Cash::new(currency, -cash_flow.withdrawal),
                    description: Some(description),
                });
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
    _9: SkipCell,
    #[column(name="Промежуточный клиринг (FORTS)")]
    _10: SkipCell,
}

impl TableReader for CashFlowRow {
    fn skip_row(row: &[&Cell]) -> GenericResult<bool> {
        Ok(
            xls::get_string_cell(row[0])?.starts_with("Итого по валюте ") ||
                xls::get_string_cell(row[1])? == "Итого:"
        )
    }
}