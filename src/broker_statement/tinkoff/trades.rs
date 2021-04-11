use num_traits::{FromPrimitive, Zero};

use xls_table_derive::XlsTableRow;

use crate::broker_statement::trades::{ForexTrade, StockBuy, StockSell};
use crate::broker_statement::xls::{XlsStatementParser, SectionParser};
use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::forex::parse_forex_code;
use crate::formatting::format_date;
use crate::time::{Date, Time, DateTime};
use crate::types::Decimal;
use crate::util::DecimalRestrictions;
use crate::xls::{self, SheetReader, Cell, SkipCell, TableReader};

use super::common::{
    read_next_table_row, parse_cash, parse_date_cell, parse_decimal_cell, parse_quantity_cell,
    parse_time_cell};

pub struct TradesParser {
}

impl SectionParser for TradesParser {
    fn parse(&mut self, parser: &mut XlsStatementParser) -> EmptyResult {
        let mut trades = xls::read_table::<TradeRow>(&mut parser.sheet)?;
        trades.sort_by_key(|trade| (trade.date, trade.time, trade.id));

        for trade in trades {
            if !trade.accumulated_coupon_income.is_zero() {
                return Err!("Bonds aren't supported yet");
            } else if trade.leverage_rate.is_some() {
                return Err!("Leverage is not supported yet");
            }

            let conclusion_time = DateTime::new(trade.date, trade.time);
            if trade.quantity == 0 {
                return Err!("Invalid {} trade quantity: {:?}", trade.symbol, trade.quantity);
            }

            let price = parse_cash(
                &trade.price_currency, trade.price, DecimalRestrictions::StrictlyPositive)?;

            let volume = parse_cash(
                &trade.settlement_currency, trade.volume, DecimalRestrictions::StrictlyPositive)?;
            debug_assert_eq!(volume, (price * trade.quantity).round());

            let commission = match trade.commission_currency {
                Some(currency) => {
                    parse_cash(&currency, trade.commission, DecimalRestrictions::PositiveOrZero)?
                }
                None if trade.commission.is_zero() => {
                    Cash::new(&trade.settlement_currency, trade.commission)
                },
                None => return Err!(
                    "Got {} trade at {} without commission currency",
                    trade.symbol, format_date(conclusion_time),
                ),
            };

            let forex = parse_forex_code(&trade.symbol);

            match trade.operation.as_str() {
                "Покупка" => {
                    if let Ok((base, _quote, _lot_size)) = forex {
                        let from = volume;
                        let to = Cash::new(base, Decimal::from_u32(trade.quantity).unwrap());
                        parser.statement.forex_trades.push(ForexTrade::new(
                            conclusion_time.into(), from, to, commission));
                    } else {
                        parser.statement.stock_buys.push(StockBuy::new_trade(
                            &trade.symbol, trade.quantity.into(), price, volume, commission,
                            conclusion_time.into(), trade.execution_date, false));
                    }
                },
                "Продажа" => {
                    if let Ok((base, _quote, _lot_size)) = forex {
                        let from = Cash::new(base, Decimal::from_u32(trade.quantity).unwrap());
                        let to = volume;
                        parser.statement.forex_trades.push(ForexTrade::new(
                            conclusion_time.into(), from, to, commission));
                    } else {
                        parser.statement.stock_sells.push(StockSell::new_trade(
                            &trade.symbol, trade.quantity.into(), price, volume,
                            commission, conclusion_time.into(), trade.execution_date, false, false));
                    }
                },
                _ => return Err!("Unsupported trade operation: {:?}", trade.operation),
            }
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
    #[column(name="Дата заключения", parse_with="parse_date_cell")]
    date: Date,
    #[column(name="Время", parse_with="parse_time_cell")]
    time: Time,
    #[column(name="Торговая площадка")]
    _4: SkipCell,
    #[column(name="Режим торгов")]
    _5: SkipCell,
    #[column(name="Вид сделки")]
    operation: String,
    #[column(name="Сокращенное наименование актива")]
    _7: SkipCell,
    #[column(name="Код актива")]
    symbol: String,
    #[column(name="Цена за единицу", parse_with="parse_decimal_cell")]
    price: Decimal,
    #[column(name="Валюта цены")]
    price_currency: String,
    #[column(name="Количество", parse_with="parse_quantity_cell")]
    quantity: u32,
    #[column(name="Сумма (без НКД)")]
    _12: SkipCell,
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
    _18: Option<String>,
    #[column(name="Валюта комиссии биржи", optional=true)]
    _19: Option<String>,
    #[column(name="Комиссия клир. центра", optional=true)]
    _20: Option<String>,
    #[column(name="Валюта комиссии клир. центра", optional=true)]
    _21: Option<String>,

    #[column(name="Ставка РЕПО(%)")]
    leverage_rate: Option<String>,
    #[column(name="Контрагент")]
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

fn parse_trade_id(cell: &Cell) -> GenericResult<u64> {
    let value = xls::get_string_cell(cell)?;
    Ok(value.parse().map_err(|_| format!("Got an unexpected trade ID: {:?}", value))?)
}