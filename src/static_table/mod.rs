struct Table {
    columns: Vec<Column>,
    rows: Vec<Vec<Cell>>,
}

impl Table {
    fn add_row(&mut self, row: Vec<Cell>) {
        assert_eq!(row.len(), self.columns.len());
        self.rows.push(row);
    }
}

#[cfg_attr(test, derive(Debug, PartialEq))]
struct Column {
    id: &'static str,
    name: Option<&'static str>,
}

// FIXME: Do we need it?
trait Row {
    fn render(self) -> Vec<Cell>;
}

struct Cell {
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
        #[cell(name="Колонка B")]
        b: String,
    }

    #[test]
    fn test() {
        let mut table = TestTable::new();

        assert_eq!(table.raw_table.columns, vec![Column {
            id: "a", name: None,
        }, Column {
            id: "b", name: Some("Колонка B"),
        }]);

        table.add_row(TestRow {
            a: s!("A"),
            b: s!("B"),
        });
    }
}