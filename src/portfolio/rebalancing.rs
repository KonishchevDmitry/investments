use std::collections::HashSet;

use num_traits::Zero;

use types::Decimal;

use super::asset_allocation::{Portfolio, AssetAllocation, Holding};

// FIXME: implement
pub fn rebalance_portfolio(portfolio: &mut Portfolio) {
    match sell_overbought_assets(&mut portfolio.assets, portfolio.total_value, portfolio.min_trade_volume) {
        SellResult::Ok => (),
        SellResult::Debt(debt) => panic!("Sell failed: {}", debt),
    };
}

enum SellResult {
    Ok,
    Debt(Decimal),
}

fn sell_overbought_assets(assets: &mut Vec<AssetAllocation>, target_total_value: Decimal, min_trade_volume: Decimal) -> SellResult {
    let mut correctable_holdings = HashSet::new();
    for index in 0..assets.len() {
        correctable_holdings.insert(index);
    }

    let mut force_selling = false;
    let mut uncorrectable_holdings: HashSet<usize> = HashSet::new();

    loop {
        let mut uncorrectable_weight = dec!(0);
        let mut uncorrectable_value = dec!(0);

        for index in &uncorrectable_holdings {
            let asset = &assets[*index];

            uncorrectable_weight += asset.expected_weight;
            uncorrectable_value += asset.target_value;
        }

        let mut correctable_target_total_value = target_total_value - uncorrectable_value;
        let divider = dec!(1) - uncorrectable_weight;
        let mut correctable_debt = dec!(0);

        if correctable_target_total_value.is_sign_negative() {
            correctable_debt = correctable_target_total_value.abs();
            correctable_target_total_value = dec!(0);
        }

        let mut changed = false;

        // FIXME: Sort on force selling
        for index in correctable_holdings.clone() {
            let asset = &mut assets[index];
            let prev_target_value = asset.target_value;

            asset.target_value = correctable_target_total_value * asset.expected_weight / divider;

            match asset.holding {
                Holding::Group(ref mut sub_assets) => {
                    // FIXME: force selling?
                    match sell_overbought_assets(sub_assets, asset.target_value, min_trade_volume) {
                        SellResult::Ok => (),
                        SellResult::Debt(debt) => {
                            correctable_holdings.remove(&index);
                            uncorrectable_holdings.insert(index);

                            assert!(debt > dec!(0));
                            asset.target_value += debt;
                            correctable_debt += debt;
                        },
                    };
                }
                Holding::Stock(ref mut holding) => {
                    if asset.current_value > asset.target_value {
                        if asset.restrict_selling.unwrap_or(false) || asset.current_value < min_trade_volume {
                            let debt = asset.current_value - asset.target_value;
                            assert!(debt > dec!(0));

                            asset.target_value = asset.current_value;
                            correctable_debt += debt;

                            correctable_holdings.remove(&index);
                            uncorrectable_holdings.insert(index);
                        } else if asset.current_value - asset.target_value < min_trade_volume {
                            if force_selling {
                                let target_value = asset.target_value;
                                asset.target_value = asset.current_value - min_trade_volume;

                                let extra_assets = target_value - asset.target_value;
                                assert!(extra_assets >= dec!(0));

                                correctable_debt -= extra_assets;
                                if correctable_debt.is_sign_negative() {
                                    correctable_debt = dec!(0);
                                }

                                // FIXME: HERE
                                if correctable_debt.is_zero() {
                                    break;
                                }
                            } else {
                                let debt = asset.current_value - asset.target_value;
                                assert!(debt > dec!(0));

                                asset.target_value = asset.current_value;
                                correctable_debt += debt;
                            }
                        }

                        // FIXME: HERE
                    }
                },
            };

            changed |= asset.target_value != prev_target_value;
        }

        if correctable_debt.is_zero() {
            return SellResult::Ok;
        }

        if correctable_holdings.is_empty() {
            return SellResult::Debt(correctable_debt);
        }

        if !changed {
            force_selling = true;
        }
    }
}