use std::cell::RefCell;
use std::collections::{HashMap, hash_map::Entry};
use std::rc::Rc;

use log::debug;
use num_traits::FromPrimitive;

use xls_table_derive::XlsTableRow;

use crate::broker_statement::partial::{PartialBrokerStatement, PartialBrokerStatementRc};
use crate::broker_statement::trades::{ForexTrade, StockBuy, StockSell};
use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::exchanges::Exchange;
use crate::forex::parse_forex_code;
use crate::formatting::format_date;
use crate::time::{Date, Time, DateTime};
use crate::types::Decimal;
use crate::util::DecimalRestrictions;
use crate::xls::{self, XlsStatementParser, SectionParser, SheetReader, Cell, SkipCell, TableReader};

use super::common::{
    read_next_table_row, parse_cash, parse_date_cell, parse_decimal_cell, parse_quantity_cell,
    parse_time_cell};

pub type TradesRegistryRc = Rc<RefCell<HashMap<u64, bool>>>;

pub struct TradesParser {
    executed: bool,
    statement: PartialBrokerStatementRc,
    processed_trades: TradesRegistryRc,
}

impl TradesParser {
    pub fn new(
        executed: bool, statement: PartialBrokerStatementRc, processed_trades: TradesRegistryRc,
    ) -> Box<dyn SectionParser> {
        Box::new(TradesParser {executed, processed_trades, statement})
    }

    fn check_trade_id(&self, trade_id: u64) -> GenericResult<bool> {
        Ok(match self.processed_trades.borrow_mut().entry(trade_id) {
            Entry::Vacant(entry) => {
                entry.insert(self.executed);
                true
            },

            Entry::Occupied(mut entry) => {
                if self.executed {
                    let processed_executed = entry.get_mut();
                    if *processed_executed {
                        return Err!("Got a duplicated #{} trade", trade_id);
                    }
                    *processed_executed = true;
                }
                false
            },
        })
    }
}

impl SectionParser for TradesParser {
    fn parse(&mut self, parser: &mut XlsStatementParser) -> EmptyResult {
        let mut statement = self.statement.borrow_mut();

        let mut trades = xls::read_table::<TradeRow>(&mut parser.sheet)?;
        trades.sort_by_key(|trade| (trade.date, trade.time, trade.id));

        for trade in trades {
            if !self.check_trade_id(trade.id)? {
                debug!(
                    "{}: Skipping #{} trade: it's already processed for another statement.",
                    statement.get_period()?.format(), trade.id,
                );
                continue;
            }

            trade.parse(&mut statement)?;
        }

        Ok(())
    }
}

#[derive(XlsTableRow)]
struct TradeRow {
    #[column(name="Номер сделки", parse_with="parse_trade_id")]
    id: u64,
    #[column(name="Номер поручения")]
    _1: SkipCell,
    #[column(name="Признак исполнения", optional=true)]
    _2: Option<SkipCell>,
    #[column(name="Дата заключения", parse_with="parse_date_cell")]
    date: Date,
    #[column(name="Время", parse_with="parse_time_cell")]
    time: Time,
    #[column(name="Торговая площадка")]
    exchange: String,
    #[column(name="Режим торгов")]
    _6: SkipCell,
    #[column(name="Вид сделки")]
    operation: String,
    #[column(name="Сокращенное наименование актива")]
    _8: SkipCell,
    #[column(name="Код актива")]
    symbol: String,
    #[column(name="Цена за единицу", parse_with="parse_decimal_cell")]
    price: Decimal,
    #[column(name="Валюта цены")]
    price_currency: String,
    #[column(name="Количество", parse_with="parse_quantity_cell")]
    quantity: u32,
    #[column(name="Сумма (без НКД)")]
    _13: SkipCell,
    #[column(name="НКД", parse_with="parse_decimal_cell")]
    accumulated_coupon_income: Decimal,
    #[column(name="Сумма сделки", parse_with="parse_decimal_cell")]
    volume: Decimal,
    #[column(name="Валюта расчетов")]
    settlement_currency: String,

    #[column(name="Комиссия брокера", parse_with="parse_decimal_cell")]
    commission: Decimal,
    #[column(name="Валюта комиссии")]
    commission_currency: Option<String>,

    // The following fees are actually included into brokerage commission:
    #[column(name="Комиссия биржи", optional=true)]
    _19: Option<String>,
    #[column(name="Валюта комиссии биржи", optional=true)]
    _20: Option<String>,
    #[column(name="Комиссия клир. центра", optional=true)]
    _21: Option<String>,
    #[column(name="Валюта комиссии клир. центра", optional=true)]
    _22: Option<String>,

    #[column(name="Ставка РЕПО(%)")]
    leverage_rate: Option<String>,
    #[column(name="Контрагент / Брокер", alias="Контрагент")]
    _23: SkipCell,
    #[column(name="Дата расчетов", parse_with="parse_date_cell")]
    execution_date: Date,
    #[column(name="Дата поставки")]
    _25: SkipCell,
    #[column(name="Статус брокера")]
    _26: SkipCell,
    #[column(name="Тип дог.")]
    _27: SkipCell,
    #[column(name="Номер дог.")]
    _28: SkipCell,
    #[column(name="Дата дог.")]
    _29: SkipCell,
    #[column(name="Тип расчета по сделке", optional=true)]
    _30: Option<SkipCell>,
}

impl TableReader for TradeRow {
    fn next_row(sheet: &mut SheetReader) -> Option<&[Cell]> {
        read_next_table_row(sheet)
    }
}

impl TradeRow {
    fn parse(self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        if !self.accumulated_coupon_income.is_zero() {
            return Err!("Bonds aren't supported yet");
        } else if self.leverage_rate.is_some() {
            return Err!("Leverage is not supported yet");
        }

        let conclusion_time = DateTime::new(self.date, self.time);
        if self.quantity == 0 {
            return Err!("Invalid {} trade quantity: {:?}", self.symbol, self.quantity);
        }

        let price = parse_cash(
            &self.price_currency, self.price, DecimalRestrictions::StrictlyPositive)?;

        let volume = parse_cash(
            &self.settlement_currency, self.volume, DecimalRestrictions::StrictlyPositive)?;
        debug_assert_eq!(volume, (price * self.quantity).round());

        let commission = match self.commission_currency {
            Some(currency) => {
                parse_cash(&currency, self.commission, DecimalRestrictions::PositiveOrZero)?
            }
            None if self.commission.is_zero() => {
                Cash::new(&self.settlement_currency, self.commission)
            },
            None => return Err!(
                "Got {} trade at {} without commission currency",
                self.symbol, format_date(conclusion_time),
            ),
        };

        let forex = parse_forex_code(&self.symbol);

        if forex.is_err() {
            let exchange = match self.exchange.as_str() {
                "ММВБ" | "МосБиржа" => Exchange::Moex,
                "СПБ" | "СПБиржа" => Exchange::Spb,
                _ => return Err!("Unknown exchange: {:?}", self.exchange),
            };
            statement.instrument_info.get_or_add(&self.symbol).exchanges.add_prioritized(exchange);
        }

        match self.operation.as_str() {
            "Покупка" => {
                if let Ok((base, _quote, _lot_size)) = forex {
                    let from = volume;
                    let to = Cash::new(base, Decimal::from_u32(self.quantity).unwrap());
                    statement.forex_trades.push(ForexTrade::new(
                        conclusion_time.into(), from, to, commission));
                } else {
                    statement.stock_buys.push(StockBuy::new_trade(
                        &self.symbol, self.quantity.into(), price, volume, commission,
                        conclusion_time.into(), self.execution_date, false));
                }
            },
            "Продажа" => {
                if let Ok((base, _quote, _lot_size)) = forex {
                    let from = Cash::new(base, Decimal::from_u32(self.quantity).unwrap());
                    let to = volume;
                    statement.forex_trades.push(ForexTrade::new(
                        conclusion_time.into(), from, to, commission));
                } else {
                    statement.stock_sells.push(StockSell::new_trade(
                        &self.symbol, self.quantity.into(), price, volume,
                        commission, conclusion_time.into(), self.execution_date, false, false));
                }
            },
            _ => return Err!("Unsupported trade operation: {:?}", self.operation),
        }

        Ok(())
    }
}

fn parse_trade_id(cell: &Cell) -> GenericResult<u64> {
    let value = xls::get_string_cell(cell)?;
    Ok(value.parse().map_err(|_| format!("Got an unexpected trade ID: {:?}", value))?)
}