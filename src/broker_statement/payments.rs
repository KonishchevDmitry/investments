use crate::core::GenericResult;
use crate::currency::Cash;

/// Calculates result amount from a series of payments and reversals. Doesn't require payments and
/// reversals to be in order because Interactive Brokers' statement does't guarantee the order.
pub struct Payments {
    payments: Vec<Cash>,
    reversals: Vec<Cash>,
}

impl Payments {
    pub fn new() -> Payments {
        Payments {
            payments: Vec::new(),
            reversals: Vec::new(),
        }
    }

    pub fn add(&mut self, amount: Cash) {
        self.payments.push(amount);
    }

    pub fn reverse(&mut self, amount: Cash) {
        self.reversals.push(amount);
    }

    pub fn merge(&mut self, other: &Payments) {
        for &amount in &other.payments {
            self.add(amount);
        }

        for &amount in &other.reversals {
            self.reverse(amount);
        }
    }

    pub fn get_result(self) -> GenericResult<Option<Cash>> {
        let Payments { mut payments, reversals } = self;

        for reversal in reversals {
            let index = payments.iter()
                .position(|&payment| payment == reversal)
                .ok_or_else(|| format!("Unexpected reversal: {}", reversal))?;

            payments.remove(index);
        }

        let mut result = match payments.pop() {
            Some(amount) => amount,
            None => return Ok(None),
        };

        for amount in payments {
            result.add_assign(amount).ok().ok_or_else(|| format!(
                "Mixed currency: {} and {}", result.currency, amount.currency))?;
        }

        Ok(Some(result))
    }
}