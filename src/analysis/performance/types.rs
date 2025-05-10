use std::collections::BTreeMap;

use static_table_derive::StaticTable;

use crate::currency;
use crate::formatting::{self, table::{Cell, Style}};
use crate::types::Decimal;
use crate::util;

#[derive(Clone, Copy, PartialEq, Eq)]
#[derive(strum::Display, strum::EnumIter, strum::EnumMessage, strum::EnumString, strum::IntoStaticStr)]
#[strum(serialize_all = "kebab-case")]
pub enum PerformanceAnalysisMethod {
    #[strum(message = "don't take taxes into account")]
    Virtual,
    #[strum(message = "take taxes into account")]
    Real,
    #[strum(message = "take taxes and inflation into account")]
    InflationAdjusted,
}

impl PerformanceAnalysisMethod {
    pub fn tax_aware(self) -> bool {
        match self {
            PerformanceAnalysisMethod::Virtual => false,
            PerformanceAnalysisMethod::Real => true,
            PerformanceAnalysisMethod::InflationAdjusted => true,
        }
    }
}

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

    pub fn commit(self) -> Self {
        let instruments = self.instruments.into_iter().map(|(instrument, statistics)| {
            (instrument, statistics.commit())
        }).collect();

        PortfolioPerformanceAnalysis {
            income_structure: self.income_structure.commit(),
            instruments,
            portfolio: self.portfolio.commit(),
        }
    }
}

#[derive(Default)]
pub struct IncomeStructure {
    pub net_profit: Decimal,

    pub dividends: Decimal,
    pub interest: Decimal,

    pub trading_taxes: Decimal,
    pub dividend_taxes: Decimal,
    pub interest_taxes: Decimal,

    pub trading_tax_deductions: Decimal,
    pub additional_tax_deductions: Decimal,

    pub commissions: Decimal,

    // For now process it as a part of trading income since its amount is too small to make a distinct category for it
    pub grants: Decimal,
}

impl IncomeStructure {
    pub fn profit(&self) -> Decimal {
        self.net_profit + self.taxes() + self.commissions
    }

    pub fn net_trading_income(&self) -> Decimal {
        self.net_profit - self.net_dividend_income() - self.net_interest_income() - self.tax_deductions()
    }

    pub fn net_dividend_income(&self) -> Decimal {
        self.dividends - self.dividend_taxes
    }

    pub fn net_interest_income(&self) -> Decimal {
        self.interest - self.interest_taxes
    }

    pub fn taxes(&self) -> Decimal {
        self.trading_taxes + self.dividend_taxes + self.interest_taxes
    }

    pub fn tax_deductions(&self) -> Decimal {
        self.trading_tax_deductions + self.additional_tax_deductions
    }

    fn commit(self) -> Self {
        IncomeStructure {
            net_profit: currency::round(self.net_profit),

            dividends: currency::round(self.dividends),
            interest: currency::round(self.interest),

            trading_taxes: currency::round(self.trading_taxes),
            dividend_taxes: currency::round(self.dividend_taxes),
            interest_taxes: currency::round(self.interest_taxes),

            trading_tax_deductions: currency::round(self.trading_tax_deductions),
            additional_tax_deductions: currency::round(self.additional_tax_deductions),

            commissions: currency::round(self.commissions),
            grants: currency::round(self.grants),
        }
    }
}

pub struct InstrumentPerformanceAnalysis {
    pub name: String,
    pub days: u32,
    pub investments: Decimal,
    pub result: Decimal,
    pub performance: Option<Decimal>,
    pub inactive: bool,
}

impl InstrumentPerformanceAnalysis {
    fn commit(self) -> Self {
        InstrumentPerformanceAnalysis {
            name: self.name,
            days: self.days,
            investments: currency::round(self.investments),
            result: currency::round(self.result),
            performance: self.performance,
            inactive: self.inactive,
        }
    }
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
    #[column(name="Performance", align="right")]
    performance: Option<String>,
}

impl InstrumentPerformanceAnalysis {
    pub fn net_profit(&self) -> Decimal {
        self.result - self.investments
    }

    fn format(&self, table: &mut Table, name: &str) {
        let investments = util::round(self.investments, 0);
        let result = util::round(self.result, 0);
        let profit = result - investments;

        let mut row = table.add_row(Row {
            instrument: name.to_owned(),
            investments: Cell::new_round_decimal(investments),
            profit: Cell::new_round_decimal(profit),
            result: Cell::new_round_decimal(result),
            duration: formatting::format_days(self.days),
            performance: self.performance.map(|performance| format!("{}%", performance)),
        });

        if self.inactive {
            let style = Style::new().dimmed();
            for cell in &mut row {
                cell.style(style);
            }
        }
    }
}