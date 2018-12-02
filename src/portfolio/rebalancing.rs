use std::collections::HashSet;

use log;
use num_traits::{FromPrimitive, ToPrimitive, Zero};

use brokers::BrokerInfo;
use core::{GenericResult, EmptyResult};
use currency::converter::CurrencyConverter;
use types::Decimal;
use util;

use super::asset_allocation::{Portfolio, AssetAllocation, Holding, StockHolding};

#[derive(Clone, Copy)]
enum Action {
    Sell,
    Buy,
}

// FIXME: implement
pub fn rebalance_portfolio(portfolio: &mut Portfolio, converter: &CurrencyConverter) -> GenericResult<Decimal> {
    // The first step is bottom-up and calculates strict limits on asset min/max value
    calculate_restrictions(&mut portfolio.assets);

    // The second step is top-down and tries to apply the specified weights and limits calculated in
    // the first step to the actual free assets
    debug!("");
    debug!("Calculating assets target value...");
    calculate_target_value(
        &portfolio.name, &mut portfolio.assets, portfolio.total_value - portfolio.min_cash_assets,
        portfolio.min_trade_volume);

    // The next step is bottom-up and calculates the result of the rebalancing operation
    let (current_value, commissions) = calculate_result_value(
        &mut portfolio.assets, &portfolio.broker, &portfolio.currency, converter)?;

    portfolio.total_value -= commissions;
    portfolio.target_cash_assets = portfolio.total_value - current_value;

    // FIXME
    distribute_cash_assets(portfolio, converter)?;

    Ok(commissions) // FIXME: print + free assets -> free assets
}

fn calculate_restrictions(assets: &mut Vec<AssetAllocation>) -> (Decimal, Option<Decimal>) {
    let mut total_min_value = dec!(0);
    let mut total_max_value = dec!(0);
    let mut all_with_max_value = true;

    for asset in assets {
        let (min_value, max_value) = match &mut asset.holding {
            Holding::Group(assets) => calculate_restrictions(assets),
            Holding::Stock(_) => {
                let min_value = if asset.restrict_selling.unwrap_or(false) {
                    asset.current_value
                } else {
                    dec!(0)
                };

                let max_value = if asset.restrict_buying.unwrap_or(false) {
                    Some(asset.current_value)
                } else {
                    None
                };

                (min_value, max_value)
            },
        };

        asset.min_value = min_value;
        asset.max_value = max_value;

        total_min_value += min_value;

        if let Some(max_value) = max_value {
            total_max_value += max_value;
        } else {
            all_with_max_value = false;
        }
    }

    let total_max_value = if all_with_max_value {
        Some(total_max_value)
    } else {
        None
    };

    (total_min_value, total_max_value)
}

fn calculate_target_value(
    name: &str, assets: &mut Vec<AssetAllocation>, target_total_value: Decimal,
    min_trade_volume: Decimal
) -> Decimal {
    debug!("{name}:", name=name);
    debug!("* Initial target values:");
    for asset in assets.iter_mut() {
        asset.target_value = target_total_value * asset.expected_weight;
        debug!("  * {name}: {current_value} -> {target_value}",
               name=asset.full_name(), current_value=asset.current_value.normalize(),
               target_value=asset.target_value.normalize());
    }

    let mut balance = dec!(0);

    debug!("* Rounding:");

    for asset in assets.iter_mut() {
        let mut difference = asset.target_value - asset.current_value;

        if let Holding::Stock(ref holding) = asset.holding {
            difference = util::round_to(difference / holding.price, 0) * holding.price;
        }

        if difference.abs() < min_trade_volume {
            difference = dec!(0);
        }

        let target_value = (asset.current_value + difference).normalize();

        if target_value != asset.target_value {
            debug!("  * {name}: {target_value} -> {corrected_target_value}",
                name=asset.full_name(), target_value=asset.target_value.normalize(),
                corrected_target_value=target_value.normalize());

            balance += asset.target_value - target_value;
            asset.target_value = target_value;
        }
    }

    debug!("* Rebalancing:");

    // First process assets with max value limit to release free cash assets
    for asset in assets.iter_mut() {
        let max_value = match asset.max_value {
            Some(max_value) => max_value,
            None => continue,
        };

        if asset.target_value > max_value {
            balance += asset.target_value - max_value;
            asset.target_value = max_value;
            asset.buy_blocked = true;

            debug!("  * {name}: buying is blocked at {value}",
                   name=asset.full_name(), value=max_value.normalize());
        }
    }

    // Then process assets with min value limit to adapt to restrictions provided by the caller
    for asset in assets.iter_mut() {
        let min_value = asset.min_value;

        if asset.target_value < min_value {
            balance += asset.target_value - min_value;
            asset.target_value = min_value;
            asset.sell_blocked = true;

            debug!("  * {name}: selling is blocked at {value}",
                   name=asset.full_name(), value=min_value.normalize());
        }
    }

    struct PossibleTrade {
        index: usize,
        volume: Decimal,
        result: Decimal,
    }

    let balance_before_distribution = balance;
    let mut target_values_before_distribution = Vec::new();

    if log_enabled!(log::Level::Debug) {
        for asset in assets.iter() {
            target_values_before_distribution.push(asset.target_value);
        }
    }

    for action in &[Action::Sell, Action::Buy] {
        let mut correctable_holdings: HashSet<usize> = (0..assets.len()).collect();

        while match action {
            Action::Sell => balance.is_sign_negative(),
            Action::Buy => balance.is_sign_positive(),
        } {
            // FIXME: Deduplicate code
            let mut best_trade: Option<PossibleTrade> = None;

            for index in correctable_holdings.iter().cloned().collect::<Vec<_>>() {
                let asset = &mut assets[index];

                let trade_volume = match action {
                    Action::Sell => calculate_min_sell_volume(asset, min_trade_volume),
                    Action::Buy => {
                        match calculate_min_buy_volume(asset, min_trade_volume) {
                            Some(trade_volume) if trade_volume <= balance => Some(trade_volume),
                            _ => None,
                        }
                    },
                };

                let trade_volume = match trade_volume {
                    Some(trade_volume) => match action {
                        Action::Sell => -trade_volume,
                        Action::Buy => trade_volume,
                    },
                    None => {
                        correctable_holdings.remove(&index);
                        continue
                    },
                };

                let expected_value = target_total_value * asset.expected_weight;  // FIXME: expected total value?
                let target_value = asset.target_value + trade_volume;

                let possible_trade = PossibleTrade {
                    index: index,
                    volume: trade_volume,
                    result: target_value / expected_value,  // FIXME: Zero division
                };

                // FIXME: Deduplicate code
                best_trade = Some(match best_trade {
                    Some(best_trade) => {
                        if match action {
                            Action::Sell => possible_trade.result > best_trade.result,
                            Action::Buy => possible_trade.result < best_trade.result,
                        } {
                            possible_trade
                        } else {
                            best_trade
                        }
                    }
                    None => possible_trade,
                });
            }

            let trade = match best_trade {
                Some(best_trade) => best_trade,
                None => break,
            };

            let asset = &mut assets[trade.index];
            asset.target_value += trade.volume;
            balance -= trade.volume;
        }
    }

    for asset in assets.iter_mut() {
        let asset_name = asset.full_name();

        if let Holding::Group(ref mut holdings) = asset.holding {
            let group_balance = calculate_target_value(
                &asset_name, holdings, asset.target_value, min_trade_volume);

            asset.target_value -= balance;
            balance += group_balance;
        }
    }

    if log_enabled!(log::Level::Debug) {
        debug!("{name}:", name=name);
        debug!("* Distribution: {prev_balance} -> {balance}:",
               prev_balance=balance_before_distribution.normalize(), balance=balance.normalize());

        for (index, asset) in assets.iter().enumerate() {
            let prev_target_value = target_values_before_distribution[index];
            if prev_target_value != asset.target_value {
                debug!("  * {name}: {prev_target_value} -> {target_value}",
                       name=asset.full_name(), prev_target_value=prev_target_value.normalize(),
                       target_value=asset.target_value.normalize())
            }
        }
    }

    return balance
}

fn calculate_result_value(
    assets: &mut Vec<AssetAllocation>, broker: &BrokerInfo,
    currency: &str, converter: &CurrencyConverter
) -> GenericResult<(Decimal, Decimal)> {
    let mut total_value = dec!(0);
    let mut total_commissions = dec!(0);

    for asset in assets.iter_mut() {
        let name = asset.full_name();

        let (value, commissions) = match asset.holding {
            Holding::Stock(ref mut holding) => {
                assert_eq!(holding.target_shares, holding.current_shares);

                let commission = change_to(
                    &name, holding, asset.target_value, broker, currency, converter)?;

                (asset.target_value, commission)
            },
            Holding::Group(ref mut holdings) => {
                calculate_result_value(holdings, broker, currency, converter)?
            },
        };

        total_value += value;
        total_commissions += commissions;
    }

    Ok((total_value, total_commissions))
}

// FIXME
struct PossibleDeepTrade {
    path: Vec<usize>,
    volume: Decimal,
    result: Decimal,
}

fn distribute_cash_assets(portfolio: &mut Portfolio, converter: &CurrencyConverter) -> EmptyResult {
    debug!("");
    debug!("Cash assets distribution:");

    for action in [Action::Sell, Action::Buy].iter().cloned() {
        loop {
            let free_cash_assets = portfolio.target_cash_assets - portfolio.min_cash_assets;

            if !match action {
                Action::Sell => free_cash_assets.is_sign_negative(),
                Action::Buy => free_cash_assets.is_sign_positive(),
            } {
                break;
            }

            let expected_total_value = portfolio.total_value - portfolio.min_cash_assets;

            let trade = find_assets_for_cash_distribution(
                action, &portfolio.assets, expected_total_value, free_cash_assets,
                portfolio.min_trade_volume);

            let trade = match trade {
                Some(trade) => trade,
                None => break,
            };

            portfolio.target_cash_assets -= trade.volume;
            // FIXME: to portfolio
            let commission = process_trade(
                &mut portfolio.assets, trade, &portfolio.broker, &portfolio.currency, converter)?;
            portfolio.target_cash_assets -= commission;
        }
    }

    Ok(())
}

fn find_assets_for_cash_distribution(
    action: Action, assets: &Vec<AssetAllocation>, expected_total_value: Decimal,
    cash_assets: Decimal, min_trade_volume: Decimal
) -> Option<PossibleDeepTrade> {
    let mut best_trade: Option<PossibleDeepTrade> = None;

    for (index, asset) in assets.iter().enumerate() {
        let expected_value = expected_total_value * asset.expected_weight;

        let trade = match asset.holding {
            Holding::Stock(_) => {
                calculate_min_trade_volume(
                    action, asset, expected_value, cash_assets, min_trade_volume)
            },
            Holding::Group(ref holdings) => {
                find_assets_for_cash_distribution(
                    action, holdings, expected_value, cash_assets, min_trade_volume)
            },
        };

        let trade = match trade {
            Some(mut trade) => {
                trade.path.push(index);
                trade
            },
            None => continue,
        };

        best_trade = Some(get_best_trade(action, best_trade, trade));
    }

    best_trade
}

fn process_trade(
    assets: &mut Vec<AssetAllocation>, mut trade: PossibleDeepTrade,
    broker: &BrokerInfo, currency: &str, converter: &CurrencyConverter
) -> GenericResult<Decimal> {
    let index = trade.path.pop().unwrap();
    let asset = &mut assets[index];

    let name = asset.full_name();
    let target_value = asset.target_value + trade.volume;

    let commission = match asset.holding {
        Holding::Stock(ref mut holding) => {
            assert!(trade.path.is_empty());

            debug!("* {name}: {prev_target_value} -> {target_value}",
                   name=name, prev_target_value=asset.target_value.normalize(),
                   target_value=target_value.normalize());

            change_to(&name, holding, target_value, broker, currency, converter)?
        },
        Holding::Group(ref mut holdings) => {
            process_trade(holdings, trade, broker, currency, converter)?
        },
    };

    asset.target_value = target_value;

    Ok(commission)
}

fn calculate_min_trade_volume(
    action: Action, asset: &AssetAllocation, expected_value: Decimal,
    cash_assets: Decimal, min_trade_volume: Decimal
) -> Option<PossibleDeepTrade> {
    let trade_volume = match action {
        Action::Sell => calculate_min_sell_volume(asset, min_trade_volume),
        Action::Buy => {
            match calculate_min_buy_volume(asset, min_trade_volume) {
                Some(trade_volume) if trade_volume <= cash_assets => Some(trade_volume),
                _ => None,
            }
        },
    };

    let trade_volume = match trade_volume {
        Some(trade_volume) => match action {
            Action::Sell => -trade_volume,
            Action::Buy => trade_volume,
        },
        None => return None,
    };

    let target_value = asset.target_value + trade_volume;
    let result = if expected_value.is_zero() {
        if target_value.is_zero() {
            dec!(0)
        } else {
            Decimal::max_value()
        }
    } else {
        target_value / expected_value
    };

    Some(PossibleDeepTrade {
        path: Vec::new(),
        volume: trade_volume,
        result: result
    })
}

fn get_best_trade(action: Action, best_trade: Option<PossibleDeepTrade>, trade: PossibleDeepTrade) -> PossibleDeepTrade {
    match best_trade {
        Some(best_trade) => {
            if best_trade.result == trade.result {
                if best_trade.volume <= trade.volume {
                    best_trade
                } else {
                    trade
                }
            } else {
                if match action {
                    Action::Sell => best_trade.result > trade.result,
                    Action::Buy => best_trade.result < trade.result,
                } {
                    best_trade
                } else {
                    trade
                }
            }
        },
        None => trade,
    }
}

fn change_to(
    name: &str, holding: &mut StockHolding, target_value: Decimal,
    broker: &BrokerInfo, currency: &str, converter: &CurrencyConverter
) -> GenericResult<Decimal> {
    let target_shares_fractional = target_value / holding.price;

    let target_shares = target_shares_fractional.to_u32().unwrap();
    assert_eq!(target_shares_fractional, Decimal::from_u32(target_shares).unwrap());

    let prev_target_shares = holding.target_shares;
    let paid_commission = calculate_target_commission(
        name, holding, prev_target_shares, broker, currency, converter)?;

    let current_commission = calculate_target_commission(
        name, holding, target_shares, broker, currency, converter)?;

    holding.target_shares = target_shares;

    Ok(current_commission - paid_commission)
}

fn calculate_target_commission(
    name: &str, holding: &mut StockHolding, target_shares: u32,
    broker: &BrokerInfo, currency: &str, converter: &CurrencyConverter
) -> GenericResult<Decimal> {
    if target_shares == holding.current_shares {
        return Ok(dec!(0))
    }

    let shares = if target_shares > holding.current_shares {
        target_shares - holding.current_shares
    } else {
        holding.current_shares - target_shares
    };

    let commission = broker.get_trade_commission(shares, holding.cash_price)
        .map_err(|e| format!("{}: {}", name, e))?;

    Ok(converter.convert_to(util::today(), commission, currency)?)
}

fn calculate_min_sell_volume(asset: &AssetAllocation, min_trade_volume: Decimal) -> Option<Decimal> {
    let trade_granularity = get_trade_granularity(asset);

    let trade_volume = if asset.target_value <= asset.current_value {
        // target <= current

        if asset.target_value <= asset.current_value - min_trade_volume {
            // trade < target <= min < current
            trade_granularity
        } else {
            // trade <= min < target <= current
            round_min_trade_volume(
                min_trade_volume - (asset.current_value - asset.target_value),
                trade_granularity
            )
        }
    } else {
        // current < target

        if asset.target_value - trade_granularity >= asset.current_value + min_trade_volume {
            // current < min <= trade < target
            trade_granularity
        } else {
            // trade <= min < current < target
            round_min_trade_volume(
                asset.target_value - (asset.current_value - min_trade_volume),
                trade_granularity,
            )
        }
    };

    if asset.target_value - trade_volume < asset.min_value {
        return None
    }

    Some(trade_volume)
}

fn calculate_min_buy_volume(asset: &AssetAllocation, min_trade_volume: Decimal) -> Option<Decimal> {
    let trade_granularity = get_trade_granularity(asset);

    let trade_volume = if asset.target_value >= asset.current_value {
        // current <= target

        if asset.target_value >= asset.current_value + min_trade_volume {
            // current < min <= target < trade
            trade_granularity
        } else {
            // current <= target < min <= trade
            round_min_trade_volume(
                min_trade_volume - (asset.target_value - asset.current_value),
                trade_granularity,
            )
        }
    } else {
        // target < current

        if asset.target_value + trade_granularity <= asset.current_value - min_trade_volume {
            // target < trade <= min < current
            trade_granularity
        } else {
            // target < current < min <= trade
            round_min_trade_volume(
                asset.current_value + min_trade_volume - asset.target_value,
                trade_granularity
            )
        }
    };

    if let Some(max_value) = asset.max_value {
        if asset.target_value + trade_volume > max_value {
            return None;
        }
    }

    Some(trade_volume)
}

fn get_trade_granularity(asset: &AssetAllocation) -> Decimal {
    match asset.holding {
        Holding::Stock(ref holding) => holding.price,
        Holding::Group(_) => dec!(1), // FIXME
    }
}

fn round_min_trade_volume(volume: Decimal, granularity: Decimal) -> Decimal {
    (volume / granularity).ceil() * granularity
}