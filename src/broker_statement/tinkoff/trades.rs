use num_traits::{FromPrimitive, Zero};

use xls_table_derive::XlsTableRow;

use crate::broker_statement::trades::{ForexTrade, StockBuy, StockSell};
use crate::broker_statement::xls::{XlsStatementParser, SectionParser};
use crate::core::EmptyResult;
use crate::currency::Cash;
use crate::types::{Date, Time, Decimal};
use crate::util::DecimalRestrictions;
use crate::xls::{self, SheetReader, Cell, SkipCell, TableReader};

use super::common::{read_next_table_row, parse_date, parse_time, parse_decimal, parse_cash};

pub struct TradesParser {
}

impl SectionParser for TradesParser {
    fn parse(&mut self, parser: &mut XlsStatementParser) -> EmptyResult {
        let mut trades = Vec::new();

        struct Trade {
            date: Date,
            time: Time,
            info: TradeRow,
        }

        for trade in xls::read_table::<TradeRow>(&mut parser.sheet)? {
            trades.push(Trade {
                date: parse_date(&trade.date)?,
                time: parse_time(&trade.time)?,
                info: trade,
            });
        }
        trades.sort_by_key(|trade| (trade.date, trade.time));

        for Trade {date: conclusion_date, info: trade, ..} in trades {
            let accumulated_coupon_income = parse_decimal(
                &trade.accumulated_coupon_income, DecimalRestrictions::No)?;

            if !accumulated_coupon_income.is_zero() {
                return Err!("Bonds aren't supported yet");
            }

            if trade.leverage_rate.is_some() {
                return Err!("Leverage is not supported yet");
            }

            let execution_date = parse_date(&trade.execution_date)?;

            let quantity: u32 = match trade.quantity.parse() {
                Ok(quantity) if quantity > 0 => quantity,
                _ => return Err!("Invalid {} trade quantity: {:?}", trade.symbol, trade.quantity),
            };

            let price = parse_cash(
                &trade.price_currency, &trade.price, DecimalRestrictions::StrictlyPositive)?;

            let volume = parse_cash(
                &trade.settlement_currency, &trade.volume, DecimalRestrictions::StrictlyPositive)?;
            debug_assert_eq!(volume, (price * quantity).round());

            let commission = parse_cash(
                &trade.commission_currency, &trade.commission, DecimalRestrictions::PositiveOrZero)?;

            let forex = if trade.symbol == "USD000UTSTOM" {
                Some("USD")
            } else {
                None
            };

            match trade.operation.as_str() {
                "Покупка" => {
                    if let Some(currency) = forex {
                        parser.statement.forex_trades.push(ForexTrade {
                            from: volume,
                            to: Cash::new(currency, Decimal::from_u32(quantity).unwrap()),
                            commission,
                            conclusion_date
                        })
                    } else {
                        parser.statement.stock_buys.push(StockBuy::new(
                            &trade.symbol, quantity.into(), price, volume, commission,
                            conclusion_date, execution_date));
                    }
                },
                "Продажа" => {
                    if let Some(currency) = forex {
                        parser.statement.forex_trades.push(ForexTrade {
                            from: Cash::new(currency, Decimal::from_u32(quantity).unwrap()),
                            to: volume,
                            commission,
                            conclusion_date
                        })
                    } else {
                        parser.statement.stock_sells.push(StockSell::new(
                            &trade.symbol, quantity.into(), price, volume, commission,
                            conclusion_date, execution_date, false));
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
    #[column(name="Номер сделки")]
    _0: SkipCell,
    #[column(name="Номер поручения")]
    _1: SkipCell,
    #[column(name="Дата заключения")]
    date: String,
    #[column(name="Время")]
    time: String,
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
    #[column(name="Цена за единицу")]
    price: String,
    #[column(name="Валюта цены")]
    price_currency: String,
    #[column(name="Количество")]
    quantity: String,
    #[column(name="Сумма (без НКД)")]
    _12: SkipCell,
    #[column(name="НКД")]
    accumulated_coupon_income: String,
    #[column(name="Сумма сделки")]
    volume: String,
    #[column(name="Валюта расчетов")]
    settlement_currency: String,
    #[column(name="Комиссия брокера")]
    commission: String,
    #[column(name="Валюта комиссии")]
    commission_currency: String,
    #[column(name="Ставка РЕПО(%)")]
    leverage_rate: Option<String>,
    #[column(name="Контрагент")]
    _19: SkipCell,
    #[column(name="Дата расчетов")]
    execution_date: String,
    #[column(name="Дата поставки")]
    _21: SkipCell,
    #[column(name="Статус брокера")]
    _22: SkipCell,
    #[column(name="Тип дог.")]
    _23: SkipCell,
    #[column(name="Номер дог.")]
    _24: SkipCell,
    #[column(name="Дата дог.")]
    _25: SkipCell,
}

impl TableReader for TradeRow {
    fn next_row(sheet: &mut SheetReader) -> Option<&[Cell]> {
        read_next_table_row(sheet)
    }
}