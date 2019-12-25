mod builders;

use std::collections::{BTreeMap, HashMap};
use std::ops::Bound;

use num_traits::Zero;

use crate::core::GenericResult;
use crate::currency::Cash;
use crate::types::{Date, Decimal, TradeType};
use crate::util::{self, RoundingMethod};

pub use builders::*;

#[derive(Clone, Debug)]
pub struct CommissionSpec {
    currency: &'static str,
    rounding_method: RoundingMethod,

    trade: TradeCommissionSpec,
    cumulative: CumulativeCommissionSpec,
}

#[derive(Default, Clone, Debug)]
pub struct TradeCommissionSpec {
    commission: TransactionCommissionSpec,
    transaction_fees: Vec<(TradeType, TransactionCommissionSpec)>,
}

#[derive(Default, Clone, Debug)]
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

#[derive(Default, Clone, Debug)]
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

    pub fn add_trade(&mut self, date: Date, trade_type: TradeType, shares: u32, price: Cash) -> GenericResult<Cash> {
        let mut commission = self.add_trade_precise(date, trade_type, shares, price)?;
        commission.amount = util::round_with(commission.amount, 2, self.spec.rounding_method);
        Ok(commission)
    }

    pub fn add_trade_precise(&mut self, date: Date, trade_type: TradeType, shares: u32, price: Cash) -> GenericResult<Cash> {
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

    pub fn calculate(self) -> HashMap<Date, Cash> {
        self.volume.iter().filter_map(|(&date, &volume)| {
            let commission = self.calculate_daily(volume);
            if commission.is_zero() {
                None
            } else {
                Some((date, Cash::new(self.spec.currency, commission)))
            }
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