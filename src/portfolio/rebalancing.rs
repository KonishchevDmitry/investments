use std::collections::HashSet;

use log::{self, log_enabled, debug};
use num_traits::ToPrimitive;

use crate::brokers::BrokerInfo;
use crate::commissions::CommissionCalc;
use crate::core::{GenericResult, EmptyResult};
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverterRc;
use crate::time;
use crate::types::{Decimal, TradeType};
use crate::util;

use super::asset_allocation::{Portfolio, AssetAllocation, Holding, StockHolding};

pub fn rebalance_portfolio(portfolio: &mut Portfolio, converter: CurrencyConverterRc) -> EmptyResult {
    let portfolio_info = PortfolioInfo::new(portfolio);

    // The first step is bottom-up and calculates strict limits on asset min/max value
    calculate_restrictions(&mut portfolio.assets);

    // The second step is top-down and tries to apply the specified weights and limits calculated in
    // the first step to the current assets
    debug!("");
    debug!("Calculating assets target value...");
    AssetGroupRebalancer::rebalance(
        &portfolio.name, &mut portfolio.assets,
        portfolio.target_net_value - portfolio.min_cash_assets,
        portfolio.min_trade_volume);

    // The next step is bottom-up and calculates the result of the previous step
    let target_value = calculate_result_value(&portfolio_info, converter.clone(), &mut portfolio.assets)?;
    portfolio.target_cash_assets = portfolio.target_net_value - target_value;

    let (interim_trade_commissions, interim_additional_commissions) =
        calculate_total_commissions(portfolio, converter.clone())?;

    let interim_total_commissions = interim_trade_commissions + interim_additional_commissions;
    assert!(portfolio.commissions.is_zero());
    portfolio.change_commission(interim_total_commissions);

    // The rebalancing logic is relatively inaccurate because it distributes funds only inside of
    // one group which leads to accumulation of free cash / debt from each asset group. Also it
    // can't take into account accumulated commissions. The following step operates on all levels of
    // asset tree and distributes accumulated free cash / debt in the optimal way according to asset
    // allocation configuration.
    distribute_cash_assets(portfolio, &portfolio_info, converter.clone())?;

    let (trade_commissions, additional_commissions) = calculate_total_commissions(portfolio, converter)?;
    assert_eq!(
        portfolio.commissions - interim_total_commissions,
        trade_commissions - interim_trade_commissions,
    );
    portfolio.change_commission(additional_commissions - interim_additional_commissions);

    Ok(())
}

fn calculate_restrictions(assets: &mut Vec<AssetAllocation>) -> (Decimal, Option<Decimal>) {
    let mut total_min_value = dec!(0);
    let mut total_max_value = dec!(0);
    let mut all_with_max_value = true;

    for asset in assets {
        let (min_value, max_value) = match &mut asset.holding {
            Holding::Group(holdings) => calculate_restrictions(holdings),
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

        // Treat zero weight as a special case of restrictions (deprecated asset)
        if asset.expected_weight.is_zero() {
            propagate_zero_weight(asset)
        }

        total_min_value += asset.min_value;

        if let Some(max_value) = asset.max_value {
            assert!(max_value >= asset.min_value);
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

fn propagate_zero_weight(asset: &mut AssetAllocation) {
    if asset.min_value.is_zero() {
        if let Holding::Group(ref mut holdings) = asset.holding {
            for holding in holdings {
                assert!(holding.min_value.is_zero());
                propagate_zero_weight(holding);
            }
        }
    } else if let Some(max_value) = asset.max_value {
        assert_eq!(max_value, asset.min_value);
    }

    asset.max_value = Some(asset.min_value);
}

struct AssetGroupRebalancer<'a> {
    name: &'a str,
    assets: &'a mut Vec<AssetAllocation>,
    target_total_value: Decimal,
    min_trade_volume: Decimal,
    balance: Decimal,
}

impl<'a> AssetGroupRebalancer<'a> {
    fn rebalance(
        name: &str, assets: &mut Vec<AssetAllocation>, target_total_value: Decimal,
        min_trade_volume: Decimal
    ) -> Decimal {
        let mut rebalancer = AssetGroupRebalancer {
            name, assets, target_total_value, min_trade_volume,
            balance: dec!(0),
        };

        debug!("{name}:", name=name);
        rebalancer.calculate_initial_target_values();
        rebalancer.apply_restrictions();
        rebalancer.correct_balance();
        rebalancer.propagate_changes();
        rebalancer.balance
    }

    fn calculate_initial_target_values(&mut self) {
        debug!("* Initial target values:");
        for asset in self.assets.iter_mut() {
            asset.target_value = self.target_total_value * asset.expected_weight;
            debug!("  * {name}: {current_value} -> {target_value}",
                   name=asset.full_name(), current_value=asset.current_value.normalize(),
                   target_value=asset.target_value.normalize());
        }

        let state = self.get_current_state();

        for asset in self.assets.iter_mut() {
            let mut difference = asset.target_value - asset.current_value;

            if let Holding::Stock(ref holding) = asset.holding {
                let trade_granularity = holding.trade_granularity();

                difference = util::round(difference / trade_granularity, 0) * trade_granularity;
                if difference.is_sign_negative() && -difference > asset.current_value {
                    difference += trade_granularity
                }
            }

            if difference.abs() < self.min_trade_volume {
                difference = dec!(0);
            }

            let target_value = asset.current_value + difference;
            self.balance += asset.target_value - target_value;
            asset.target_value = target_value;
        }

        self.log_state_changes("Rounding", state);
    }

    fn apply_restrictions(&mut self) {
        let state = self.get_current_state();

        let mut logged = false;
        let mut log_restriction_applying = |name: &str, action: &str, value: Decimal| {
            if !logged {
                debug!("* Applying restrictions:");
                logged = true;
            }

            debug!("  * {name}: {action} is blocked at {value}",
                   name=name, action=action, value=value.normalize());
        };

        for asset in self.assets.iter_mut() {
            if let Some(max_value) = asset.max_value {
                if asset.target_value > max_value {
                    if asset.restrict_buying.unwrap_or(false) && asset.target_value > asset.current_value {
                        log_restriction_applying(&asset.full_name(), "buying", max_value);
                        asset.buy_blocked = true;
                    }

                    self.balance += asset.target_value - max_value;
                    asset.target_value = max_value;
                }
            }

            let min_value = asset.min_value;

            if asset.target_value < min_value {
                log_restriction_applying(&asset.full_name(), "selling", min_value);
                asset.sell_blocked = true;

                self.balance += asset.target_value - min_value;
                asset.target_value = min_value;
            }
        }

        self.log_state_changes("Restrictions applying", state);
    }

    fn correct_balance(&mut self) {
        let state = self.get_current_state();

        for trade_type in [TradeType::Sell, TradeType::Buy].iter().cloned() {
            let mut correctable_assets: HashSet<usize> = (0..self.assets.len()).collect();

            while match trade_type {
                TradeType::Sell => self.balance.is_sign_negative(),
                TradeType::Buy => self.balance.is_sign_positive(),
            } {
                let mut best_trade: Option<PossibleTrade> = None;

                for index in correctable_assets.iter().cloned().collect::<Vec<_>>() {
                    let asset = &mut self.assets[index];
                    let expected_value = self.target_total_value * asset.expected_weight;
                    let possible_trade = calculate_min_trade_volume(
                        trade_type, asset, expected_value, self.balance, self.min_trade_volume);

                    match possible_trade {
                        Some(mut trade) => {
                            trade.path.push(index);
                            best_trade = Some(get_best_trade(trade_type, best_trade, trade));
                        },
                        None => {
                            correctable_assets.remove(&index);
                        },
                    };
                }

                let trade = match best_trade {
                    Some(trade) => trade,
                    None => break,
                };

                assert_eq!(trade.path.len(), 1);
                let asset = &mut self.assets[*trade.path.last().unwrap()];

                asset.target_value += trade.volume;
                self.balance -= trade.volume;
            }
        }

        self.log_state_changes("Balance correction", state);
    }

    fn propagate_changes(&mut self) {
        let state = self.get_current_state();
        let mut propagated = false;

        for asset in self.assets.iter_mut() {
            let asset_name = asset.full_name();

            if let Holding::Group(ref mut holdings) = asset.holding {
                let balance = AssetGroupRebalancer::rebalance(
                    &asset_name, holdings, asset.target_value, self.min_trade_volume);

                asset.target_value -= balance;
                self.balance += balance;
                propagated = true;
            }
        }

        if propagated {
            debug!("{name}:", name=self.name);
            self.log_state_changes("Target value change propagation", state);
        }
    }

    fn get_current_state(&self) -> Option<AssetGroupRebalancingState> {
        if !log_enabled!(log::Level::Debug) {
            return None;
        }

        let mut state = AssetGroupRebalancingState {
            target_values: Vec::new(),
            balance: self.balance,
        };

        for asset in self.assets.iter() {
            state.target_values.push(asset.target_value);
        }

        Some(state)
    }

    fn log_state_changes(&self, changes_summary: &str, prev_state: Option<AssetGroupRebalancingState>) {
        let prev_state = match prev_state {
            Some(state) => state,
            None => return,
        };

        let changed = self.balance != prev_state.balance ||
            self.assets.iter().enumerate().any(|item| {
                let (index, asset) = item;
                asset.target_value != prev_state.target_values[index]
            });

        if !changed {
            return;
        }

        debug!("* {changes_summary} ({prev_balance} -> {balance}):",
               changes_summary=changes_summary, prev_balance=prev_state.balance.normalize(),
               balance=self.balance.normalize());

        for (index, asset) in self.assets.iter().enumerate() {
            let prev_target_value = prev_state.target_values[index];
            if prev_target_value != asset.target_value {
                debug!("  * {name}: {prev_target_value} -> {target_value}",
                       name=asset.full_name(), prev_target_value=prev_target_value.normalize(),
                       target_value=asset.target_value.normalize())
            }
        }
    }
}

struct PortfolioInfo {
    broker: BrokerInfo,
    currency: String,
    net_value: Decimal,
}

impl PortfolioInfo {
    fn new(portfolio: &Portfolio) -> PortfolioInfo {
        PortfolioInfo {
            broker: portfolio.broker.clone(),
            currency: portfolio.currency.clone(),
            net_value: portfolio.current_net_value,
        }
    }
}

struct AssetGroupRebalancingState {
    target_values: Vec<Decimal>,
    balance: Decimal,
}

fn calculate_result_value(
    portfolio: &PortfolioInfo, converter: CurrencyConverterRc,
    assets: &mut Vec<AssetAllocation>,
) -> GenericResult<Decimal> {
    let mut total_value = dec!(0);

    for asset in assets.iter_mut() {
        let name = asset.full_name();

        total_value += match asset.holding {
            Holding::Stock(ref mut holding) => {
                assert_eq!(holding.target_shares, holding.current_shares);
                change_to(portfolio, converter.clone(), &name, holding, asset.target_value)?;
                asset.target_value
            },
            Holding::Group(ref mut holdings) => {
                calculate_result_value(portfolio, converter.clone(), holdings)?
            },
        };
    }

    Ok(total_value)
}

fn distribute_cash_assets(
    portfolio: &mut Portfolio, portfolio_info: &PortfolioInfo, converter: CurrencyConverterRc,
) -> EmptyResult {
    debug!("");
    debug!("Cash assets distribution:");

    for trade_type in [TradeType::Sell, TradeType::Buy].iter().cloned() {
        loop {
            let free_cash_assets = portfolio.target_cash_assets - portfolio.min_cash_assets;

            if !match trade_type {
                TradeType::Sell => free_cash_assets.is_sign_negative(),
                TradeType::Buy => free_cash_assets.is_sign_positive(),
            } {
                break;
            }

            let expected_net_value = portfolio.target_net_value - portfolio.min_cash_assets;

            let trade = find_assets_for_cash_distribution(
                trade_type, &portfolio.assets, expected_net_value, free_cash_assets,
                portfolio.min_trade_volume);

            let trade = match trade {
                Some(trade) => trade,
                None => break,
            };

            portfolio.target_cash_assets -= trade.volume;

            let commission = process_trade(
                portfolio_info, converter.clone(), &mut portfolio.assets, trade)?;
            portfolio.change_commission(commission);
        }
    }

    Ok(())
}

struct PossibleTrade {
    path: Vec<usize>,
    volume: Decimal,
    result: f64,
}

fn find_assets_for_cash_distribution(
    trade_type: TradeType, assets: &[AssetAllocation], expected_total_value: Decimal,
    cash_assets: Decimal, min_trade_volume: Decimal
) -> Option<PossibleTrade> {
    let mut best_trade: Option<PossibleTrade> = None;

    for (index, asset) in assets.iter().enumerate() {
        let expected_value = expected_total_value * asset.expected_weight;

        let trade = match asset.holding {
            Holding::Stock(_) => {
                calculate_min_trade_volume(
                    trade_type, asset, expected_value, cash_assets, min_trade_volume)
            },
            Holding::Group(ref holdings) => {
                let mut trade = find_assets_for_cash_distribution(
                    trade_type, holdings, expected_value, cash_assets, min_trade_volume);

                if let Some(ref mut trade) = trade {
                    trade.result = calculate_trade_result(
                        expected_value, asset.target_value, trade.volume);
                }

                trade
            },
        };

        let trade = match trade {
            Some(mut trade) => {
                trade.path.push(index);
                trade
            },
            None => continue,
        };

        best_trade = Some(get_best_trade(trade_type, best_trade, trade));
    }

    best_trade
}

fn process_trade(
    portfolio: &PortfolioInfo, converter: CurrencyConverterRc,
    assets: &mut Vec<AssetAllocation>, mut trade: PossibleTrade,
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

            change_to(portfolio, converter, &name, holding, target_value)?
        },
        Holding::Group(ref mut holdings) => {
            process_trade(portfolio, converter, holdings, trade)?
        },
    };

    asset.target_value = target_value;

    Ok(commission)
}

fn calculate_min_trade_volume(
    trade_type: TradeType, asset: &AssetAllocation, expected_value: Decimal,
    cash_assets: Decimal, min_trade_volume: Decimal
) -> Option<PossibleTrade> {
    let trade_volume = match trade_type {
        TradeType::Sell => calculate_min_sell_volume(asset, min_trade_volume),
        TradeType::Buy => {
            match calculate_min_buy_volume(asset, min_trade_volume) {
                Some(trade_volume) if trade_volume <= cash_assets => Some(trade_volume),
                _ => None,
            }
        },
    };

    let trade_volume = match trade_volume {
        Some(trade_volume) => match trade_type {
            TradeType::Sell => -trade_volume,
            TradeType::Buy => trade_volume,
        },
        None => return None,
    };

    Some(PossibleTrade {
        path: Vec::new(),
        volume: trade_volume,
        result: calculate_trade_result(expected_value, asset.target_value, trade_volume),
    })
}

fn calculate_trade_result(expected_value: Decimal, target_value: Decimal, trade_volume: Decimal) -> f64 {
    // We use f64 here instead of Decimal due to https://github.com/paupino/rust-decimal/issues/276
    // Without this workaround the division takes too much time making this function the hottest
    // point of all rebalancing logic and slowing down it from fraction of second to minutes (when
    // building with devel profile) on big unbalanced portfolios with fractional shares or low cost
    // stocks.

    let result_value = target_value + trade_volume;

    if expected_value.is_zero() {
        if result_value.is_zero() {
            1.0
        } else {
            f64::MAX
        }
    } else {
        let result_value = result_value.to_f64().unwrap();
        let expected_value = expected_value.to_f64().unwrap();
        result_value / expected_value
    }
}

fn get_best_trade(trade_type: TradeType, best_trade: Option<PossibleTrade>, trade: PossibleTrade) -> PossibleTrade {
    match best_trade {
        Some(best_trade) => {
            #[allow(clippy::float_cmp)]
            if best_trade.result == trade.result {
                if best_trade.volume <= trade.volume {
                    best_trade
                } else {
                    trade
                }
            } else if match trade_type {
                TradeType::Sell => best_trade.result > trade.result,
                TradeType::Buy => best_trade.result < trade.result,
            } {
                best_trade
            } else {
                trade
            }
        },
        None => trade,
    }
}

fn change_to(
    portfolio: &PortfolioInfo, converter: CurrencyConverterRc,
    name: &str, holding: &mut StockHolding, target_value: Decimal,
) -> GenericResult<Decimal> {
    let calculate_commission = |target_shares| -> GenericResult<Decimal> {
        // We use a temporary calculator because we can work only with non-accumulated commissions
        // here since the returned commission difference can't be calculated for accumulated
        // commissions. Accumulated commissions will be calculated separately. This logic is used to
        // increase rebalancing accuracy.

        let mut commission_calc = CommissionCalc::new(
            converter.clone(), portfolio.broker.commission_spec.clone(),
            Cash::new(&portfolio.currency, portfolio.net_value))?;

        calculate_target_commission(
            name, holding, target_shares, &mut commission_calc,
            &portfolio.currency, converter.clone())
    };

    let target_shares = (target_value / holding.price).normalize();
    assert!(util::decimal_precision(target_shares) <= util::decimal_precision(holding.current_shares));

    let paid_commission = calculate_commission(holding.target_shares)?;
    let current_commission = calculate_commission(target_shares)?;
    holding.target_shares = target_shares;

    Ok(current_commission - paid_commission)
}

fn calculate_target_commission(
    name: &str, holding: &StockHolding, target_shares: Decimal, commission_calc: &mut CommissionCalc,
    currency: &str, converter: CurrencyConverterRc,
) -> GenericResult<Decimal> {
    if target_shares == holding.current_shares {
        return Ok(dec!(0))
    }

    let (trade_type, shares) = if target_shares > holding.current_shares {
        (TradeType::Buy, target_shares - holding.current_shares)
    } else {
        (TradeType::Sell, holding.current_shares - target_shares)
    };

    let date = time::today_trade_conclusion_time().date;
    let commission = commission_calc.add_trade(date, trade_type, shares, holding.currency_price)
        .map_err(|e| format!("{}: {}", name, e))?;

    converter.convert_to(date, commission, currency)
}

fn calculate_total_commissions(portfolio: &Portfolio, converter: CurrencyConverterRc) -> GenericResult<(Decimal, Decimal)> {
    let mut calc = CommissionCalc::new(
        converter.clone(), portfolio.broker.commission_spec.clone(),
        Cash::new(&portfolio.currency, portfolio.current_net_value))?;

    let trade_commissions = calculate_trade_commissions(
        &portfolio.assets, &mut calc, &portfolio.currency, converter.clone())?;

    let date = time::today_trade_conclusion_time().date;
    let mut additional_commissions = dec!(0);

    for commissions in calc.calculate()?.values() {
        for commission in commissions.iter() {
            additional_commissions += converter.convert_to(date, commission, &portfolio.currency)?;
        }
    }

    Ok((trade_commissions, additional_commissions))
}

fn calculate_trade_commissions(
    assets: &[AssetAllocation], calc: &mut CommissionCalc,
    currency: &str, converter: CurrencyConverterRc,
) -> GenericResult<Decimal> {
    let mut trade_commissions = dec!(0);

    for asset in assets {
        match &asset.holding {
            Holding::Stock(holding) => {
                trade_commissions += calculate_target_commission(
                    &asset.full_name(), holding, holding.target_shares, calc,
                    currency, converter.clone(),
                )?;
            },
            Holding::Group(assets) => {
                trade_commissions += calculate_trade_commissions(
                    assets, calc, currency, converter.clone())?;
            },
        }
    }

    Ok(trade_commissions)
}

fn calculate_min_sell_volume(asset: &AssetAllocation, min_trade_volume: Decimal) -> Option<Decimal> {
    let trade_granularity = asset.iterative_trading_granularity(TradeType::Sell);

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
            // current = trade < min <= target
            asset.target_value - asset.current_value
        }
    };

    if asset.target_value - trade_volume < asset.min_value {
        return None
    }

    Some(trade_volume)
}

fn calculate_min_buy_volume(asset: &AssetAllocation, min_trade_volume: Decimal) -> Option<Decimal> {
    let trade_granularity = asset.iterative_trading_granularity(TradeType::Buy);

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
            // target <= min < trade = current
            asset.current_value - asset.target_value
        }
    };

    if let Some(max_value) = asset.max_value {
        if asset.target_value + trade_volume > max_value {
            return None;
        }
    }

    Some(trade_volume)
}

fn round_min_trade_volume(volume: Decimal, granularity: Decimal) -> Decimal {
    (volume / granularity).ceil() * granularity
}