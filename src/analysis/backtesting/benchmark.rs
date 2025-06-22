use crate::analysis::deposit::{Transaction, InterestPeriod};
use crate::analysis::deposit::performance::compare_instrument_to_bank_deposit;
use crate::core::GenericResult;
use crate::currency::{Cash, CashAssets};
use crate::formatting;
use crate::time::Date;
use crate::types::Decimal;

use super::BenchmarkPerformanceType;

pub trait Benchmark {
    fn name(&self) -> String;
    fn provider(&self) -> Option<String>;

    fn full_name(&self) -> String {
        match self.provider() {
            Some(provider) => format!("{} ({provider})", self.name()),
            None => self.name(),
        }
    }

    fn backtest(
        &self, method: BenchmarkPerformanceType, currency: &str, cash_flows: &[CashAssets], today: Date, full: Option<Date>,
    ) -> GenericResult<Vec<BacktestingResult>>;
}

pub struct BacktestingResult {
    pub date: Date,
    pub net_value: Decimal,
    pub performance: Option<Decimal>,
}

impl BacktestingResult {
    pub fn calculate(
        name: &str, date: Date, net_value: Cash, method: BenchmarkPerformanceType,
        transactions: &[Transaction], with_performance: bool,
    ) -> GenericResult<BacktestingResult> {
        let mut result = BacktestingResult {
            date,
            net_value: net_value.round().amount,
            performance: None,
        };

        if with_performance {
            let name = format!("{} @ {}", name, formatting::format_date(date));

            let transactions = method.adjust_transactions(net_value.currency, date, transactions)?;
            let interest_periods = [InterestPeriod::new(transactions.first().unwrap().date, date)];

            result.performance = compare_instrument_to_bank_deposit(
                &name, net_value.currency, &transactions, &interest_periods, net_value.amount)?;
        }

        Ok(result)
    }
}