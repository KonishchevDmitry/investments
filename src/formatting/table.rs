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

    pub fn add_empty_row(&mut self) -> &mut Row {
        let row = (0..self.columns.len()).map(|_| Cell::new_empty()).collect();
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

    fn new_empty() -> Cell {
        Cell::new(String::new(), Alignment::LEFT)
    }

    pub fn new_ratio(ratio: Decimal) -> Cell {
        Cell::new(format!("{}%", util::round_to(ratio * dec!(100), 1)), Alignment::RIGHT)
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

macro_rules! impl_from_number_to_cell {
    ($T:ty) => {
        impl From<$T> for Cell {
            fn from(value: $T) -> Cell {
                Cell::new(value.to_string(), Alignment::RIGHT)
            }
        }
    };
}
impl_from_number_to_cell!(u32);

impl From<String> for Cell {
    fn from(text: String) -> Cell {
        Cell::new(text, Alignment::LEFT)
    }
}

impl<T: Into<Cell>> From<Option<T>> for Cell {
    fn from(value: Option<T>) -> Cell {
        match value {
            Some(value) => value.into(),
            None => Cell::new_empty(),
        }
    }
}

impl From<Cash> for Cell {
    fn from(amount: Cash) -> Cell {
        Cell::new(amount.format(), Alignment::RIGHT)
    }
}

impl From<MultiCurrencyCashAccount> for Cell {
    fn from(amounts: MultiCurrencyCashAccount) -> Cell {
        let mut amounts: Vec<_> = amounts.iter()
            .map(|(currency, amount)| Cash::new(*currency, *amount))
            .collect();
        amounts.sort_by_key(|amount| amount.currency);

        let result = amounts.iter()
            .map(|amount| amount.format())
            .collect::<Vec<_>>()
            .join(" + ");

        Cell::new(result, Alignment::RIGHT)
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