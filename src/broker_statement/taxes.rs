use crate::core::GenericResult;
use crate::currency::Cash;
use crate::types::Date;

#[derive(PartialEq, Eq, Hash)]
pub struct TaxId {
    pub date: Date,
    pub description: String,
}

/*
impl TaxId {
    fn new(date: Date, description: String) -> TaxId {
        TaxId { date, description }
    }
}
*/

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

        match withheld.len() {
            // It's may be ok, but for now return an error until we'll see it in the real life
            0 => Err!("Got a fully refunded tax"),

            1 => Ok(withheld.pop().unwrap()),
            _ => Err!("Got {} withheld taxes without refund", withheld.len()),
        }
    }
}