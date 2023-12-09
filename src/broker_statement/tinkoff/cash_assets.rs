use std::cmp::Ordering;
use std::collections::HashSet;

use chrono::Datelike;
use lazy_static::lazy_static;
use regex::Regex;

use xls_table_derive::XlsTableRow;

use crate::broker_statement::fees::Fee;
use crate::broker_statement::partial::{PartialBrokerStatement, PartialBrokerStatementRc};
use crate::broker_statement::payments::Withholding;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, CashAssets};
use crate::formats::xls::{self, XlsStatementParser, SectionParser, SheetReader, Cell, SkipCell, TableReader};
use crate::instruments::{InstrumentId, ISIN_REGEX};
use crate::time::{Date, Time};
use crate::types::Decimal;
use crate::util::{self, DecimalRestrictions};

use super::common::{read_next_table_row, parse_date_cell, parse_decimal_cell, parse_time_cell};

pub struct CashAssetsParser {
    statement: PartialBrokerStatementRc,
}

impl CashAssetsParser {
    pub fn new(statement: PartialBrokerStatementRc) -> Box<dyn SectionParser> {
        Box::new(CashAssetsParser {statement})
    }
}

impl SectionParser for CashAssetsParser {
    fn parse(&mut self, parser: &mut XlsStatementParser) -> EmptyResult {
        let mut statement = self.statement.borrow_mut();
        let currencies = parse_current_assets(parser, &mut statement)?;
        parse_cash_flows(parser, &mut statement, &currencies)?;
        Ok(())
    }
}

fn parse_current_assets(
    parser: &mut XlsStatementParser, statement: &mut PartialBrokerStatement,
) -> GenericResult<HashSet<String>> {
    let mut currencies = HashSet::new();
    statement.has_starting_assets.get_or_insert(false);

    for assets in &xls::read_table::<AssetsRow>(&mut parser.sheet)? {
        currencies.insert(assets.currency.clone());

        if !assets.starting.is_zero() {
            statement.has_starting_assets.replace(true);
        }

        let planned = Cash::new(&assets.currency, assets.planned);
        if !planned.is_zero() {
            statement.assets.cash.as_mut().unwrap().deposit(planned);
        }

        if !assets.debt.is_zero() {
            return Err!("Debt is not supported yet");
        }

        if !assets.uncovered.is_zero() {
            return Err!("Leverage is not supported yet");
        }
    }

    Ok(currencies)
}

#[derive(XlsTableRow)]
struct AssetsRow {
    #[column(name="Валюта")]
    currency: String,
    #[column(name="Входящий остаток на начало периода:", parse_with="parse_decimal_cell")]
    starting: Decimal,
    #[column(name="Исходящий остаток на конец периода:")]
    _2: SkipCell,

    // Regex to support variations:
    // * "Плановый исходящий остаток на конец периода (с учетом неисполненных на дату отчета сделок):"
    // * "Плановый исходящий остаток на конец периода (с учетом неисполненных на дату "
    #[column(name=r"^Плановый исходящий остаток на конец периода", regex=true, parse_with="parse_decimal_cell")]
    planned: Decimal,

    #[column(name="Задолженность клиента перед брокером:", parse_with="parse_decimal_cell")]
    debt: Decimal,
    #[column(name="Сумма непокрытого остатка:", parse_with="parse_decimal_cell")]
    uncovered: Decimal,
    #[column(name="Задолженность клиента перед Депозитарием (справочно)")]
    _6: SkipCell,
}

impl TableReader for AssetsRow {
    fn next_row(sheet: &mut SheetReader) -> Option<&[Cell]> {
        read_next_table_row(sheet)
    }
}

fn parse_cash_flows(
    parser: &mut XlsStatementParser, statement: &mut PartialBrokerStatement,
    currencies: &HashSet<String>,
) -> EmptyResult {
    let mut cash_flows = Vec::new();

    struct CashFlow<'a> {
        date: Date,
        time: Option<Time>,
        currency: &'a str,
        info: CashFlowRow,
    }

    loop {
        let row = xls::strip_row_expecting_columns(parser.sheet.next_row_checked()?, 1)?;
        let title = xls::get_string_cell(row[0])?;

        let currency = match currencies.get(title) {
            Some(currency) => currency.as_str(),
            None => {
                parser.sheet.step_back();
                break;
            }
        };

        for cash_flow in xls::read_table::<CashFlowRow>(&mut parser.sheet)? {
            let (date, time) = match cash_flow.date {
                Some(date) => (date, cash_flow.time),
                None => (cash_flow.execution_date, None),
            };

            cash_flows.push(CashFlow {
                date, time,
                currency: currency,
                info: cash_flow,
            });
        }
    }

    cash_flows.sort_by(|a, b| {
        a.date.cmp(&b.date).then_with(|| {
            match (a.time, b.time) {
                (Some(a), Some(b)) => a.cmp(&b),
                _ => Ordering::Equal,
            }
        })
    });

    for CashFlow {date, currency, info: cash_flow, ..} in cash_flows {
        cash_flow.parse(date, currency, statement)?;
    }

    Ok(())
}

#[derive(XlsTableRow)]
struct CashFlowRow {
    #[column(name="Дата", parse_with="parse_date_cell")]
    date: Option<Date>,
    #[column(name="Время совершения", parse_with="parse_time_cell")]
    time: Option<Time>,
    #[column(name="Дата исполнения", parse_with="parse_date_cell")]
    execution_date: Date,
    #[column(name="Операция")]
    operation: String,
    #[column(name="Сумма зачисления", parse_with="parse_decimal_cell")]
    deposit: Decimal,
    #[column(name="Сумма списания", parse_with="parse_decimal_cell")]
    withdrawal: Decimal,
    #[column(name="Примечание")]
    comment: Option<String>,
}

impl TableReader for CashFlowRow {
    fn next_row(sheet: &mut SheetReader) -> Option<&[Cell]> {
        read_next_table_row(sheet)
    }
}

impl CashFlowRow {
    fn parse(&self, date: Date, currency: &str, statement: &mut PartialBrokerStatement) -> EmptyResult {
        let operation = &self.operation;

        let deposit = util::validate_named_cash(
            "deposit amount", currency, self.deposit, DecimalRestrictions::PositiveOrZero)?;

        let withdrawal = util::validate_named_cash(
            "withdrawal amount", currency, self.withdrawal, DecimalRestrictions::PositiveOrZero)?;

        let check_amount = |amount: Cash| -> GenericResult<Cash> {
            if amount.is_zero() || !matches!((deposit.is_zero(), withdrawal.is_zero()), (true, false) | (false, true)) {
                return Err!(
                    "Got an unexpected deposit and withdrawal amounts for {} operation: {} and {}",
                    operation, deposit, withdrawal);
            }

            Ok(amount)
        };

        match operation.as_str() {
            "Пополнение счета" => {
                statement.deposits_and_withdrawals.push(CashAssets::new_from_cash(
                    date, check_amount(deposit)?));
            },
            "Вывод средств" => {
                statement.deposits_and_withdrawals.push(CashAssets::new_from_cash(
                    date, -check_amount(withdrawal)?));
            },

            "Покупка/продажа"
                | "DVP/RVP" // Trade from non-brokerage account (https://github.com/KonishchevDmitry/investments/issues/83)
                | "РЕПО" => {
                // All trade-related cash flows are calculated during trades processing
            },

            "Комиссия за сделки" | "Гербовый сбор" => {
                // All commissions are calculated during trades processing
            },

            "Комиссия по тарифу" => {
                let amount = check_amount(withdrawal)?;
                let description = operation.clone();
                statement.fees.push(Fee::new(date, Withholding::new(amount), Some(description)));
            },

            "Выплата дивидендов" => {
                let description = self.comment.as_deref().unwrap_or_default();
                let issuer_name = parse_dividend_description(description)?;
                let issuer_id = InstrumentId::Name(issuer_name.to_owned());
                let amount = check_amount(deposit)?;
                statement.dividend_accruals(self.execution_date, issuer_id, true).add(date, amount);
            },
            "Налог (дивиденды)" => {
                let description = self.comment.as_deref().unwrap_or_default();
                let issuer_name = parse_dividend_description(description)?;
                let issuer_id = InstrumentId::Name(issuer_name.to_owned());
                let amount = check_amount(withdrawal)?;
                statement.tax_accruals(self.execution_date, issuer_id, true).add(date, amount);
            },

            "Налог" => {
                let year = date.year();

                let withholding = if deposit.is_zero() {
                    Withholding::Withholding(check_amount(withdrawal)?)
                } else {
                    Withholding::Refund(check_amount(deposit)?)
                };

                statement.tax_agent_withholdings.add(date, year, withholding)?;
            },

            _ => return Err!("Unsupported cash flow operation: {:?}", operation),
        };

        Ok(())
    }
}

fn parse_dividend_description(description: &str) -> GenericResult<&str> {
    lazy_static! {
        static ref REGEX: Regex = Regex::new(&format!(
            r"^(?:{isin}/ )?(?P<issuer_name>[^/]+)/ \d+ шт\.$", isin=ISIN_REGEX),
        ).unwrap();
    }

    let captures = REGEX.captures(description).ok_or_else(|| format!(
        "Unexpected dividend description: {:?}", description))?;

    Ok(captures.name("issuer_name").unwrap().as_str())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(description, issuer,
        case("Ростел -ап/ 20 шт.", "Ростел -ап"),
        case("KYG875721634/ Tencent Holdings LTD_ORD SHS/ 251 шт.", "Tencent Holdings LTD_ORD SHS"),
    )]
    fn dividend_parsing(description: &str, issuer: &str) {
        assert_eq!(parse_dividend_description(description).unwrap(), issuer);
    }
}