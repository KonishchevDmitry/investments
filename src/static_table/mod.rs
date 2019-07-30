// FIXME: https://github.com/Calmynt/Tablefy
// rust procedural macros and attributes tutorial
// https://doc.rust-lang.org/book/ch19-06-macros.html
// Reference - https://doc.rust-lang.org/reference/macros.html

#[cfg(test)]
mod tests {
    use super::*;

    use static_table_derive::StaticTable;

    pub trait HelloMacro {
        fn hello_macro();
    }

    #[derive(StaticTable)]
    #[table(name="TestTable")]
    struct TestRow {
        a: String,
        #[cell(name="some-name")]
        b: String,
    }

    #[test]
    fn test() {
        TestRow::hello_macro();
    }
}