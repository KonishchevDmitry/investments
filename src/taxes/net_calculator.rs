use std::collections::HashMap;

use crate::currency::Cash;
use crate::localities::Country;
use crate::taxes::long_term_ownership::{LtoDeductibleProfit, LtoDeductionCalculator};
use crate::time::Date;

use super::{IncomeType, TaxPaymentDay};

pub struct NetTaxCalculator {
    country: Country,
    tax_payment_day: TaxPaymentDay,
    profit: HashMap<(i32, Date), NetProfit>,
}

pub struct NetTax {
    pub tax_payment_date: Date,
    pub tax_to_pay: Cash,
    pub tax_deduction: Cash,
    pub lto_loss: Cash,
}

struct NetProfit {
    total: Cash,
    taxable: Cash,
    lto: LtoDeductionCalculator,
}

impl NetTaxCalculator {
    pub fn new(country: Country, tax_payment_day: TaxPaymentDay) -> NetTaxCalculator {
        NetTaxCalculator {
            country,
            tax_payment_day,
            profit: HashMap::new(),
        }
    }

    pub fn add_profit(
        &mut self, date: Date, total: Cash, taxable: Cash, lto_deductible: &[LtoDeductibleProfit],
    ) {
        let currency = self.country.currency;
        let key = self.tax_payment_day.get(date, true);

        let net_profit = self.profit.entry(key).or_insert_with(|| NetProfit {
            total: Cash::zero(currency),
            taxable: Cash::zero(currency),
            lto: LtoDeductionCalculator::new(),
        });

        net_profit.total += total.round();
        net_profit.taxable += taxable.round();

        for &LtoDeductibleProfit{profit, years} in lto_deductible {
            net_profit.lto.add(profit, years);
        }
    }

    pub fn calculate(self) -> HashMap<i32, NetTax> {
        let mut taxes = HashMap::new();

        for ((tax_year, tax_payment_date), profit) in self.profit.into_iter() {
            let (lto_deduction, _, lto_loss) = profit.lto.calculate();

            let lto_deduction = Cash::new(self.country.currency, lto_deduction);
            let lto_loss = Cash::new(self.country.currency, lto_loss);

            let tax_to_pay = self.country.tax_to_pay(
                IncomeType::Trading, tax_year, profit.taxable - lto_deduction, None);

            let tax_without_deduction = self.country.tax_to_pay(
                IncomeType::Trading, tax_year, profit.total, None);

            let tax_deduction = tax_without_deduction - tax_to_pay;
            assert!(!tax_deduction.is_negative());

            let net_tax = NetTax {tax_payment_date, tax_to_pay, tax_deduction, lto_loss};
            assert!(taxes.insert(tax_year, net_tax).is_none());
        }

        taxes
    }
}