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

impl CommissionSpec {
    pub fn builder(currency: &'static str) -> CommissionSpecBuilder {
        CommissionSpecBuilder(CommissionSpec {
            currency,
            rounding_method: RoundingMethod::Round,

            trade: Default::default(),
            cumulative: Default::default(),
        })
    }

    // FIXME: A temporary solution for transition period
    pub fn calculate(&self, trade_type: TradeType, shares: u32, price: Cash) -> GenericResult<Cash> {
        CommissionCalc::new(self.clone()).add_trade(date!(1, 1, 2000), trade_type, shares, price)
    }

    // FIXME: A temporary solution for transition period
    fn calculate_precise(&self, trade_type: TradeType, shares: u32, price: Cash) -> GenericResult<Cash> {
        CommissionCalc::new(self.clone()).add_trade_precise(date!(1, 1, 2000), trade_type, shares, price)
    }
}

#[derive(Default, Clone)]  // FIXME: Default?
pub struct TradeCommissionSpec {
    commission: TransactionCommissionSpec,
    transaction_fees: Vec<(TradeType, TransactionCommissionSpec)>,
}

impl TradeCommissionSpec {
    pub fn builder() -> TradeCommissionSpecBuilder {
        TradeCommissionSpecBuilder::default()
    }
}

#[derive(Default, Clone)]  // FIXME: Default?
pub struct TransactionCommissionSpec {
    percent: Option<Decimal>,
    per_share: Option<Decimal>,

    minimum: Option<Decimal>,
    maximum_percent: Option<Decimal>,
}

impl TransactionCommissionSpec {
    pub fn builder() -> TransactionCommissionSpecBuilder {
        TransactionCommissionSpecBuilder::default()
    }

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

#[derive(Default, Clone)]
pub struct CumulativeCommissionSpec {
    tiers: Option<BTreeMap<Decimal, Decimal>>,
    minimum_daily: Option<Decimal>,
}

impl CumulativeCommissionSpec {
    pub fn builder() -> CumulativeCommissionSpecBuilder {
        CumulativeCommissionSpecBuilder::default()
    }
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

    pub fn calculate(self) -> HashMap<Date, Cash> {
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

pub struct CommissionSpecBuilder(CommissionSpec);

impl CommissionSpecBuilder {
    pub fn rounding_method(mut self, method: RoundingMethod) -> CommissionSpecBuilder {
        self.0.rounding_method = method;
        self
    }

    pub fn trade(mut self, spec: TradeCommissionSpec) -> CommissionSpecBuilder {
        self.0.trade = spec;
        self
    }

    pub fn cumulative(mut self, spec: CumulativeCommissionSpec) -> CommissionSpecBuilder {
        self.0.cumulative = spec;
        self
    }

    pub fn build(self) -> CommissionSpec {
        self.0
    }
}

#[derive(Default)]
pub struct TradeCommissionSpecBuilder(TradeCommissionSpec);

impl TradeCommissionSpecBuilder {
    pub fn commission(mut self, spec: TransactionCommissionSpec) -> TradeCommissionSpecBuilder {
        self.0.commission = spec;
        self
    }

    pub fn transaction_fee(mut self, trade_type: TradeType, spec: TransactionCommissionSpec) -> TradeCommissionSpecBuilder {
        self.0.transaction_fees.push((trade_type, spec));
        self
    }

    pub fn build(self) -> TradeCommissionSpec {
        self.0
    }
}

#[derive(Default)]
pub struct TransactionCommissionSpecBuilder(TransactionCommissionSpec);

impl TransactionCommissionSpecBuilder {
    pub fn minimum(mut self, minimum: Decimal) -> TransactionCommissionSpecBuilder {
        self.0.minimum = Some(minimum);
        self
    }

    pub fn per_share(mut self, per_share: Decimal) -> TransactionCommissionSpecBuilder {
        self.0.per_share = Some(per_share);
        self
    }

    pub fn percent(mut self, percent: Decimal) -> TransactionCommissionSpecBuilder {
        self.0.percent = Some(percent);
        self
    }

    pub fn maximum_percent(mut self, maximum_percent: Decimal) -> TransactionCommissionSpecBuilder {
        self.0.maximum_percent = Some(maximum_percent);
        self
    }

    pub fn build(self) -> GenericResult<TransactionCommissionSpec> {
        match (self.0.per_share, self.0.percent) {
            (Some(_), None) | (None, Some(_)) => (),
            _ => return Err!("Invalid commission specification"),
        };

        Ok(self.0)
    }
}

#[derive(Default)]
pub struct CumulativeCommissionSpecBuilder(CumulativeCommissionSpec);

impl CumulativeCommissionSpecBuilder {
    pub fn tiers(mut self, tiers: BTreeMap<Decimal, Decimal>) -> GenericResult<CumulativeCommissionSpecBuilder> {
        if tiers.is_empty() || tiers.get(&dec!(0)).is_none() {
            return Err!(concat!(
                "Invalid tiered commission specification: ",
                "There is no tier with zero starting volume",
            ));
        }

        self.0.tiers.replace(tiers);
        Ok(self)
    }

    pub fn minimum_daily(mut self, minimum: Decimal) -> CumulativeCommissionSpecBuilder {
        self.0.minimum_daily.replace(minimum);
        self
    }

    pub fn build(self) -> CumulativeCommissionSpec {
        self.0
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