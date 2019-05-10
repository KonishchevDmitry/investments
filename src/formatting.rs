use chrono::Duration;
use num_traits::ToPrimitive;

use prettytable::{Row as RawRow, Cell as RawCell};
use prettytable::format::{FormatBuilder, LinePosition, LineSeparator};

use separator::Separatable;

use crate::currency::{Cash, MultiCurrencyCashAccount};
use crate::types::{Date, Decimal};
use crate::util;

pub use prettytable::{Table, format::Alignment};

pub fn format_date(date: Date) -> String {
    date.format("%d.%m.%Y").to_string()
}

pub fn format_period(start: Date, end: Date) -> String {
    format!("{} - {}", format_date(start), format_date(end - Duration::days(1)))
}

#[derive(Clone)]
pub struct Cell {
    text: String,
    align: Alignment,
}

impl Cell {
    pub fn new(text: &str) -> Cell {
        Cell::new_align(text, Alignment::LEFT)
    }

    pub fn new_align(text: &str, align: Alignment) -> Cell {
        Cell {
            text: text.to_owned(),
            align: align,
        }
    }
}

pub fn empty_cell() -> Cell {
    Cell::new("")
}

pub fn date_cell(date: Date) -> Cell {
    Cell::new_align(&format_date(date), Alignment::CENTER)
}

pub fn decimal_cell(value: Decimal) -> Cell {
    Cell::new_align(&value.to_string(), Alignment::RIGHT)
}

pub fn round_decimal_cell(value: Decimal) -> Cell {
    Cell::new_align(&value.to_i64().unwrap().separated_string(), Alignment::RIGHT)
}

pub fn cash_cell(amount: Cash) -> Cell {
    Cell::new_align(&amount.format(), Alignment::RIGHT)
}

pub fn multi_currency_cash_cell(amounts: MultiCurrencyCashAccount) -> Cell {
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

pub fn ratio_cell(ratio: Decimal) -> Cell {
    Cell::new_align(&format!("{}%", util::round_to(ratio * dec!(100), 1)), Alignment::RIGHT)
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

pub fn print_statement(name: &str, titles: &[&str], mut statement: Table) {
    statement.set_format(FormatBuilder::new().padding(1, 1).build());
    statement.set_titles(RawRow::new(
        titles.iter().map(|name| RawCell::new_align(*name, Alignment::CENTER)).collect()));

    let mut table = Table::new();

    table.set_format(FormatBuilder::new()
        .separator(LinePosition::Title, LineSeparator::new(' ', ' ', ' ', ' '))
        .build());

    table.set_titles(RawRow::new(vec![
        RawCell::new_align(&("\n".to_owned() + name), Alignment::CENTER),
    ]));

    table.add_row(RawRow::new(vec![RawCell::new(&statement.to_string())]));
    table.printstd();
}