use crate::core::GenericResult;
use crate::currency::Cash;
use crate::types::Date;

#[derive(PartialEq, Eq, Hash)]
pub struct TaxId {
    pub date: Date,
    pub description: String,
}

impl TaxId {
    pub fn new(date: Date, description: &str) -> TaxId {
        TaxId { date, description: description.to_owned() }
    }
}

// FIXME: Replace with payments API
// Calculates result tax from a series of withholds and refunds. Doesn't require withholds and
// refunds to be in order because Interactive Brokers' statements don't guarantee the order.
pub struct TaxChanges {
    withheld: Vec<Cash>,
    refunded: Vec<Cash>,
}

impl TaxChanges {
    pub fn new() -> TaxChanges {
        TaxChanges {
            withheld: Vec::new(),
            refunded: Vec::new(),
        }
    }

    pub fn withhold(&mut self, tax: Cash) {
        self.withheld.push(tax);
    }

    pub fn refund(&mut self, tax: Cash) {
        self.refunded.push(tax);
    }

    pub fn merge(&mut self, other: &TaxChanges) {
        for &amount in &other.withheld {
            self.withhold(amount);
        }

        for &amount in &other.refunded {
            self.refund(amount);
        }
    }

    pub fn get_result_tax(self) -> GenericResult<Cash> {
        let TaxChanges { mut withheld, refunded } = self;

        for refund in refunded {
            let index = withheld.iter()
                .position(|&amount| amount == refund)
                .ok_or_else(|| format!(
                    "Unexpected tax refund: {}. Unable to find the matching withheld tax", refund))?;

            withheld.remove(index);
        }

        let mut result = match withheld.pop() {
            Some(amount) => amount,

            // It's may be ok, but for now return an error until we'll see it in the real life
            None => return Err!("Got a fully refunded tax"),
        };

        for amount in withheld {
            result.add_assign(amount).ok().ok_or_else(|| format!(
                "Got a few withheld taxes in different currency: {} and {}",
                result.currency, amount.currency
            ))?;
        }

        Ok(result)
    }
}