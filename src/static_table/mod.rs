// FIXME: https://github.com/Calmynt/Tablefy
// rust procedural macros and attributes tutorial
// https://doc.rust-lang.org/book/ch19-06-macros.html
// Reference - https://doc.rust-lang.org/reference/macros.html

struct Table {
    columns: Vec<Column>,
    rows: Vec<Vec<String>>,
}

impl Table {
    fn add_row(&mut self, row: &dyn Row) {
        let row = row.render();
        assert_eq!(row.len(), self.columns.len());
        self.rows.push(row);
    }
}

#[cfg_attr(test, derive(Debug, PartialEq))]
struct Column {
    id: &'static str,
    name: Option<&'static str>,
}

trait Row {
    fn render(&self) -> Vec<String>;
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

        table.add_row(&TestRow {
            a: s!("A"),
            b: s!("B"),
        });
    }
}