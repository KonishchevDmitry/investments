use chrono::Datelike;
use isin::ISIN;
use lazy_static::lazy_static;
use regex::{Regex, RegexBuilder};
use scraper::ElementRef;

use crate::broker_statement::CashGrant;
use crate::broker_statement::partial::{PartialBrokerStatement, PartialBrokerStatementRc};
use crate::broker_statement::payments::Withholding;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, CashAssets, POSITIVE_AMOUNT_REGEX, CURRENCY_REGEX};
use crate::formats::html::{self, HtmlTableRow, SectionParser, SkipCell};
use crate::formatting;
use crate::instruments::{self, InstrumentId, ISIN_REGEX};
use crate::time::Date;
use crate::types::Decimal;
use crate::util::{self, DecimalRestrictions};

use super::common::{parse_date_cell, parse_decimal_cell, skip_row, trim_column_title};

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
#[table(trim_column_title="trim_column_title", skip_row="skip_row")]
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

            "Списание д/с" => {
                statement.deposits_and_withdrawals.push(CashAssets::new_from_cash(
                    self.date, -check_amount(withdrawal)?));
            },

            "Списание д/с. Налог на доходы физ.лиц" => {
                let year = self.date.year();
                let withholding = Withholding::Withholding(check_amount(withdrawal)?);
                statement.tax_agent_withholdings.add(self.date, year, withholding)?;
            },

            _ if operation.starts_with("Дивиденды ") => {
                let dividend = parse_dividend_description(operation)?;
                let issuer = InstrumentId::Isin(dividend.isin);
                let result = check_amount(deposit)?;

                if result.currency != dividend.amount.currency {
                    return Err!(
                        "Got a {} dividend from {} in {} with a payment in {}",
                        dividend.name, formatting::format_date(self.date), dividend.amount.currency, result.currency);
                } else if result > dividend.amount {
                    return Err!(
                        "Got a {} dividend from {} with credited amount which is bigger the dividend amount: {} vs {}",
                        dividend.name, formatting::format_date(self.date), result, dividend.amount);
                }

                statement.dividend_accruals(self.date, issuer.clone(), true).add(self.date, dividend.amount);
                statement.tax_accruals(self.date, issuer, true).add(self.date, dividend.amount - result);
            },

            _ if operation.starts_with("Зачисление участнику акции ") => {
                statement.cash_grants.push(CashGrant::new(
                    self.date, check_amount(deposit)?, operation));
            },

            _ => return Err!("Unsupported cash flow operation: {:?}", operation),
        };

        Ok(())
    }
}

#[derive(Debug, PartialEq)]
struct Dividend {
    name: String,
    isin: ISIN,
    amount: Cash,
}

fn parse_dividend_description(description: &str) -> GenericResult<Dividend> {
    lazy_static! {
        static ref DESCRIPTION_REGEX: Regex = RegexBuilder::new(&format!(concat!("^",
            r"Дивиденды\ (?P<issuer>[^;]+);\ ",
            r"ISIN\ (?P<isin>{isin});\ ",
            r"Дата\ Фиксации\ \d{{2}}/\d{{2}}/\d{{4}};\ ",
            r"Кол-во\ {amount};\ ",
            r"Ставка\ Выплаты\ {amount};\ ",
            r"Курс\ конвертации\ {amount};\ ",
            r"Налог\ удержан\ ",
            r"Дополнительная\ информация:\ Дивиденды\ (?P<amount>{amount})\ (?P<currency>{currency})\ по\ курсу\ ЦБ\ {amount}",
        "$"), isin=ISIN_REGEX, amount=POSITIVE_AMOUNT_REGEX, currency=CURRENCY_REGEX)).ignore_whitespace(true).build().unwrap();
    }

    Ok(DESCRIPTION_REGEX.captures(description).and_then(|captures| {
        let mut currency = captures.name("currency")?.as_str();
        if currency == "RUR" {
            currency = "RUB";
        }

        Some(Dividend {
            name: captures.name("issuer")?.as_str().to_owned(),
            isin: instruments::parse_isin(captures.name("isin")?.as_str()).ok()?,
            amount: Cash::new(currency, captures.name("amount")?.as_str().parse().ok()?)
        })
    }).ok_or_else(|| format!("Unexpected dividend description: {description:?}"))?)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(description, dividend,
        case("Дивиденды Сбербанк-п; ISIN RU0009029557; Дата Фиксации 18/07/2025; Кол-во 20; Ставка Выплаты 34.8400000000; Курс конвертации 1.0000; Налог удержан Дополнительная информация: Дивиденды 696.80 RUR по курсу ЦБ 1", Dividend {
            name: s!("Сбербанк-п"),
            isin: instruments::parse_isin("RU0009029557").unwrap(),
            amount: Cash::new("RUB", dec!(696.80))
        }),
    )]
    fn dividend_parsing(description: &str, dividend: Dividend) {
        assert_eq!(parse_dividend_description(description).unwrap(), dividend);
    }
}