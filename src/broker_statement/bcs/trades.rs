#![allow(unused_imports)] // FIXME

use crate::core::{EmptyResult, GenericResult};
use crate::currency::CashAssets;
use crate::types::Decimal;
use crate::util::{self, DecimalRestrictions};
use crate::xls::{self, TableReader, Cell, SkipCell};

use xls_table_derive::XlsTableRow;

use super::{Parser, SectionParser};
use super::common::{parse_short_date, parse_currency};

pub struct TradesParser {
}

impl SectionParser for TradesParser {
    fn parse(&self, parser: &mut Parser) -> EmptyResult {
        for trade in &xls::read_table::<TradeRow>(&mut parser.sheet)? {
            self.process_trade(parser, trade)?;
        }

        Ok(())
    }
}

impl TradesParser {
    fn process_trade(&self, _parser: &mut Parser, _trade: &TradeRow) -> EmptyResult {
        /*
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
        */

        Ok(())
    }
}

#[derive(XlsTableRow)]
struct TradeRow {
    #[column(name="Дата")]
    _0: SkipCell,
    #[column(name="Номер")]
    _1: SkipCell,
    #[column(name="Время")]
    _2: SkipCell,
    #[column(name="Куплено, шт")]
    _3: SkipCell,
    #[column(name="Цена")]
    _4: SkipCell,
    #[column(name="Сумма платежа")]
    _5: SkipCell,
    #[column(name="Продано, шт")]
    _6: SkipCell,
    #[column(name="Цена")]
    _7: SkipCell,
    #[column(name="Сумма выручки")]
    _8: SkipCell,
    #[column(name="Валюта")]
    _9: SkipCell,
    #[column(name="Валюта платежа")]
    _10: SkipCell,
    #[column(name="Дата соверш.")]
    _11: SkipCell,
    #[column(name="Время соверш.")]
    _12: SkipCell,
    #[column(name="Тип сделки")]
    _13: SkipCell,
    #[column(name="Оплата (факт)")]
    _14: SkipCell,
    #[column(name="Поставка (факт)")]
    _15: SkipCell,
    #[column(name="Место сделки")]
    _16: SkipCell,
}

impl TableReader for TradeRow {
    fn skip_row(row: &[&Cell]) -> GenericResult<bool> {
        Ok(xls::get_string_cell(row[0])?.starts_with("Итого по "))
    }
}