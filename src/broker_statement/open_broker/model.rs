use std::collections::HashMap;

use chrono::Duration;
use log::{warn, error};
use num_traits::Zero;
use serde::Deserialize;

use crate::broker_statement::PartialBrokerStatement;
use crate::broker_statement::trades::{StockBuy, StockSell};
use crate::core::{EmptyResult, GenericResult};
use crate::currency::{Cash, CashAssets};
use crate::types::{Date, Decimal};
use crate::util::{self, DecimalRestrictions};

use super::parsers::{CashFlowType, deserialize_date, parse_security_description, parse_quantity};

#[derive(Deserialize)]
pub struct BrokerReport {
    #[serde(deserialize_with = "deserialize_date")]
    date_from: Date,

    #[serde(deserialize_with = "deserialize_date")]
    date_to: Date,

    #[serde(rename = "spot_account_totally")]
    account_summary: AccountSummary,

    #[serde(rename = "spot_assets")]
    assets: Assets,

    #[serde(rename = "spot_main_deals_conclusion")]
    concluded_trades: Option<ConcludedTrades>,

    #[serde(rename = "spot_main_deals_executed")]
    executed_trades: Option<ExecutedTrades>,

    #[serde(rename = "spot_non_trade_money_operations")]
    cash_flow: Option<CashFlows>,

    #[serde(rename = "spot_portfolio_security_params")]
    securities: Securities,
}

impl BrokerReport {
    pub fn parse(&self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        statement.period = Some((self.date_from, self.date_to + Duration::days(1)));
        self.account_summary.parse(statement)?;

        let securities = self.securities.parse(statement)?;
        self.assets.parse(statement, &securities)?;

        let mut trades_with_shifted_execution_date = if let Some(ref trades) = self.executed_trades {
            trades.parse()?
        } else {
            HashMap::new()
        };

        if let Some(ref trades) = self.concluded_trades {
            trades.parse(statement, &securities, &mut trades_with_shifted_execution_date)?;
        }

        if let Some(ref cash_flow) = self.cash_flow {
            cash_flow.parse(statement)?;
        }

        // Actually, we should check trade execution dates on statements merging stage when we have
        // a full view, but it would add an extra unneeded complexity to its generic logic. So now
        // we just do our best here and log found cases. If they actually will - we'll generalize
        // statements merging logic and add an ability to consider such things in the right place.
        if !trades_with_shifted_execution_date.is_empty() {
            let trade_ids = trades_with_shifted_execution_date.keys()
                .map(|trade_id| trade_id.to_string())
                .collect::<Vec<_>>();

            error!(concat!(
                "Actual execution date of the following trades differs from the planned one and ",
                "can't be fixed: {}."), trade_ids.join(", "));
        }

        Ok(())
    }
}

#[derive(Deserialize)]
struct AccountSummary {
    #[serde(rename = "item")]
    items: Vec<AccountSummaryItem>,
}

#[derive(Deserialize)]
struct AccountSummaryItem {
    #[serde(rename = "row_name")]
    name: String,

    #[serde(rename = "value")]
    amount: Decimal,
}

impl AccountSummary {
    fn parse(&self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        for item in &self.items {
            if item.name == "Входящий остаток (факт)" {
                statement.set_starting_assets(!item.amount.is_zero())?;
            }
        }

        Ok(())
    }
}

#[derive(Deserialize)]
struct Assets {
    #[serde(rename = "item")]
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    #[serde(rename = "asset_type")]
    type_: String,

    #[serde(rename = "asset_name")]
    name: String,

    #[serde(rename = "asset_code")]
    code: String,

    #[serde(rename = "opening_position_plan")]
    start_amount: Decimal,

    #[serde(rename = "closing_position_plan")]
    end_amount: Decimal,
}

impl Assets {
    fn parse(&self, statement: &mut PartialBrokerStatement, securities: &HashMap<String, String>) -> EmptyResult {
        let mut has_starting_assets = false;

        for asset in &self.assets {
            has_starting_assets |= !asset.start_amount.is_zero();

            match asset.type_.as_str() {
                "Акции" | "ПАИ" => {
                    let symbol = get_symbol(securities, &asset.name)?;
                    let amount = parse_quantity(asset.end_amount, true)?;

                    if amount != 0 {
                        if statement.open_positions.insert(symbol.clone(), amount).is_some() {
                            return Err!("Duplicated open position: {}", symbol);
                        }
                    }
                },
                "Денежные средства" => {
                    statement.cash_assets.deposit(Cash::new(&asset.code, asset.end_amount));
                },
                _ => return Err!("Unsupported asset type: {:?}", asset.type_),
            };
        }

        if has_starting_assets {
            statement.starting_assets = Some(true);
        }

        Ok(())
    }
}

#[derive(Deserialize)]
struct ConcludedTrades {
    #[serde(rename = "item")]
    trades: Vec<ConcludedTrade>,
}

#[derive(Deserialize)]
struct ConcludedTrade {
    #[serde(rename = "deal_no")]
    id: u64,

    security_name: String,

    #[serde(deserialize_with = "deserialize_date")]
    conclusion_date: Date,

    #[serde(deserialize_with = "deserialize_date")]
    execution_date: Date,

    #[serde(rename = "buy_qnty")]
    buy_quantity: Option<Decimal>,

    #[serde(rename = "sell_qnty")]
    sell_quantity: Option<Decimal>,

    #[serde(rename = "price_currency_code")]
    currency: String,

    #[serde(rename = "accounting_currency_code")]
    accounting_currency: String,

    price: Decimal,

    #[serde(rename = "broker_commission")]
    commission: Decimal,
}

impl ConcludedTrades {
    fn parse(
        &self, statement: &mut PartialBrokerStatement, securities: &HashMap<String, String>,
        trades_with_shifted_execution_date: &mut HashMap<u64, Date>,
    ) -> EmptyResult {
        for trade in &self.trades {
            let symbol = get_symbol(securities, &trade.security_name)?;
            let price = util::validate_decimal(trade.price, DecimalRestrictions::StrictlyPositive)
                .map_err(|_| format!("Invalid {} price: {}", symbol, trade.price))?.normalize();
            let commission = util::validate_decimal(trade.commission, DecimalRestrictions::PositiveOrZero)
                .map_err(|_| format!("Invalid commission: {}", trade.commission))?;

            // Just don't know which one exactly is
            if trade.currency != trade.accounting_currency {
                return Err!(
                    "Trade currency for {} is not equal to accounting currency which is not supported yet",
                     symbol);
            }

            let price = Cash::new(&trade.currency, price);
            let commission = Cash::new(&trade.accounting_currency, commission);
            let execution_date = match trades_with_shifted_execution_date.remove(&trade.id) {
                Some(execution_date) => {
                    warn!(concat!(
                        "Actual execution date of {:?} trade differs from the planned one. ",
                        "Fix execution date for this trade."
                    ), trade.id);

                    execution_date
                },
                None => trade.execution_date,
            };

            match (trade.buy_quantity, trade.sell_quantity) {
                (Some(quantity), None) => {
                    let quantity = parse_quantity(quantity, false)?;

                    statement.stock_buys.push(StockBuy::new(
                        symbol, quantity, price, commission,
                        trade.conclusion_date, execution_date));
                },
                (None, Some(quantity)) => {
                    let quantity = parse_quantity(quantity, false)?;

                    statement.stock_sells.push(StockSell::new(
                        symbol, quantity, price, commission,
                        trade.conclusion_date, execution_date, false));
                },
                _ => return Err!("Got an unexpected trade: Can't match it as buy or sell trade")
            };
        }

        Ok(())
    }
}

#[derive(Deserialize)]
struct ExecutedTrades {
    #[serde(rename = "item")]
    trades: Vec<ExecutedTrade>,
}

#[derive(Deserialize)]
struct ExecutedTrade {
    #[serde(rename = "deal_no")]
    id: u64,

    #[serde(deserialize_with = "deserialize_date")]
    plan_execution_date: Date,

    #[serde(deserialize_with = "deserialize_date")]
    fact_execution_date: Date,
}

impl ExecutedTrades {
    fn parse(&self) -> GenericResult<HashMap<u64, Date>> {
        let mut trades_with_shifted_execution_date = HashMap::new();

        for trade in &self.trades {
            if trade.fact_execution_date != trade.plan_execution_date {
                if trades_with_shifted_execution_date.insert(trade.id, trade.fact_execution_date).is_some() {
                    return Err!("Got a duplicated {:?} trade", trade.id);
                }
            }
        }

        Ok(trades_with_shifted_execution_date)
    }
}

#[derive(Deserialize)]
struct CashFlows {
    #[serde(rename = "item")]
    cash_flows: Vec<CashFlow>,
}

#[derive(Deserialize)]
struct CashFlow {
    #[serde(rename = "operation_date", deserialize_with = "deserialize_date")]
    date: Date,

    #[serde(rename = "currency_code")]
    currency: String,

    amount: Decimal,

    #[serde(rename = "comment")]
    description: String,
}

impl CashFlows {
    fn parse(&self, statement: &mut PartialBrokerStatement) -> EmptyResult {
        for cash_flow in &self.cash_flows {
            let date = cash_flow.date;
            let currency = &cash_flow.currency;
            let amount = cash_flow.amount;

            match CashFlowType::parse(&cash_flow.description)? {
                CashFlowType::Deposit => {
                    let amount = util::validate_decimal(amount, DecimalRestrictions::StrictlyPositive)
                        .map_err(|_| format!("Invalid deposit amount: {}", amount))?;

                    statement.cash_flows.push(
                        CashAssets::new_from_cash(date, Cash::new(currency, amount)));
                },
                CashFlowType::Commission => (),
            };
        }

        Ok(())
    }
}

#[derive(Deserialize)]
struct Securities {
    #[serde(rename = "item")]
    securities: Vec<Security>,
}

#[derive(Deserialize)]
struct Security {
    #[serde(rename = "security_name")]
    name: String,

    #[serde(rename = "ticker")]
    symbol: String,

    #[serde(rename = "issuer_name")]
    description: String,
}

impl Securities {
    fn parse(&self, statement: &mut PartialBrokerStatement) -> GenericResult<HashMap<String, String>> {
        let mut securities = HashMap::new();

        for security in &self.securities {
            if securities.insert(security.name.clone(), security.symbol.clone()).is_some() {
                return Err!("Duplicated security name: {:?}", security.name);
            }

            let description = parse_security_description(&security.description);
            if statement.instrument_names.insert(security.symbol.clone(), description.to_owned()).is_some() {
                return Err!("Duplicated security symbol: {}", security.symbol);
            }
        }

        Ok(securities)
    }
}

fn get_symbol<'a>(securities: &'a HashMap<String, String>, name: &str) -> GenericResult<&'a String> {
    Ok(securities.get(name).ok_or_else(|| format!(
        "Unable to find security info by its name ({:?})", name))?)
}