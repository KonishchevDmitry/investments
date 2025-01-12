use std::cmp::Ordering;
use std::cell::RefCell;
use std::collections::{HashMap, hash_map::Entry};
use std::fmt::{self, Display, Formatter};
use std::rc::Rc;

use log::debug;

use crate::broker_statement::cash_flows::{CashFlow, CashFlowType};
use crate::broker_statement::partial::{PartialBrokerStatement, PartialBrokerStatementRc};
use crate::broker_statement::trades::{ForexTrade, StockBuy, StockSell};
use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::forex::parse_forex_code;
use crate::formats::xls::{self, XlsTableRow, XlsStatementParser, SectionParser, SheetReader, Cell, SkipCell, TableReader};
use crate::formatting::format_date;
use crate::time::{Date, Time, DateTime};
use crate::types::Decimal;
use crate::util;
use crate::util::DecimalRestrictions;

use super::common::{
    read_next_table_row, parse_date_cell, parse_planned_actual_date_cell, parse_decimal_cell,
    parse_fractional_quantity_cell, parse_time_cell, save_instrument_exchange_info, trim_column_title};

pub type TradesRegistryRc = Rc<RefCell<HashMap<TradeId, bool>>>;

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

    fn check_trade_id(&self, trade_id: &TradeId) -> GenericResult<bool> {
        Ok(match self.processed_trades.borrow_mut().entry(trade_id.clone()) {
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
        trades.sort_by(|a, b| -> Ordering {
            let ord = a.date.cmp(&b.date);
            if ord != Ordering::Equal {
                return ord;
            }

            let ord = a.time.cmp(&b.time);
            if ord != Ordering::Equal {
                return ord;
            }

            a.id.cmp(&b.id)
        });

        for mut trade in trades {
            // REPO trades share their ID
            if matches!(trade.operation.as_str(), "РЕПО 1 Продажа" | "РЕПО 2 Покупка") {
                trade.id = TradeId::String(format!("{}/{}", trade.id, trade.operation));
            }

            if !self.check_trade_id(&trade.id)? {
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
#[table(trim_column_title="trim_column_title", case_insensitive_match=true, space_insensitive_match=true)]
struct TradeRow {
    #[column(name="Номер сделки", parse_with="TradeId::parse")]
    id: TradeId,
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
    #[column(name="Режим торгов", optional=true)]
    _6: Option<SkipCell>,
    #[column(name="Вид сделки")]
    operation: String,
    #[column(name="Сокращенное наименование", alias="Сокращенное наименование актива")]
    _8: SkipCell,
    #[column(name="Код актива")]
    symbol: String,
    #[column(name="Цена за единицу", parse_with="parse_decimal_cell")]
    price: Decimal,
    #[column(name="Валюта цены")]
    price_currency: String,
    #[column(name="Количество", parse_with="parse_fractional_quantity_cell")]
    quantity: Decimal,
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
    _19: Option<SkipCell>,
    #[column(name="Валюта комиссии биржи", optional=true)]
    _20: Option<SkipCell>,
    #[column(name="Комиссия клир. центра", optional=true)]
    _21: Option<SkipCell>,
    #[column(name="Валюта комиссии клир. центра", optional=true)]
    _22: Option<SkipCell>,

    #[column(name="Гербовый сбор", parse_with="parse_decimal_cell", optional=true)]
    stamp_duty: Option<Decimal>,
    #[column(name="Валюта гербового сбора", optional=true)]
    stamp_duty_currency: Option<String>,

    #[column(name="Ставка РЕПО (%)")]
    _24: Option<SkipCell>,
    #[column(name="Контрагент / Брокер", alias="Контрагент", optional=true)]
    _25: Option<SkipCell>,
    #[column(name="Дата расчетов план/факт", alias="Дата расчетов", parse_with="parse_planned_actual_date_cell")]
    execution_date: Date,
    #[column(name="Дата поставки план/факт", alias="Дата поставки")]
    _27: SkipCell,
    #[column(name="Статус брокера")]
    _28: SkipCell,
    #[column(name="Тип дог.")]
    _29: SkipCell,
    #[column(name="Номер дог.")]
    _30: SkipCell,
    #[column(name="Дата дог.")]
    _31: SkipCell,
    #[column(name="Тип расчета по сделке", optional=true)]
    _32: Option<SkipCell>,
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
        }

        let forex = parse_forex_code(&self.symbol).ok();
        let operation = self.operation.as_str();

        let conclusion_time = DateTime::new(self.date, self.time);
        if self.quantity.is_zero() {
            return Err!("Invalid {} trade quantity: {}", self.symbol, self.quantity);
        }

        let price = util::validate_named_cash(
            "price", &self.price_currency, self.price, DecimalRestrictions::StrictlyPositive)?;

        let volume = util::validate_named_cash(
            "trade volume", &self.settlement_currency, self.volume, DecimalRestrictions::StrictlyPositive)?;
        debug_assert_eq!(volume, (price * self.quantity).round());

        let mut commission = match self.commission_currency {
            Some(currency) => {
                util::validate_named_cash(
                    "commission amount", &currency, self.commission,
                    DecimalRestrictions::PositiveOrZero)?
            }
            None if self.commission.is_zero() => {
                Cash::new(&self.settlement_currency, self.commission)
            },
            None => return Err!(
                "Got {} trade at {} without commission currency",
                self.symbol, format_date(conclusion_time),
            ),
        };

        if let Some(amount) = self.stamp_duty {
            let currency = self.stamp_duty_currency.ok_or_else(|| format!(
                "Got {} trade with stamp duty but without stamp duty currency", self.symbol))?;

            if currency != commission.currency {
                return Err!(concat!(
                    "Got {} trade with {} stamp duty currency which differs from broker commission ",
                    "currency ({}), which is not supported yet"
                ), self.symbol, currency, commission.currency);
            }

            commission.add_assign(util::validate_named_cash(
                "stamp duty amount", &currency, amount, DecimalRestrictions::PositiveOrZero,
            )?).unwrap();
        }

        let repo_trade = match operation {
            "Покупка" => {
                if let Some((base, _quote, _lot_size)) = forex {
                    let from = volume;
                    let to = Cash::new(base, self.quantity);
                    statement.forex_trades.push(ForexTrade::new(
                        conclusion_time.into(), from, to, commission));
                } else {
                    statement.stock_buys.push(StockBuy::new_trade(
                        &self.symbol, self.quantity, price, volume, commission,
                        conclusion_time.into(), self.execution_date));
                }
                false
            },
            "Продажа" => {
                if let Some((base, _quote, _lot_size)) = forex {
                    let from = Cash::new(base, self.quantity);
                    let to = volume;
                    statement.forex_trades.push(ForexTrade::new(
                        conclusion_time.into(), from, to, commission));
                } else {
                    statement.stock_sells.push(StockSell::new_trade(
                        &self.symbol, self.quantity, price, volume, commission,
                        conclusion_time.into(), self.execution_date, false));
                }
                false
            },
            "РЕПО 1 Продажа" | "РЕПО 2 Покупка" if forex.is_none() => {
                let amount = if operation == "РЕПО 2 Покупка" {
                    -volume
                } else {
                    volume
                };

                statement.cash_flows.push(CashFlow::new(conclusion_time.into(), amount, CashFlowType::Repo {
                    symbol: self.symbol.clone(),
                    commission
                }));

                true
            },
            _ => return Err!("Unsupported trade operation: {:?}", self.operation),
        };

        // Old statements contain a valid exchange, but later the column has been broken and now always contains the same value "Б"
        if forex.is_none() && !repo_trade && self.exchange != "Б" {
            save_instrument_exchange_info(
                &mut statement.instrument_info, &self.symbol, &self.exchange)?;
        }

        Ok(())
    }
}

#[derive(Eq, Hash, Ord, PartialEq, PartialOrd, Clone)]
pub enum TradeId {
    String(String), // Used in REPO trades
    Integer(u64),
}

impl TradeId {
    fn parse(cell: &Cell) -> GenericResult<TradeId> {
        let value = xls::get_string_cell(cell)?;
        Ok(match value.parse::<u64>() {
            Ok(id) => TradeId::Integer(id),
            Err(_) => TradeId::String(value.to_owned()),
        })
    }
}

impl Display for TradeId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let id: &dyn Display = match self {
            TradeId::Integer(value) => value,
            TradeId::String(value) => value,
        };
        id.fmt(f)
    }
}