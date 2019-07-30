// FIXME: Rename module
#![allow(dead_code, unused_imports)]  // FIXME: Remove

use num_traits::ToPrimitive;
use prettytable::{Table as RawTable, Row as RawRow, Cell as RawCell};
use prettytable::format::{FormatBuilder, LinePosition, LineSeparator};
use separator::Separatable;
use term;

use crate::currency::{Cash, MultiCurrencyCashAccount};
use crate::formatting::old_table::print_table;
use crate::types::{Date, Decimal};
use crate::util;

pub use ansi_term::Style;
pub use prettytable::format::Alignment;

pub struct Table {
    columns: Vec<Column>,
    rows: Vec<Vec<Cell>>,
}

impl Table {
    pub fn new(columns: Vec<Column>) -> Table {
        Table {
            columns,
            rows: Vec::new(),
        }
    }

    pub fn add_row(&mut self, row: Row) -> &mut Row {
        assert_eq!(row.len(), self.columns.len());
        self.rows.push(row);
        self.rows.last_mut().unwrap()
    }

    // FIXME: Rewrite
    pub fn print(&self, title: &str) {
        let mut table = RawTable::new();
        for row in &self.rows {
            table.add_row(RawRow::new(row.iter().enumerate().map(|(column_id, cell)| {
                let column = &self.columns[column_id];
                cell.render(column)
            }).collect()));
        }

        let column_names: Vec<&str> = self.columns.iter().map(|column| column.name).collect();
        print_table(title, &column_names, table);
    }
}

#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct Column {
    name: &'static str,
    alignment: Option<Alignment>,
}

impl Column {
    pub fn new(name: &'static str, alignment: Option<Alignment>) -> Column {
        Column {name, alignment}
    }
}

pub type Row = Vec<Cell>;

pub struct Cell {
    text: String,
    default_alignment: Alignment,
    style: Option<Style>,
}

impl Cell {
    fn new(text: String, default_alignment: Alignment) -> Cell {
        Cell {text, default_alignment, style: None}
    }

    pub fn new_round_decimal(value: Decimal) -> Cell {
        Cell::new(value.to_i64().unwrap().separated_string(), Alignment::RIGHT)
    }

    pub fn style(&mut self, style: Style) -> &mut Cell {
        self.style = Some(style);
        self
    }

    fn render(&self, column: &Column) -> RawCell {
        let alignment = column.alignment.unwrap_or(self.default_alignment);
        match self.style {
            Some(style) => {
                let text = style.paint(&self.text).to_string();
                RawCell::new_align(&text, alignment)
            },
            None => RawCell::new_align(&self.text, alignment),
        }
    }
}

impl Into<Cell> for String {
    fn into(self) -> Cell {
        Cell::new(self, Alignment::LEFT)
    }
}

#[cfg(test)]
mod tests {
    use static_table_derive::StaticTable;
    use super::*;

    #[derive(StaticTable)]
    #[table(name="TestTable")]
    struct TestRow {
        a: String,
        #[column(name="Колонка B")]
        b: String,
        #[column(align="center")]
        c: String,
    }

    #[test]
    fn test() {
        let mut table = TestTable::new();

        assert_eq!(table.raw_table.columns, vec![
            Column {name: "a", alignment: None},
            Column {name: "Колонка B", alignment: None},
            Column {name: "c", alignment: Some(Alignment::CENTER)},
        ]);

        // FIXME: Delete / something else?
        table.add_row(TestRow {
            a: s!("A"),
            b: s!("B"),
            c: s!("C"),
        });
    }
}