#![allow(dead_code)]  // FIXME: Remove

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

    pub fn add_row(&mut self, row: Row) {
        assert_eq!(row.len(), self.columns.len());
        self.rows.push(row);
    }
}

#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct Column {
    name: &'static str,
}

impl Column {
    pub fn new(name: &'static str) -> Column {
        Column {name}
    }
}

pub type Row = Vec<Cell>;

pub struct Cell {
    value: String,
}

impl Cell {
    fn new(value: String) -> Cell {
        Cell {value}
    }
}

impl Into<Cell> for String {
    fn into(self) -> Cell {
        Cell::new(self)
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
    }

    #[test]
    fn test() {
        let mut table = TestTable::new();

        assert_eq!(table.raw_table.columns, vec![
            Column {name: "a"},
            Column {name: "Колонка B"},
        ]);

        table.add_row(TestRow {
            a: s!("A"),
            b: s!("B"),
        });
    }
}