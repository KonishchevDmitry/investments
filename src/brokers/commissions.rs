#[cfg(test)] use brokers;
#[cfg(test)] use config::Config;
use core::{EmptyResult, GenericResult};
use currency::Cash;
use types::Decimal;

#[derive(Debug, Clone)]
pub struct CommissionSpec {
    minimum: Option<Cash>,
    per_share: Option<Cash>,
    percent: Option<Decimal>,
    maximum_percent: Option<Decimal>,
}

impl CommissionSpec {
    pub fn calculate(&self, shares: u32, price: Cash) -> GenericResult<Cash> {
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

        Ok(match self.minimum {
            Some(minimum) => {
                validate_currency(minimum, commissions)?;

                if commissions.amount < minimum.amount {
                    minimum
                } else {
                    commissions
                }
            },
            None => commissions,
        }.round())
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
        let commission_spec = brokers::interactive_brokers(&Config::mock()).unwrap().commission_spec;

        // Minimum commission > per share commission
        assert_eq!(commission_spec.calculate(199, Cash::new("USD", dec!(100))).unwrap(),
                   Cash::new("USD", dec!(1)));

        // Minimum commission == per share commission
        assert_eq!(commission_spec.calculate(200, Cash::new("USD", dec!(100))).unwrap(),
                   Cash::new("USD", dec!(1)));

        // Per share commission > minimum commission
        assert_eq!(commission_spec.calculate(201, Cash::new("USD", dec!(100))).unwrap(),
                   Cash::new("USD", decf!(1.01)));

        // Per share commission > minimum commission
        assert_eq!(commission_spec.calculate(300, Cash::new("USD", dec!(100))).unwrap(),
                   Cash::new("USD", decf!(1.5)));

        // Per share commission > maximum commission
        assert_eq!(commission_spec.calculate(300, Cash::new("USD", decf!(0.4))).unwrap(),
                   Cash::new("USD", decf!(1.2)));
    }

    #[test]
    fn open_broker_commission() {
        let commission_spec = brokers::open_broker(&Config::mock()).unwrap().commission_spec;

        // Percent commission > minimum commission
        assert_eq!(commission_spec.calculate(73, Cash::new("RUB", dec!(2758))).unwrap(),
                   Cash::new("RUB", decf!(114.76)));

        // Percent commission < minimum commission
        assert_eq!(commission_spec.calculate(1, Cash::new("RUB", dec!(1))).unwrap(),
                   Cash::new("RUB", decf!(0.04)));
    }
}