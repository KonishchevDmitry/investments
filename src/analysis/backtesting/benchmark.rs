use crate::core::GenericResult;
use crate::currency::CashAssets;
use crate::time::Date;
use crate::types::Decimal;

use super::BenchmarkPerformanceType;

pub trait Benchmark {
    fn name(&self) -> String;
    fn provider(&self) -> Option<String>;

    fn backtest(
        &self, method: BenchmarkPerformanceType, currency: &str, cash_flows: &[CashAssets], today: Date, full: bool,
    ) -> GenericResult<Vec<BacktestingResult>>;
}

pub struct BacktestingResult {
    pub date: Date,
    pub net_value: Decimal,
    pub performance: Option<Decimal>,
}