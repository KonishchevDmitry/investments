//! This module provides a thin wrapper around prettytable.
//!
//! The main reason for it is to support ansi_term styling because term (which prettytable natively
//! supports) has not enough functionality - for example it doesn't support dimming style on Mac.
// FIXME: Move to static table ^

use prettytable::{Row as RawRow, Cell as RawCell};
use prettytable::format::{FormatBuilder, LinePosition, LineSeparator};
use term;

pub use ansi_term::Style;
pub use prettytable::{Table, format::Alignment};

pub fn print_table(name: &str, titles: &[&str], mut table: Table) {
    table.set_format(FormatBuilder::new().padding(1, 1).build());
    table.set_titles(RawRow::new(
        titles.iter().map(|name| RawCell::new_align(*name, Alignment::CENTER)).collect()));

    let mut wrapping_table = Table::new();

    wrapping_table.set_format(FormatBuilder::new()
        .separator(LinePosition::Title, LineSeparator::new(' ', ' ', ' ', ' '))
        .build());

    wrapping_table.set_titles(RawRow::new(vec![
        RawCell::new_align(&("\n".to_owned() + name), Alignment::CENTER).with_style(term::Attr::Bold),
    ]));

    wrapping_table.add_row(RawRow::new(vec![RawCell::new(&table.to_string())]));
    wrapping_table.printstd();
}