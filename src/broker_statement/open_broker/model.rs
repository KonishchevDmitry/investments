use std::collections::HashMap;

use chrono::Duration;
use num_traits::Zero;

use broker_statement::{BrokerStatementBuilder, StockBuy};
use core::{EmptyResult, GenericResult};
use currency::Cash;
use types::{Date, Decimal};
use util::{self, DecimalRestrictions};

use super::parsers::{deserialize_date, parse_security_description, parse_quantity};

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
    trades: Trades,

    #[serde(rename = "spot_portfolio_security_params")]
    securities: Securities,
}

impl BrokerReport {
    pub fn parse(&self, statement: &mut BrokerStatementBuilder) -> EmptyResult {
        statement.period = Some((self.date_from, self.date_to + Duration::days(1)));
        self.account_summary.parse(statement)?;

        let securities = self.securities.parse(statement)?;
        self.assets.parse(statement, &securities)?;
        self.trades.parse(statement, &securities)?;

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
    fn parse(&self, statement: &mut BrokerStatementBuilder) -> EmptyResult {
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
    fn parse(&self, statement: &mut BrokerStatementBuilder, securities: &HashMap<String, String>) -> EmptyResult {
        let mut has_starting_assets = false;

        for asset in &self.assets {
            has_starting_assets |= !asset.start_amount.is_zero();

            match asset.type_.as_str() {
                "ПАИ" => {
                    let symbol = get_symbol(securities, &asset.name)?;
                    let amount = parse_quantity(asset.end_amount)?;

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
struct Trades {
    #[serde(rename = "item")]
    trades: Vec<Trade>,
}

#[derive(Deserialize)]
struct Trade {
    #[serde(rename = "security_name")]
    name: String,

    // TODO: Commission date must be equal to conclusion_date - not to execution_date
    // #[serde(deserialize_with = "deserialize_date")]
    // conclusion_date: Date,

    // TODO: Compare the trades with <spot_main_deals_executed> and check execution_date?
    #[serde(deserialize_with = "deserialize_date")]
    execution_date: Date,

    #[serde(rename = "buy_qnty")]
    buy_quantity: Option<Decimal>,

    #[serde(rename = "price_currency_code")]
    currency: String,

    #[serde(rename = "accounting_currency_code")]
    accounting_currency: String,

    price: Decimal,

    #[serde(rename = "broker_commission")]
    commission: Decimal,
}

impl Trades {
    fn parse(&self, statement: &mut BrokerStatementBuilder, securities: &HashMap<String, String>) -> EmptyResult {
        for trade in &self.trades {
            let symbol = get_symbol(securities, &trade.name)?.clone();
            let price = util::validate_decimal(trade.price, DecimalRestrictions::StrictlyPositive)
                .map_err(|_| format!("Invalid {} price: {}", symbol, trade.price))?;
            let commission = util::validate_decimal(trade.commission, DecimalRestrictions::PositiveOrZero)
                .map_err(|_| format!("Invalid commission: {}", trade.commission))?;

            // Just don't know which one exactly is
            if trade.currency != trade.accounting_currency {
                return Err!(
                    "Trade currency for {} is not equal to accounting currency which is not supported yet", symbol);
            }

            let price = Cash::new(&trade.currency, price);
            let commission = Cash::new(&trade.accounting_currency, commission);

            match trade.buy_quantity {
                Some(quantity) => {
                    let quantity = parse_quantity(quantity)?;

                    statement.stock_buys.push(StockBuy {
                        date: trade.execution_date,
                        symbol: symbol,
                        quantity: quantity,
                        price: price,
                        commission: commission,
                        sold: 0,
                    });
                },
                // TODO: Selling support
                _ => return Err!("Stock selling is not supported yet")
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
    fn parse(&self, statement: &mut BrokerStatementBuilder) -> GenericResult<HashMap<String, String>> {
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