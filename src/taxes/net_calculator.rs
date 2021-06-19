use std::collections::{HashMap, HashSet};

use crate::currency::Cash;
use crate::localities::Country;
use crate::time::Date;

use super::{IncomeType, TaxPaymentDay};

pub struct NetTaxCalculator {
    country: Country,
    tax_payment_day: TaxPaymentDay,
    profit: HashMap<(i32, Date), NetProfit>,
}

struct NetProfit {
    total: Cash,
    taxable: Cash,
}

impl NetTaxCalculator {
    pub fn new(country: Country, tax_payment_day: TaxPaymentDay) -> NetTaxCalculator {
        NetTaxCalculator {
            country,
            tax_payment_day,
            profit: HashMap::new(),
        }
    }

    pub fn add_profit(&mut self, date: Date, total: Cash, taxable: Cash) {
        let currency = self.country.currency;
        let key = self.tax_payment_day.get(date, true);

        let profit = self.profit.entry(key).or_insert_with(|| NetProfit {
            total: Cash::zero(currency),
            taxable: Cash::zero(currency),
        });

        profit.total += total.round();
        profit.taxable += taxable.round();
    }

    pub fn get_taxes(&self) -> HashMap<Date, (Cash, Cash)> {
        let mut taxes = HashMap::new();
        let mut years = HashSet::new();

        for (&(tax_year, tax_payment_date), profit) in self.profit.iter() {
            assert!(years.insert(tax_year)); // Ensure that we have only one tax payment per year

            let tax_to_pay = self.country.tax_to_pay(
                IncomeType::Trading, tax_year, profit.taxable, None);

            let tax_without_deduction = self.country.tax_to_pay(
                IncomeType::Trading, tax_year, profit.total, None);

            let tax_deduction = tax_without_deduction - tax_to_pay;
            assert!(!tax_deduction.is_negative());

            assert_eq!(taxes.insert(tax_payment_date, (tax_to_pay, tax_deduction)), None);
        }

        taxes
    }
}