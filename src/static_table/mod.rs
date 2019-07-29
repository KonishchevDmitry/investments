// FIXME: https://github.com/Calmynt/Tablefy
// rust procedural macros and attributes tutorial
// https://doc.rust-lang.org/book/ch19-06-macros.html
// Reference - https://doc.rust-lang.org/reference/macros.html

use static_table_derive::StaticTable;

pub trait HelloMacro {
    fn hello_macro();
}

#[derive(StaticTable)]
#[table(name="my table")]
struct Pancakes {
    a: String,
    #[cell(name="some-name", description="A B C",)]
    b: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        Pancakes::hello_macro();
    }
}