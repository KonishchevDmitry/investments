//! This module provides a thin wrapper around prettytable.
//!
//! The main reason for it is to support ansi_term styling because term (which prettytable natively
//! supports) has not enough functionality - for example it doesn't support dimming style on Mac.

use num_traits::ToPrimitive;

use prettytable::{Row as RawRow, Cell as RawCell};
use prettytable::format::{FormatBuilder, LinePosition, LineSeparator};

use separator::Separatable;

use crate::currency::{Cash, MultiCurrencyCashAccount};
use crate::types::{Date, Decimal};
use crate::util;

pub use prettytable::{Table, format::Alignment};

#[derive(Clone)]
pub struct Cell {
    text: String,
    align: Alignment,
}

impl Cell {
    pub fn new(text: &str) -> Cell {
        Cell::new_align(text, Alignment::LEFT)
    }

    pub fn new_empty() -> Cell {
        Cell::new("")
    }

    pub fn new_align(text: &str, align: Alignment) -> Cell {
        Cell {
            text: text.to_owned(),
            align: align,
        }
    }

    pub fn new_date(date: Date) -> Cell {
        Cell::new_align(&super::format_date(date), Alignment::CENTER)
    }

    pub fn new_decimal(value: Decimal) -> Cell {
        Cell::new_align(&value.to_string(), Alignment::RIGHT)
    }

    pub fn new_round_decimal(value: Decimal) -> Cell {
        Cell::new_align(&value.to_i64().unwrap().separated_string(), Alignment::RIGHT)
    }

    pub fn new_cash(amount: Cash) -> Cell {
        Cell::new_align(&amount.format(), Alignment::RIGHT)
    }

    pub fn new_multi_currency_cash(amounts: MultiCurrencyCashAccount) -> Cell {
        let mut amounts: Vec<_> = amounts.iter()
            .map(|(currency, amount)| Cash::new(*currency, *amount))
            .collect();
        amounts.sort_by_key(|amount| amount.currency);

        let result = amounts.iter()
            .map(|amount| amount.format())
            .collect::<Vec<_>>()
            .join(" + ");

        Cell::new_align(&result, Alignment::RIGHT)
    }

    pub fn new_ratio(ratio: Decimal) -> Cell {
        Cell::new_align(&format!("{}%", util::round_to(ratio * dec!(100), 1)), Alignment::RIGHT)
    }
}

pub struct Row {
}

impl Row {
    pub fn new(row: &[Cell]) -> RawRow {
        let mut cells = Vec::with_capacity(row.len());

        for cell in row {
            cells.push(RawCell::new_align(&cell.text, cell.align));
        }

        RawRow::new(cells)
    }
}

pub fn print_table(name: &str, titles: &[&str], mut table: Table) {
    table.set_format(FormatBuilder::new().padding(1, 1).build());
    table.set_titles(RawRow::new(
        titles.iter().map(|name| RawCell::new_align(*name, Alignment::CENTER)).collect()));

    let mut wrapping_table = Table::new();

    wrapping_table.set_format(FormatBuilder::new()
        .separator(LinePosition::Title, LineSeparator::new(' ', ' ', ' ', ' '))
        .build());

    wrapping_table.set_titles(RawRow::new(vec![
        RawCell::new_align(&("\n".to_owned() + name), Alignment::CENTER),
    ]));

    wrapping_table.add_row(RawRow::new(vec![RawCell::new(&table.to_string())]));
    wrapping_table.printstd();
}