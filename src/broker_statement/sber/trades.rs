use scraper::ElementRef;

use crate::broker_statement::partial::{PartialBrokerStatement, PartialBrokerStatementRc};
use crate::broker_statement::trades::{StockBuy, StockSell};
use crate::core::EmptyResult;
use crate::currency::Cash;
use crate::formats::html::{self, SectionParser};
use crate::formats::xls::SkipCell;
use crate::time::{Date, DateTime, Time};
use crate::types::Decimal;
use crate::util::{self, DecimalRestrictions};

use xls_table_derive::XlsTableRow;

use super::common::{parse_date_cell, parse_time_cell, parse_integer_cell, parse_quantity_cell, parse_decimal_cell};

pub struct TradesParser {
    statement: PartialBrokerStatementRc,
}

impl TradesParser {
    pub fn new(statement: PartialBrokerStatementRc) -> Box<dyn SectionParser> {
        Box::new(TradesParser {statement})
    }
}

impl SectionParser for TradesParser {
    fn parse(&mut self, table: ElementRef) -> EmptyResult {
        let mut statement = self.statement.borrow_mut();

        let mut trade = html::read_table::<TradeRow>(table)?;
        trade.sort_by_key(|trade| trade.id);

        for trade in trade {
            trade.parse(&mut statement)?;
        }

        Ok(())
    }
}

#[derive(XlsTableRow)]
struct TradeRow {
    #[column(name="Дата заключения", parse_with="parse_date_cell")]
    date: Date,
    #[column(name="Дата расчетов", parse_with="parse_date_cell")]
    settlement_date: Date,
    #[column(name="Время заключения", parse_with="parse_time_cell")]
    time: Time,
    #[column(name="Наименование ЦБ")]
    _3: SkipCell,
    #[column(name="Код ЦБ")]
    symbol: String,
    #[column(name="Валюта")]
    currency: String,
    #[column(name="Вид")]
    operation: String,
    #[column(name="Количество, шт.", parse_with="parse_quantity_cell")]
    quantity: u32,
    #[column(name="Цена", parse_with="parse_decimal_cell")]
    price: Decimal,
    #[column(name="Сумма", parse_with="parse_decimal_cell")]
    volume: Decimal,
    #[column(name="НКД", parse_with="parse_decimal_cell")]
    accumulated_coupon_income: Decimal,
    #[column(name="Комиссия Брокера", parse_with="parse_decimal_cell")]
    broker_commission: Decimal,
    #[column(name="Комиссия Биржи", parse_with="parse_decimal_cell")]
    exchange_commission: Decimal,
    #[column(name="Номер сделки", parse_with="parse_integer_cell")]
    id: u64,
    #[column(name="Комментарий")]
    _14: String,
    #[column(name="Статус сделки")]
    _15: String,
}

impl TradeRow {
    fn parse(&self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        if !self.accumulated_coupon_income.is_zero() {
            return Err!("Bonds aren't supported yet");
        }

        let time = DateTime::new(self.date, self.time);

        let price = util::validate_named_decimal("price", self.price, DecimalRestrictions::StrictlyPositive)
        .map(|price| Cash::new(&self.currency, price))?;

        let volume = util::validate_named_decimal("trade volume", self.volume, DecimalRestrictions::StrictlyPositive)
            .map(|volume| Cash::new(&self.currency, volume))?;
        debug_assert_eq!(volume, (price * self.quantity).round());

        let broker_commission = util::validate_named_decimal(
            "broker commission", self.broker_commission, DecimalRestrictions::PositiveOrZero)?;

        let exchange_commission = util::validate_named_decimal(
            "exchange commission", self.exchange_commission, DecimalRestrictions::PositiveOrZero)?;

        let commission = Cash::new(&self.currency, broker_commission + exchange_commission);

        match self.operation.as_str() {
            "Покупка" => {
                statement.stock_buys.push(StockBuy::new_trade(
                    &self.symbol, self.quantity.into(), price, volume, commission,
                    time.into(), self.settlement_date));
            },
            "Продажа" => {
                statement.stock_sells.push(StockSell::new_trade(
                    &self.symbol, self.quantity.into(), price, volume, commission,
                    time.into(), self.settlement_date, false));
            },
            _ => return Err!("Unsupported trade operation: {:?}", self.operation),
        }

        Ok(())
    }
}