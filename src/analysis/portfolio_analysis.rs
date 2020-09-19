use std::collections::BTreeMap;

use static_table_derive::StaticTable;

use crate::formatting::table::{Cell, Style};
use crate::types::Decimal;
use crate::util;

pub struct PortfolioPerformanceAnalysis {
    pub income_structure: IncomeStructure,
    pub instruments: BTreeMap<String, InstrumentPerformanceAnalysis>,
    pub portfolio: InstrumentPerformanceAnalysis,
}

impl PortfolioPerformanceAnalysis {
    pub fn print(&self, name: &str) {
        let mut table = Table::new();

        for analysis in self.instruments.values() {
            analysis.format(&mut table, &analysis.name);
        }
        self.portfolio.format(&mut table, "");

        table.print(name);
    }
}

#[derive(Default)]
pub struct IncomeStructure {
    pub net_profit: Decimal,

    pub dividends: Decimal,
    pub interest: Decimal,
    pub tax_deductions: Decimal,

    pub taxes: Decimal,
    pub commissions: Decimal,
}

impl IncomeStructure {
    pub fn trading_profit(&self) -> Decimal {
        self.net_profit + self.taxes + self.commissions
            - self.dividends
            - self.interest
            - self.tax_deductions
    }
}

pub struct InstrumentPerformanceAnalysis {
    pub name: String,
    pub days: u32,
    pub investments: Decimal,
    pub result: Decimal,
    pub interest: Decimal,
    pub inactive: bool,
}

#[derive(StaticTable)]
struct Row {
    #[column(name="Instrument")]
    instrument: String,
    #[column(name="Investments")]
    investments: Cell,
    #[column(name="Profit")]
    profit: Cell,
    #[column(name="Result")]
    result: Cell,
    #[column(name="Duration", align="right")]
    duration: String,
    #[column(name="Interest", align="right")]
    interest: String,
}

impl InstrumentPerformanceAnalysis {
    pub fn net_profit(&self) -> Decimal {
        self.result - self.investments
    }

    fn format(&self, table: &mut Table, name: &str) {
        let investments = util::round(self.investments, 0);
        let result = util::round(self.result, 0);
        let profit = result - investments;

        let (duration_name, duration_days) = if self.days >= 365 {
            ("y", 365)
        } else if self.days >= 30 {
            ("m", 30)
        } else {
            ("d", 1)
        };
        let duration = format!(
            "{}{}", util::round(Decimal::from(self.days) / Decimal::from(duration_days), 1),
            duration_name);

        let mut row = table.add_row(Row {
            instrument: name.to_owned(),
            investments: Cell::new_round_decimal(investments),
            profit: Cell::new_round_decimal(profit),
            result: Cell::new_round_decimal(result),
            duration: duration,
            interest: format!("{}%", self.interest),
        });

        if self.inactive {
            let style = Style::new().dimmed();
            for cell in &mut row {
                cell.style(style);
            }
        }
    }
}