use std::collections::{HashMap, HashSet};

use crate::localities::Country;
use crate::types::{Date, Decimal};
use chrono::Datelike;

pub struct NetTaxCalculator {
    country: Country,
    profit: HashMap<Date, Decimal>,
}

impl NetTaxCalculator {
    pub fn new(country: Country) -> NetTaxCalculator {
        NetTaxCalculator {
            country,
            profit: HashMap::new(),
        }
    }

    pub fn add_profit(&mut self, date: Date, amount: Decimal) {
        let tax_payment_date = self.country.get_tax_payment_date(date);
        self.profit.entry(tax_payment_date)
            .and_modify(|profit| *profit += amount)
            .or_insert(amount);
    }

    pub fn get_taxes(&self) -> HashMap<Date, Decimal> {
        let mut taxes = HashMap::new();
        let mut years = HashSet::new();

        for (&tax_payment_date, &profit) in self.profit.iter() {
            let year = tax_payment_date.year();
            assert!(years.insert(year)); // Ensure that we have only one tax payment date per year

            let tax_to_pay = self.country.tax_to_pay(profit, None);
            assert_eq!(taxes.insert(tax_payment_date, tax_to_pay), None);
        }

        taxes
    }
}
