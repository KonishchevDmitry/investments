use std::collections::BTreeMap;

use crate::core::GenericResult;
use crate::types::{Decimal, TradeType};
use crate::util::RoundingMethod;

use super::{
    CommissionSpec, TradeCommissionSpec, TransactionCommissionSpec,
    CumulativeCommissionSpec, CumulativeFeeSpec
};

pub struct CommissionSpecBuilder(CommissionSpec);

impl CommissionSpecBuilder {
    pub fn new(currency: &'static str) -> CommissionSpecBuilder {
        CommissionSpecBuilder(CommissionSpec {
            currency,
            rounding_method: RoundingMethod::Round,
            trade: Default::default(),
            cumulative: Default::default(),
        })
    }

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
    pub fn new() -> TradeCommissionSpecBuilder {
        TradeCommissionSpecBuilder::default()
    }

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
    pub fn new() -> TransactionCommissionSpecBuilder {
        TransactionCommissionSpecBuilder::default()
    }

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
    pub fn new() -> CumulativeCommissionSpecBuilder {
        CumulativeCommissionSpecBuilder::default()
    }

    pub fn percent(self, percent: Decimal) -> CumulativeCommissionSpecBuilder {
        self.tiers(btreemap!{dec!(0) => percent}).unwrap()
    }

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

    pub fn minimum_monthly(mut self, minimum: Decimal) -> CumulativeCommissionSpecBuilder {
        self.0.minimum_monthly.replace(minimum);
        self
    }

    pub fn percent_fee(mut self, percent: Decimal) -> CumulativeCommissionSpecBuilder {
        self.0.fees.push(CumulativeFeeSpec {
            percent: percent,
        });
        self
    }

    pub fn monthly_depositary(mut self, amount: Decimal) -> CumulativeCommissionSpecBuilder {
        self.0.monthly_depositary.replace(amount);
        self
    }

    pub fn build(self) -> CumulativeCommissionSpec {
        self.0
    }
}