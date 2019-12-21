#![allow(dead_code)] // FIXME

use std::collections::HashMap;

use crate::core::GenericResult;
use crate::currency::Cash;
use crate::types::{Date, Decimal, TradeType};

use super::get_trade_volume;

struct TieredCommissionSpec {
    currency: &'static str,
    tiers: Vec<CommissionTier>,
    minimum_daily: Option<Decimal>,
}

struct CommissionTier {
    start_volume: Decimal,
    commission_spec: TierCommissionSpec,
}

impl CommissionTier {
    fn new(start_volume: Decimal, commission_spec: TierCommissionSpec) -> CommissionTier {
        CommissionTier {
            start_volume,
            commission_spec,
        }
    }
}

struct TierCommissionSpec {
    percent: Decimal,
}

impl TierCommissionSpec {
    fn new(percent: Decimal) -> TierCommissionSpec {
        TierCommissionSpec {
            percent,
        }
    }
}

struct TieredCommissionCalc {
    spec: TieredCommissionSpec,
    state: HashMap<Date, State>,
}

impl TieredCommissionCalc {
    fn new(spec: TieredCommissionSpec) -> TieredCommissionCalc {
        TieredCommissionCalc {
            spec,
            state: HashMap::new(),
        }
    }

    fn add_trade(
        &mut self, date: Date, _trade_type: TradeType, shares: u32, price: Cash
    ) -> GenericResult<Cash> {
        let state = self.state.entry(date).or_insert_with(|| {
            State {
                tier: 0,
                volume: dec!(0),
                commission: dec!(0),
            }
        });
        let mut remaining_volume = get_trade_volume(self.spec.currency, price * shares)?;

        while remaining_volume != dec!(0) {
            assert!(remaining_volume > dec!(0));

            if state.tier == self.spec.tiers.len() - 1 {
                state.add_volume(&self.spec, remaining_volume);
                break;
            }
            let next_tier_volume = self.spec.tiers[state.tier + 1].start_volume;

            if state.volume + remaining_volume < next_tier_volume {
                state.add_volume(&self.spec, remaining_volume);
                break;
            }

            let current_volume = next_tier_volume - state.volume;
            state.add_volume(&self.spec, current_volume);
            remaining_volume -= current_volume;
            state.tier += 1;
        }

        assert!(state.volume >= self.spec.tiers[state.tier].start_volume);
        if state.tier < self.spec.tiers.len() - 1 {
            assert!(state.volume < self.spec.tiers[state.tier + 1].start_volume);
        }

        Ok(Cash::new(self.spec.currency, dec!(0)))
    }

    fn calculate(self) -> HashMap<Date, Cash> {
        self.state.iter().map(|(&date, state)| {
            let mut commission = state.commission;

            if let Some(minimum) = self.spec.minimum_daily {
                if commission < minimum {
                    commission = minimum;
                }
            }

            (date, Cash::new(self.spec.currency, commission))
        }).collect()
    }
}

struct State {
    tier: usize,
    volume: Decimal,
    commission: Decimal,
}

impl State {
    fn add_volume(&mut self, commission_spec: &TieredCommissionSpec, volume: Decimal) {
        let spec = &commission_spec.tiers[self.tier].commission_spec;
        self.volume += volume;
        self.commission += volume * spec.percent / dec!(100);
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    // FIXME: Implement
    #[rstest(trade_type => [TradeType::Buy, TradeType::Sell])]
    fn bcs_commission(trade_type: TradeType) {
        let currency = "RUB";
        let mut commission_calc = TieredCommissionCalc::new(TieredCommissionSpec {
            currency: currency,
            tiers: vec![
                /*
Урегулирование сделок	0,01

                До 100 000	0,0531
                От 100 000 до 300 000	0,0413
                От 300 000 до 1 000 000	0,0354
                От 1 000 000 до 5 000 000	0,0295
                От 5 000 000 до 15 000 000	0,0236
                Свыше 15 000 000	0,0177
                */
                CommissionTier::new(dec!(0), TierCommissionSpec::new(dec!(0.0531) + dec!(0.01))),
                CommissionTier::new(dec!(100_000), TierCommissionSpec::new(dec!(0.0413) + dec!(0.01))),
            ],
            minimum_daily: None, //Some(dec!(35.4)),
        });

        for &(date, shares, price) in &[
            (date!(2, 12, 2019),  35, dec!(2959.5)),
            (date!(2, 12, 2019),   3, dec!(2960)),
            (date!(2, 12, 2019),  18, dec!(2960)),
//            (date!(3, 12, 2019), 107, dec!( 782.4)),
        ] {
            assert_eq!(
                commission_calc.add_trade(date, trade_type, shares, Cash::new(currency, price)).unwrap(),
                Cash::new(currency, dec!(0)),
            );
        }

        assert_eq!(commission_calc.calculate(), hashmap!{
            date!(2, 12, 2019) => Cash::new(currency, dec!(68.45) + dec!(16.57)),
//            date!(3, 12, 2019) => Cash::new(currency, dec!(44.45) + dec!(8.37)),
        });
    }
}