use num_traits::cast::ToPrimitive;

use crate::broker_statement::partial::PartialBrokerStatement;
use crate::broker_statement::trades::{StockBuy, StockSell};
use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::types::Decimal;
use crate::util::{self, DecimalRestrictions};
use crate::xls::{self, TableRow, SkipCell};

use xls_table_derive::XlsTableRow;

use super::{Parser, SectionParser};
use super::common::{parse_short_date, parse_currency, parse_symbol};

pub struct TradesParser {
}

impl SectionParser for TradesParser {
    fn parse(&self, parser: &mut Parser) -> EmptyResult {
        let columns_mapping = xls::map_columns(
            parser.sheet.next_row_checked()?, &TradeRow::columns())?;

        let mut current_instrument: Option<CurrentInstrument> = None;

        while let Some(row) = parser.sheet.next_row() {
            if xls::is_empty_row(row) {
                break;
            }

            let row = columns_mapping.map(row)?;
            let first_cell = xls::get_string_cell(row.first().unwrap())?;

            let symbol = match current_instrument {
                None => {
                    current_instrument = Some(CurrentInstrument::parse(first_cell)?);
                    continue;
                },
                Some(ref instrument) => {
                    if first_cell == instrument.end_marker {
                        current_instrument = None;
                        continue;
                    }

                    &instrument.symbol
                },
            };
            let trade = TradeRow::parse(&row)?;

            self.process_trade(&mut parser.statement, symbol, trade)?;
        }

        if current_instrument.is_some() {
            return Err!("Got an unexpected end of trades table");
        }

        Ok(())
    }
}

impl TradesParser {
    fn process_trade(&self, statement: &mut PartialBrokerStatement, symbol: &str, trade: TradeRow) -> EmptyResult {
        let conclusion_date = parse_short_date(&trade.conclusion_date)?;
        let execution_date = parse_short_date(&trade.execution_date)?;
        if trade.date != trade.execution_date {
            return Err!(
                "Trade completion date is different from execution date: {} vs {}",
                trade.date, trade.execution_date);
        }
        if trade.payment_date != trade.execution_date {
            return Err!(
                "Payment date is different from execution date: {} vs {}",
                trade.payment_date, trade.execution_date);
        }

        let currency = parse_currency(&trade.currency)?;
        if trade.payment_currency != trade.currency {
            return Err!(
                "Payment currency is different from trade currency: {:?} vs {:?}",
                trade.payment_currency, trade.currency);
        }

        let (buy, quantity, price) = match (
            (trade.buy_quantity, trade.buy_price),
            (trade.sell_quantity, trade.sell_price),
        ) {
            ((Some(quantity), Some(price)), (None, None)) => (true, quantity, price),
            ((None, None), (Some(quantity), Some(price))) => (false, quantity, price),
            _ => return Err!("Got conflicting buy/sell quantity/price values"),
        };

        let quantity =
            util::validate_decimal(quantity, DecimalRestrictions::StrictlyPositive).ok()
            .and_then(|quantity| {
                if quantity.trunc() == quantity {
                    quantity.to_u32()
                } else {
                    None
                }
            })
            .ok_or_else(|| format!("Invalid quantity: {}", quantity))?;

        let price = util::validate_decimal(price, DecimalRestrictions::StrictlyPositive)
            .map(|price| Cash::new(currency, price))
            .map_err(|_| format!("Invalid price: {}", price))?;

        let commission = Cash::new(currency, dec!(0)); // FIXME

        // FIXME
        if false {
            if buy {
                statement.stock_buys.push(StockBuy::new(
                    symbol, quantity, price, commission, conclusion_date, execution_date));
            } else {
                statement.stock_sells.push(StockSell::new(
                    symbol, quantity, price, commission, conclusion_date, execution_date, false));
            }
        }

        Ok(())
    }
}

#[derive(XlsTableRow)]
struct TradeRow {
    #[column(name="Дата")]
    date: String,
    #[column(name="Номер")]
    _1: SkipCell,
    #[column(name="Время")]
    _2: SkipCell,
    #[column(name="Куплено, шт")]
    buy_quantity: Option<Decimal>,
    #[column(name="Цена")]
    buy_price: Option<Decimal>,
    #[column(name="Сумма платежа")]
    _5: SkipCell,
    #[column(name="Продано, шт")]
    sell_quantity: Option<Decimal>,
    #[column(name="Цена")]
    sell_price: Option<Decimal>,
    #[column(name="Сумма выручки")]
    _8: SkipCell,
    #[column(name="Валюта")]
    currency: String,
    #[column(name="Валюта платежа")]
    payment_currency: String,
    #[column(name="Дата соверш.")]
    conclusion_date: String,
    #[column(name="Время соверш.")]
    _12: SkipCell,
    #[column(name="Тип сделки")]
    _13: SkipCell,
    #[column(name="Оплата (факт)")]
    payment_date: String,
    #[column(name="Поставка (факт)")]
    execution_date: String,
    #[column(name="Место сделки")]
    _16: SkipCell,
}

struct CurrentInstrument {
    symbol: String,
    end_marker: String,
}

impl CurrentInstrument {
    fn parse(name: &str) -> GenericResult<CurrentInstrument> {
        Ok(CurrentInstrument {
            symbol: parse_symbol(name)?,
            end_marker: format!("Итого по {}:", name),
        })
    }
}