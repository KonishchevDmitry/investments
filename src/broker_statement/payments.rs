use crate::core::GenericResult;
use crate::currency::{Cash, CashAssets};
use crate::time::Date;

/// Calculates result amount from a series of payments and reversals. Doesn't require payments and
/// reversals to be in order because Interactive Brokers' statement doesn't guarantee the order.
pub struct Payments {
    strict: bool,
    payments: Vec<CashAssets>,
    reversals: Vec<CashAssets>,
}

impl Payments {
    pub fn new(strict: bool) -> Payments {
        Payments {
            strict: strict,
            payments: Vec::new(),
            reversals: Vec::new(),
        }
    }

    pub fn add(&mut self, date: Date, amount: Cash) {
        assert!(amount.is_positive());
        self.payments.push(CashAssets::new_from_cash(date, amount));
    }

    pub fn reverse(&mut self, date: Date, amount: Cash) {
        assert!(amount.is_positive());
        self.reversals.push(CashAssets::new_from_cash(date, amount));
    }

    pub fn merge(&mut self, other: &Payments) {
        for &payment in &other.payments {
            self.add(payment.date, payment.cash);
        }

        for &reversal in &other.reversals {
            self.reverse(reversal.date, reversal.cash);
        }
    }

    // FIXME(konishchev): Return all payments
    pub fn get_result(self) -> GenericResult<Option<Cash>> {
        let Payments { strict, mut payments, reversals } = self;

        if strict {
            for reversal in reversals {
                let index = payments.iter()
                    .position(|&payment| payment.cash == reversal.cash)
                    .ok_or_else(|| format!("Unexpected reversal: {}", reversal.cash))?;

                payments.remove(index);
            }

            let mut result = match payments.pop() {
                Some(payment) => payment.cash,
                None => return Ok(None),
            };

            for payment in payments {
                let amount = payment.cash;
                result.add_assign(amount).map_err(|_| format!(
                    "Mixed currency: {} and {}", result.currency, amount.currency))?;
            }

            Ok(Some(result))
        } else {
            let mut result: Option<Cash> = None;

            for payment in payments {
                let amount = payment.cash;

                match result.as_mut() {
                    None => {
                        result.replace(amount);
                    },
                    Some(result) => {
                        result.add_assign(amount).map_err(|_| format!(
                            "Mixed currency: {} and {}", result.currency, amount.currency))?;
                    },
                };
            }

            for reversal in reversals {
                let amount = reversal.cash;
                let result = result.as_mut().ok_or_else(|| format!(
                    "Unexpected reversal: {}", amount))?;

                result.sub_assign(amount).map_err(|_| format!(
                    "Mixed currency: {} and {}", result.currency, amount.currency))?;
            }

            if let Some(result) = result {
                if result.is_zero() {
                    return Ok(None);
                } else if result.is_negative() {
                    return Err!("Got a negative result: {}", result);
                }
            }

            Ok(result)
        }
    }
}