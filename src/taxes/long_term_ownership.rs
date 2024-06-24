// Long-term ownership tax exemption logic

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::Datelike;
use isin::ISIN;
use num_traits::Zero;

use static_table_derive::StaticTable;

use crate::currency::{self, Cash};
use crate::time::Date;
use crate::types::Decimal;

#[derive(Clone, Copy)]
pub struct LtoDeductibleProfit {
    pub profit: Decimal,
    pub years: u32,
}

pub struct LtoDeductionCalculator {
    profit: Decimal,
    weighted_profit: Decimal,
    out_of_limit: Decimal,
}

impl LtoDeductionCalculator {
    pub fn new() -> LtoDeductionCalculator {
        LtoDeductionCalculator {
            profit: Decimal::default(),
            weighted_profit: Decimal::default(),
            out_of_limit: Decimal::default(),
        }
    }

    pub fn add(&mut self, profit: Decimal, years: u32, out_of_limit: bool) {
        assert!(profit.is_sign_positive());
        assert!(years >= 3);

        if out_of_limit {
            self.out_of_limit += profit;
        } else {
            self.profit += profit;
            self.weighted_profit += profit * Decimal::from(years);
        }
    }

    pub fn calculate(self) -> LtoDeduction {
        let mut total_profit = self.out_of_limit;
        let mut total_limit = self.out_of_limit;
        let mut total_deduction = self.out_of_limit;

        if !self.profit.is_zero() {
            let limit = currency::round(self.weighted_profit / self.profit * dec!(3_000_000));

            total_profit += self.profit;
            total_limit += limit;
            total_deduction += std::cmp::min(self.profit, limit);
        }

        LtoDeduction {
            deduction: total_deduction,
            limit:     total_limit,
            loss:      total_profit - total_deduction,
        }
    }
}

// Calculates result of a separate applying of LTO tax exemption by different tax agents
pub struct NetLtoDeductionCalculator {
    tax_years: HashMap<i32, TaxYearLto>
}

#[derive(PartialEq, Debug)]
pub struct NetLtoDeduction {
    pub applied_above_limit: Decimal,
    pub loss: Decimal,
}

struct TaxYearLto {
    calc: LtoDeductionCalculator,
    applied_deduction: Decimal,
    applied_loss: Decimal,
}

impl NetLtoDeductionCalculator {
    pub fn new() -> NetLtoDeductionCalculator {
        NetLtoDeductionCalculator {
            tax_years: HashMap::new(),
        }
    }

    pub fn add_profit(&mut self, tax_year: i32, profit: Decimal, years: u32, out_of_limit: bool) {
        self.tax_year(tax_year).calc.add(profit, years, out_of_limit);
    }

    pub fn add_applied_deduction(&mut self, tax_year: i32, deduction: Decimal, loss: Decimal) {
        assert!(deduction.is_sign_positive());
        assert!(loss.is_sign_positive());

        let stat = self.tax_year(tax_year);
        stat.applied_deduction += deduction;
        stat.applied_loss += loss;
    }

    pub fn calculate(self) -> BTreeMap<i32, NetLtoDeduction> {
        let mut tax_years = BTreeMap::new();

        for (tax_year, stat) in self.tax_years.into_iter() {
            let limit = stat.calc.calculate().limit;
            let applied_above_limit = std::cmp::max(dec!(0), stat.applied_deduction - limit);

            tax_years.insert(tax_year, NetLtoDeduction {
                applied_above_limit,
                loss: stat.applied_loss,
            });
        }

        tax_years
    }

    fn tax_year(&mut self, year: i32) -> &mut TaxYearLto {
        self.tax_years.entry(year).or_insert_with(|| TaxYearLto {
            calc: LtoDeductionCalculator::new(),
            applied_deduction: Decimal::zero(),
            applied_loss: Decimal::zero(),
        })
    }
}

#[cfg_attr(test, derive(PartialEq, Debug))]
pub struct LtoDeduction {
    pub deduction: Decimal,
    pub limit: Decimal,
    pub loss: Decimal,
}

#[derive(StaticTable)]
#[table(name="LtoTable")]
struct LtoRow {
    #[column(name="Deduction")]
    deduction: Cash,
    #[column(name="Limit")]
    limit: Cash,
    #[column(name="Loss")]
    loss: Cash,
}

impl LtoDeduction {
    pub fn print(&self, title: &str) {
        let currency = "RUB";

        let mut table = LtoTable::new();
        if self.loss.is_zero() {
            table.hide_loss();
        }

        table.add_row(LtoRow {
            deduction: Cash::new(currency, self.deduction),
            limit: Cash::new(currency, self.limit),
            loss: Cash::new(currency, self.loss),
        });

        table.print(title);
    }
}

pub fn is_applicable(isin: &HashSet<ISIN>, sell_date: Date) -> Option<bool> {
    // FIXME(konishchev): It's actually 2025, but since all foreign stocks are actually blocked from trading, set it to
    // 2024 to make sell simulation more realistic.
    // if sell_date.year() < 2025 {
    //     return Some(true)
    // }
    if sell_date.year() < 2024 {
        return Some(true)
    }

    let mut result = None;

    for isin in isin {
        let applicable = isin.prefix() == "RU";
        if let Some(prev) = result.replace(applicable) {
            if prev != applicable {
                return None;
            }
        }
    }

    result
}

pub fn is_deductible(isin: &HashSet<ISIN>, buy_date: Date, sell_date: Date) -> Option<u32> {
    if !is_applicable(isin, sell_date).unwrap_or_default() {
        return None;
    }

    if buy_date < date!(2014, 1, 1) {
        return None;
    }

    let years = calculate_ownership_years(buy_date, sell_date);
    if years >= 3 {
        Some(years)
    } else {
        None
    }
}

fn calculate_ownership_years(buy_date: Date, sell_date: Date) -> u32 {
    assert!(buy_date <= sell_date);
    let mut years = sell_date.year() - buy_date.year();

    let buy_month = buy_date.month();
    let sell_month = sell_date.month();

    match sell_month.cmp(&buy_month) {
        Ordering::Greater => {},
        Ordering::Less => {
            years -= 1;
        },
        Ordering::Equal => {
            if sell_date.day() < buy_date.day() && sell_date.succ_opt().unwrap().month() == sell_month {
                years -= 1;
            }
        }
    }

    cast::u32(years).unwrap()
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(buy_date, sell_date, years,
        case(date!(2014, 3, 19), date!(2014, 3, 19), 0),
        case(date!(2014, 3, 19), date!(2015, 3, 19), 1),
        case(date!(2014, 3, 19), date!(2016, 3, 19), 2),
        case(date!(2014, 3, 19), date!(2017, 3, 19), 3),
        case(date!(2014, 3, 19), date!(2017, 3, 18), 2),
        case(date!(2014, 3, 19), date!(2017, 3, 20), 3),

        case(date!(2020, 2, 29), date!(2020, 2, 29), 0),
        case(date!(2020, 2, 29), date!(2020, 3,  1), 0),
        case(date!(2020, 2, 29), date!(2021, 2, 27), 0),
        case(date!(2020, 2, 29), date!(2021, 2, 28), 1),
        case(date!(2020, 2, 29), date!(2021, 3,  1), 1),
        case(date!(2020, 2, 29), date!(2024, 2, 28), 3),
        case(date!(2020, 2, 29), date!(2024, 2, 29), 4),
        case(date!(2020, 2, 29), date!(2024, 3,  1), 4),
    )]
    fn ownership_years_calculation(buy_date: Date, sell_date: Date, years: u32) {
        assert_eq!(calculate_ownership_years(buy_date, sell_date), years);
    }

    #[rstest(with_out_of_limit => [false, true])]
    fn deduction_amount_calculation(with_out_of_limit: bool) {
        let out_of_limit = if with_out_of_limit {
            dec!(400_000)
        } else {
            dec!(0)
        };

        {
            let mut calculator = LtoDeductionCalculator::new();
            if with_out_of_limit {
                calculator.add(out_of_limit, 3, true);
            }
            assert_eq!(calculator.calculate(), LtoDeduction {
                deduction: out_of_limit,
                limit:     out_of_limit,
                loss:      dec!(0),
            });
        }

        {
            let mut calculator = LtoDeductionCalculator::new();

            calculator.add(dec!(13_000_000), 4, false);
            if with_out_of_limit {
                calculator.add(out_of_limit, 3, true);
            }

            assert_eq!(calculator.calculate(), LtoDeduction {
                deduction: dec!(12_000_000) + out_of_limit,
                limit:     dec!(12_000_000) + out_of_limit,
                loss:      dec!( 1_000_000),
            });
        }

        {
            let mut calculator = LtoDeductionCalculator::new();

            calculator.add(dec!(2_000_000), 3, false);
            calculator.add(dec!(  500_000), 3, false);
            calculator.add(dec!(2_000_000), 4, false);
            calculator.add(dec!(1_500_000), 4, false);
            calculator.add(dec!(4_000_000), 5, false);

            if with_out_of_limit {
                calculator.add(out_of_limit, 3, true);
            }

            assert_eq!(calculator.calculate(), LtoDeduction {
                deduction: dec!(10_000_000) + out_of_limit,
                limit:     dec!(12_450_000) + out_of_limit,
                loss:      dec!(0),
            });
        }
    }
}