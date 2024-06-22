use std::collections::{BTreeMap, HashMap};

use crate::currency::Cash;
use crate::localities::Country;
use crate::taxes::TaxCalculator;
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
    pub tax_deduction: Cash,
    pub tax_to_pay: Cash,

    pub lto_deduction: Cash,
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
        &mut self, date: Date, total: Cash, taxable: Cash,
        lto_deductible: &[LtoDeductibleProfit], emulated_trade: bool,
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
            net_profit.lto.add(profit, years, emulated_trade);
        }
    }

    // XXX(konishchev): Consume calculator?
    pub fn calculate(self, calculator: &mut TaxCalculator) -> BTreeMap<i32, NetTax> {
        let mut taxes = BTreeMap::new();

        for ((tax_year, tax_payment_date), profit) in self.profit.into_iter() {
            let lto = profit.lto.calculate();

            let lto_deduction = self.country.cash(lto.deduction);
            let lto_loss = self.country.cash(lto.loss);

            let tax = calculator.tax_deductible_income(
                IncomeType::Trading, tax_year, profit.total, profit.taxable - lto_deduction);

            let net_tax = NetTax {
                tax_payment_date,
                tax_deduction: tax.deduction,
                tax_to_pay: tax.to_pay,
                lto_deduction,
                lto_loss,
            };
            assert!(taxes.insert(tax_year, net_tax).is_none());
        }

        taxes
    }
}