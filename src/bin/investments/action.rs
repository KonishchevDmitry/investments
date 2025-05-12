use std::path::PathBuf;

use investments::analysis::backtesting::BenchmarkPerformanceType;
use investments::analysis::performance::types::PerformanceAnalysisMethod;
use investments::time::Date;
use investments::types::Decimal;

pub enum Action {
    Analyse {
        name: Option<String>,
        method: PerformanceAnalysisMethod,
        show_closed_positions: bool,
    },
    Backtest {
        name: Option<String>,
        method: BenchmarkPerformanceType,
        backfill: bool,
    },
    SimulateSell {
        name: String,
        positions: Option<Vec<(String, Option<Decimal>)>>,
        base_currency: Option<String>,
    },

    Sync(String),
    Buy {
        name: String,
        positions: Vec<(String, Decimal)>,
        cash_assets: Decimal,
    },
    Sell {
        name: String,
        positions: Vec<(String, Option<Decimal>)>,
        cash_assets: Decimal,
    },
    SetCashAssets(String, Decimal),

    Show {
        name: String,
        flat: bool,
    },
    Rebalance {
        name: String,
        flat: bool,
    },

    TaxStatement {
        name: String,
        year: Option<i32>,
        tax_statement_path: Option<PathBuf>,
    },
    CashFlow {
        name: String,
        year: Option<i32>,
    },

    Deposits {
        date: Date,
        cron_mode: bool,
    },

    Metrics(PathBuf),
    ShellCompletion {
        path: PathBuf,
        data: Vec<u8>,
    }
}