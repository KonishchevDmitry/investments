use std::path::PathBuf;

use investments::time::Date;
use investments::types::Decimal;

pub enum Action {
    Analyse {
        name: Option<String>,
        show_closed_positions: bool,
    },
    SimulateSell {
        name: String,
        positions: Vec<(String, Option<Decimal>)>,
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
        tax_statement_path: Option<String>,
    },
    CashFlow {
        name: String,
        year: Option<i32>,
    },

    Deposits {
        date: Date,
        cron_mode: bool,
    },

    Metrics(String),
    ShellCompletion {
        path: PathBuf,
        data: Vec<u8>,
    }
}