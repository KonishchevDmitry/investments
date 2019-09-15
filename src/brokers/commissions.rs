#[cfg(test)] use crate::brokers::Broker;
use crate::core::{EmptyResult, GenericResult};
use crate::currency::Cash;
use crate::types::{Decimal, TradeType};

#[derive(Debug, Clone)]
pub struct CommissionSpec {
    minimum: Option<Cash>,
    per_share: Option<Cash>,
    percent: Option<Decimal>,
    maximum_percent: Option<Decimal>,
    transaction_fees: Vec<(TradeType, CommissionSpec)>,
}

impl CommissionSpec {
    pub fn calculate(&self, trade_type: TradeType, shares: u32, price: Cash) -> GenericResult<Cash> {
        Ok(self.calculate_precise(trade_type, shares, price)?.round())
    }

    fn calculate_precise(&self, trade_type: TradeType, shares: u32, price: Cash) -> GenericResult<Cash> {
        let validate_currency = |a: Cash, b: Cash| -> EmptyResult {
            if a.currency != b.currency {
                return Err!(concat!(
                    "Unable to calculate trade commission: ",
                    "Commission currency doesn't match trade currency: {} vs {}"),
                    a.currency, b.currency);
            }

            Ok(())
        };

        let mut commissions = match (self.per_share, self.percent) {
            (Some(per_share), None) => per_share * shares,
            (None, Some(percent)) => price * shares * percent / 100,
            _ => unreachable!(),
        };

        if let Some(maximum_percent) = self.maximum_percent {
            let max_commissions = price * shares * maximum_percent / 100;

            validate_currency(max_commissions, commissions)?;
            if commissions.amount > max_commissions.amount {
                commissions.amount = max_commissions.amount;
            }
        }

        if let Some(minimum) = self.minimum {
            validate_currency(minimum, commissions)?;
            if commissions.amount < minimum.amount {
                commissions = minimum
            }
        }

        for (transaction_type, fee_spec) in &self.transaction_fees {
            if *transaction_type == trade_type {
                let fee = fee_spec.calculate_precise(trade_type, shares, price)?;
                validate_currency(fee, commissions)?;
                commissions.add_assign(fee)?;
            }
        }

        Ok(commissions)
    }
}

pub struct CommissionSpecBuilder {
    spec: CommissionSpec,
}

impl CommissionSpecBuilder {
    pub fn new() -> CommissionSpecBuilder {
        CommissionSpecBuilder {
            spec: CommissionSpec {
                minimum: None,
                per_share: None,
                percent: None,
                maximum_percent: None,
                transaction_fees: Vec::new(),
            }
        }
    }

    pub fn minimum(mut self, minimum: Cash) -> CommissionSpecBuilder {
        self.spec.minimum = Some(minimum);
        self
    }

    pub fn per_share(mut self, per_share: Cash) -> CommissionSpecBuilder {
        self.spec.per_share = Some(per_share);
        self
    }

    pub fn percent(mut self, percent: Decimal) -> CommissionSpecBuilder {
        self.spec.percent = Some(percent);
        self
    }

    pub fn maximum_percent(mut self, maximum_percent: Decimal) -> CommissionSpecBuilder {
        self.spec.maximum_percent = Some(maximum_percent);
        self
    }

    pub fn transaction_fee(mut self, trade_type: TradeType, fee: CommissionSpec) -> CommissionSpecBuilder {
        self.spec.transaction_fees.push((trade_type, fee));
        self
    }

    pub fn build(self) -> GenericResult<CommissionSpec> {
        match (self.spec.per_share, self.spec.percent) {
            (Some(_), None) | (None, Some(_)) => (),
            _ => return Err!("Invalid commission specification"),
        };

        Ok(self.spec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interactive_brokers_commission() {
        let commission_spec = Broker::InteractiveBrokers.get_commission_spec();

        let trade_type = TradeType::Buy;

        // Minimum commission > per share commission
        assert_eq!(commission_spec.calculate(trade_type, 199, Cash::new("USD", dec!(100))).unwrap(),
                   Cash::new("USD", dec!(1)));

        // Minimum commission == per share commission
        assert_eq!(commission_spec.calculate(trade_type, 200, Cash::new("USD", dec!(100))).unwrap(),
                   Cash::new("USD", dec!(1)));

        // Per share commission > minimum commission
        assert_eq!(commission_spec.calculate(trade_type, 201, Cash::new("USD", dec!(100))).unwrap(),
                   Cash::new("USD", dec!(1.01)));

        // Per share commission > minimum commission
        assert_eq!(commission_spec.calculate(trade_type, 300, Cash::new("USD", dec!(100))).unwrap(),
                   Cash::new("USD", dec!(1.5)));

        // Per share commission > maximum commission
        assert_eq!(commission_spec.calculate(trade_type, 300, Cash::new("USD", dec!(0.4))).unwrap(),
                   Cash::new("USD", dec!(1.2)));

        let trade_type = TradeType::Sell;

        assert_eq!(commission_spec.calculate_precise(trade_type, 26, Cash::new("USD", dec!(174.2))).unwrap(),
                   Cash::new("USD", dec!(1.0619736)));

        assert_eq!(commission_spec.calculate(trade_type, 26, Cash::new("USD", dec!(174.2))).unwrap(),
                   Cash::new("USD", dec!(1.06)));
    }

    #[test]
    fn open_broker_commission() {
        let commission_spec = Broker::OpenBroker.get_commission_spec();

        for &trade_type in &[TradeType::Buy, TradeType::Sell] {
            // Percent commission > minimum commission
            assert_eq!(
                commission_spec.calculate(trade_type, 73, Cash::new("RUB", dec!(2758))).unwrap(),
                Cash::new("RUB", dec!(114.76)),
            );

            // Percent commission < minimum commission
            assert_eq!(
                commission_spec.calculate(trade_type, 1, Cash::new("RUB", dec!(1))).unwrap(),
                Cash::new("RUB", dec!(0.04)),
            );
        }
    }
}