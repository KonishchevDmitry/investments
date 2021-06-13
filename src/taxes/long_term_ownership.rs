// Long-term ownership tax exemption logic

use chrono::Datelike;

use crate::time::Date;
use crate::types::Decimal;

pub struct LtoDeductionCalculator {
    profit: Decimal,
    weighted_profit: Decimal,
}

impl LtoDeductionCalculator {
    pub fn new() -> LtoDeductionCalculator {
        LtoDeductionCalculator {
            profit: Decimal::default(),
            weighted_profit: Decimal::default(),
        }
    }

    pub fn add(&mut self, profit: Decimal, years: u32) {
        assert!(profit.is_sign_positive());
        assert!(years >= 3);
        self.profit += profit;
        self.weighted_profit += profit * Decimal::from(years);
    }

    pub fn calculate(self) -> (Decimal, Decimal) {
        if self.profit.is_zero() {
            return (dec!(0), dec!(0));
        }

        let limit = self.weighted_profit / self.profit * dec!(3_000_000);
        let deduction = std::cmp::min(self.profit, limit);
        (deduction, limit)
    }
}

pub fn is_deductible(buy_date: Date, sell_date: Date) -> Option<u32> {
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
    if sell_month < buy_month {
        years -= 1;
    } else if sell_month == buy_month {
        if sell_date.day() < buy_date.day() && sell_date.succ().month() == sell_month {
            years -= 1;
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

    #[test]
    fn deduction_amount_calculation() {
        assert_eq!(LtoDeductionCalculator::new().calculate(), (dec!(0), dec!(0)));

        {
            let mut calculator = LtoDeductionCalculator::new();
            calculator.add(dec!(13_000_000), 4);
            assert_eq!(calculator.calculate(), (dec!(12_000_000), dec!(12_000_000)));
        }

        {
            let mut calculator = LtoDeductionCalculator::new();
            calculator.add(dec!(2_000_000), 3);
            calculator.add(dec!(  500_000), 3);
            calculator.add(dec!(2_000_000), 4);
            calculator.add(dec!(1_500_000), 4);
            calculator.add(dec!(4_000_000), 5);
            assert_eq!(calculator.calculate(), (dec!(10_000_000), dec!(12_450_000)));
        }
    }
}