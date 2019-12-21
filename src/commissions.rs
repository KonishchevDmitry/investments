#![allow(dead_code)] // FIXME

use std::collections::HashMap;

use crate::core::GenericResult;
use crate::currency::Cash;
use crate::types::{Date, Decimal, TradeType};

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
    volume: HashMap<Date, Decimal>,
}

impl TieredCommissionCalc {
    fn new(spec: TieredCommissionSpec) -> TieredCommissionCalc {
        TieredCommissionCalc {
            spec,
            volume: HashMap::new(),
        }
    }

    fn add_trade(
        &mut self, date: Date, _trade_type: TradeType, shares: u32, price: Cash
    ) -> GenericResult<Cash> {
        let volume = get_trade_volume(self.spec.currency, price * shares)?;
        *self.volume.entry(date).or_default() += volume;
        Ok(Cash::new(self.spec.currency, dec!(0)))
    }

    fn calculate(self) -> HashMap<Date, Cash> {
        self.volume.iter().map(|(&date, &volume)| {
            let commission_spec = &self.spec.tiers.iter().filter(|tier| {
                tier.start_volume <= volume
            }).last().unwrap().commission_spec;

            let mut commission = volume * commission_spec.percent / dec!(100);

            if let Some(minimum) = self.spec.minimum_daily {
                if commission < minimum {
                    commission = minimum;
                }
            }

            // FIXME: Parametrize
            // It seems that BCS truncates commission instead of rounding, so do the same
            let scale = commission.scale();
            if scale > 2 {
                commission.set_scale(scale - 2);
                commission = commission.trunc();
                commission.set_scale(2);
            }

            (date, Cash::new(self.spec.currency, commission))
        }).collect()
    }
}

fn get_trade_volume(commission_currency: &str, volume: Cash) -> GenericResult<Decimal> {
    if volume.currency != commission_currency {
        return Err!(concat!(
            "Unable to calculate trade commission: ",
            "Commission currency doesn't match trade currency: {} vs {}"),
            commission_currency, volume.currency
        );
    }

    Ok(volume.amount)
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
            (date!(3, 12, 2019), 107, dec!( 782.4)),
        ] {
            assert_eq!(
                commission_calc.add_trade(date, trade_type, shares, Cash::new(currency, price)).unwrap(),
                Cash::new(currency, dec!(0)),
            );
        }

        assert_eq!(commission_calc.calculate(), hashmap!{
            date!(2, 12, 2019) => Cash::new(currency, dec!(85.02)),
            date!(3, 12, 2019) => Cash::new(currency, dec!(52.82)),
        });
    }
}