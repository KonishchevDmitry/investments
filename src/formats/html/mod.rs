mod parser;
mod table;
mod util;

// Reuse as much as we can from XLS parser
pub use crate::formats::xls::TableColumn;
pub use crate::formats::xls::cell::*;

pub use self::parser::*;
pub use self::table::*;
pub use self::util::*;

pub use xls_table_derive::HtmlTableRow;