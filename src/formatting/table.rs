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
    rows: Vec<Row>,
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

    pub fn hide_column(&mut self, index: usize) {
        self.columns[index].hidden = true;
    }

    // FIXME: Rewrite
    pub fn print(&self, title: &str) {
        let mut table = RawTable::new();
        let columns: Vec<_> = self.columns.iter().enumerate().filter_map(|(index, column)| if column.hidden {
            None
        } else {
            Some(index)
        }).collect();

        for row in &self.rows {
            table.add_row(RawRow::new(columns.iter().map(|&index| {
                let column = &self.columns[index];
                let cell = &row[index];
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
    hidden: bool,
}

impl Column {
    pub fn new(name: &'static str, alignment: Option<Alignment>) -> Column {
        Column {name, alignment, hidden: false}
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

        assert_eq!(table.table.columns, vec![
            Column {name: "a", alignment: None, hidden: false},
            Column {name: "Колонка B", alignment: None, hidden: false},
            Column {name: "c", alignment: Some(Alignment::CENTER), hidden: false},
        ]);

        table.hide_b();
        assert!(table.table.columns[1].hidden);

        let mut row = table.add_row(TestRow {
            a: s!("A"),
            b: s!("B"),
            c: s!("C"),
        });
        row.set_b(Cell::new(s!("BB"), Alignment::RIGHT));
        assert_eq!(table.table.rows.last().unwrap()[1].text, "BB");
    }
}