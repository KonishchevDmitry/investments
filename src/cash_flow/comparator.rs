use std::collections::{BTreeMap, BTreeSet, btree_map};

use log::{Level, log};

use crate::currency::{Cash, MultiCurrencyCashAccount};
use crate::formatting::format_date;
use crate::types::Date;

pub struct CashAssetsComparator<'a> {
    iter: btree_map::Iter<'a, Date, MultiCurrencyCashAccount>,
    next: Option<(&'a Date, &'a MultiCurrencyCashAccount)>,
    important_dates: Vec<Date>,
    currencies: BTreeSet<&'static str>,
}

impl<'a> CashAssetsComparator<'a> {
    pub fn new(
        historical: &'a BTreeMap<Date, MultiCurrencyCashAccount>, important_dates: Vec<Date>,
    ) -> CashAssetsComparator<'a> {
        let mut comparator = CashAssetsComparator {
            iter: historical.iter(),
            next: None,
            important_dates,
            currencies: BTreeSet::new(),
        };
        comparator.next();
        comparator
    }

    pub fn compare(&mut self, date: Date, calculated: &MultiCurrencyCashAccount) {
        while let Some((&historical_date, actual)) = self.next {
            if historical_date >= date {
                break
            }

            self.next();
            self.compare_to(historical_date, actual, calculated);
        }
    }

    fn compare_to(&mut self,
        date: Date, actual: &MultiCurrencyCashAccount, calculated: &MultiCurrencyCashAccount,
    ) {
        self.currencies.extend(actual.iter().map(|assets| assets.currency));
        self.currencies.extend(calculated.iter().map(|assets| assets.currency));

        for &currency in &self.currencies {
            let calculated_amount = calculated.get(currency).unwrap_or_else(||
                Cash::new(currency, dec!(0)));

            let actual_amount = actual.get(currency).unwrap_or_else(||
                Cash::new(currency, dec!(0)));

            if calculated_amount == actual_amount {
                continue;
            }

            // The calculations aren't 100% accurate. For example, Forex trades information contains
            // rounded numbers which may lead to calculation error with around 0.00001 precision.
            let equal = calculated_amount.round() == actual_amount.round();

            let level = if !equal && (self.consumed() || self.important_dates.contains(&date)) {
                Level::Warn
            } else {
                Level::Debug
            };

            log!(level, "Calculation error for {}: {} vs {} ({})",
                 format_date(date), calculated_amount, actual_amount,
                 calculated_amount.sub(actual_amount).unwrap());
        }
    }

    pub fn consumed(&self) -> bool {
        self.next.is_none()
    }

    fn next(&mut self) {
        self.next = self.iter.next();
    }
}