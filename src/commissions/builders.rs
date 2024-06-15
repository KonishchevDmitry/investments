use std::collections::BTreeMap;

use crate::core::GenericResult;
use crate::types::{Decimal, TradeType};
use crate::util::RoundingMethod;

use super::{
    CommissionSpec, TradeCommissionSpec, TransactionCommissionSpec,
    CumulativeCommissionSpec, CumulativeTierType, CumulativeTieredSpec, CumulativeFeeSpec
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
        self.tiers(CumulativeTierType::Volume, btreemap!{0 => percent}).unwrap()
    }

    pub fn volume_tiered(self, tiers: BTreeMap<u64, Decimal>) -> GenericResult<CumulativeCommissionSpecBuilder> {
        self.tiers(CumulativeTierType::Volume, tiers)
    }

    pub fn portfolio_net_value_tiered(self, tiers: BTreeMap<u64, Decimal>) -> GenericResult<CumulativeCommissionSpecBuilder> {
        self.tiers(CumulativeTierType::PortfolioNetValue, tiers)
    }

    fn tiers(mut self, _type: CumulativeTierType, tiers: BTreeMap<u64, Decimal>) -> GenericResult<CumulativeCommissionSpecBuilder> {
        if tiers.is_empty() || !tiers.contains_key(&0) {
            return Err!("Invalid tiered commission specification: There is no tier with zero value");
        } else if self.0.percent.is_some() {
            return Err!("An attempt to redefine commission tiers")
        }

        self.0.percent.replace(CumulativeTieredSpec {
            _type,
            tiers: tiers.iter().map(|(&k, &v)| (k.into(), v)).collect(),
        });

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

    pub fn monthly_depositary(self, amount: Decimal) -> CumulativeCommissionSpecBuilder {
        self.monthly_depositary_tiered(btreemap!{0 => amount}).unwrap()
    }

    pub fn monthly_depositary_tiered(mut self, tiers: BTreeMap<u64, Decimal>) -> GenericResult<CumulativeCommissionSpecBuilder> {
        if tiers.is_empty() || !tiers.contains_key(&0) {
            return Err!(concat!(
                "Invalid tiered depositary commission specification: ",
                "There is no tier for zero portfolio net value",
            ));
        } else if !self.0.monthly_depositary.is_empty() {
            return Err!("An attempt to redefine depositary commission")
        }

        self.0.monthly_depositary = tiers.iter().map(|(&k, &v)| (k.into(), v)).collect();
        Ok(self)
    }

    pub fn build(self) -> CumulativeCommissionSpec {
        self.0
    }
}