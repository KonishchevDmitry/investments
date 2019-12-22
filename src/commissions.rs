#![allow(dead_code)] // FIXME

use std::collections::{BTreeMap, HashMap};
use std::ops::Bound;

use crate::core::GenericResult;
use crate::currency::Cash;
use crate::types::{Date, Decimal, TradeType};
use crate::util::{self, RoundingMethod};

#[derive(Clone)]
pub struct CommissionSpec {
    currency: &'static str,
    rounding_method: RoundingMethod,

    trade: TradeCommissionSpec,
    cumulative: CumulativeCommissionSpec,
}

#[derive(Default, Clone)]  // FIXME: Default?
pub struct TradeCommissionSpec {
    commission: TransactionCommissionSpec,
    transaction_fees: Vec<(TradeType, TransactionCommissionSpec)>,
}

#[derive(Default, Clone)]  // FIXME: Default?
pub struct TransactionCommissionSpec {
    percent: Option<Decimal>,
    per_share: Option<Decimal>,

    minimum: Option<Decimal>,
    maximum_percent: Option<Decimal>,
}

impl TransactionCommissionSpec {
    fn calculate(&self, shares: u32, volume: Decimal) -> Decimal {
        let mut commission = dec!(0);

        if let Some(per_share) = self.per_share {
            commission += per_share * Decimal::from(shares);
        }

        if let Some(percent) = self.percent {
            commission += volume * percent / dec!(100);
        }

        if let Some(maximum_percent) = self.maximum_percent {
            let max_commission = volume * maximum_percent / dec!(100);
            if commission > max_commission {
                commission = max_commission;
            }
        }

        if let Some(minimum) = self.minimum {
            if commission < minimum {
                commission = minimum
            }
        }

        commission
    }
}

#[derive(Clone)]
pub struct CumulativeCommissionSpec {
    tiers: Option<BTreeMap<Decimal, Decimal>>,
    minimum_daily: Option<Decimal>,
}

pub struct CommissionCalc {
    spec: CommissionSpec,
    volume: HashMap<Date, Decimal>,
}

impl CommissionCalc {
    pub fn new(spec: CommissionSpec) -> CommissionCalc {
        CommissionCalc {
            spec,
            volume: HashMap::new(),
        }
    }

    fn add_trade(&mut self, date: Date, trade_type: TradeType, shares: u32, price: Cash) -> GenericResult<Cash> {
        let mut commission = self.add_trade_precise(date, trade_type, shares, price)?;
        commission.amount = util::round_with(commission.amount, 2, self.spec.rounding_method);
        Ok(commission)
    }

    fn add_trade_precise(&mut self, date: Date, trade_type: TradeType, shares: u32, price: Cash) -> GenericResult<Cash> {
        let volume = get_trade_volume(self.spec.currency, price * shares)?;
        *self.volume.entry(date).or_default() += volume;

        let mut commission = self.spec.trade.commission.calculate(shares, volume);

        for (transaction_type, fee_spec) in &self.spec.trade.transaction_fees {
            if *transaction_type == trade_type {
                commission += fee_spec.calculate(shares, volume);
            }
        }

        Ok(Cash::new(self.spec.currency, commission))
    }

    fn calculate(self) -> HashMap<Date, Cash> {
        self.volume.iter().map(|(&date, &volume)| {
            let commission = self.calculate_daily(volume);
            (date, Cash::new(self.spec.currency, commission))
        }).collect()
    }

    fn calculate_daily(&self, volume: Decimal) -> Decimal {
        let tiers = match self.spec.cumulative.tiers {
            Some(ref tiers) => tiers,
            None => return dec!(0),
        };

        let percent = *tiers.range((Bound::Unbounded, Bound::Included(volume))).last().unwrap().1;
        let mut commission = volume * percent / dec!(100);

        // FIXME: Excluding exchange commission?
        if let Some(minimum) = self.spec.cumulative.minimum_daily {
            if commission < minimum {
                commission = minimum;
            }
        }

        util::round_with(commission, 2, self.spec.rounding_method)
    }
}

// FIXME: HERE
/*
impl CommissionSpec {
    pub fn calculate(&self, trade_type: TradeType, shares: u32, price: Cash) -> GenericResult<Cash> {
        Ok(self.calculate_precise(trade_type, shares, price)?.round())
    }

    fn calculate_precise(&self, trade_type: TradeType, shares: u32, price: Cash) -> GenericResult<Cash> {
    }
}
*/

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
        // FIXME: Get from BCS object + support all commissions
        // FIXME: Support depository commission for Open Broker
        let mut commission_calc = CommissionCalc::new(CommissionSpec {
            currency: currency,
            rounding_method: RoundingMethod::Truncate,
            /*
Урегулирование сделок	0,01

            До 100 000	0,0531
            От 100 000 до 300 000	0,0413
            От 300 000 до 1 000 000	0,0354
            От 1 000 000 до 5 000 000	0,0295
            От 5 000 000 до 15 000 000	0,0236
            Свыше 15 000 000	0,0177
            */
            trade: TradeCommissionSpec::default(),
            cumulative: CumulativeCommissionSpec {
                tiers: Some(btreemap!{
                    dec!(0) => dec!(0.0531) + dec!(0.01),
                    dec!(100_000) => dec!(0.0413) + dec!(0.01),
                }),
                minimum_daily: None,
            },
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