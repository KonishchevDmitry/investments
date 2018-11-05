use prettytable::{Table, Row, Cell};
use prettytable::format::{Alignment, FormatBuilder, LinePosition, LineSeparator};

use types::Date;

pub fn format_date(date: Date) -> String {
    date.format("%d.%m.%Y").to_string()
}

pub fn print_statement(name: &str, titles: Vec<&str>, mut statement: Table) {
    statement.set_format(FormatBuilder::new().padding(1, 1).build());
    statement.set_titles(Row::new(
        titles.iter().map(|name| Cell::new_align(*name, Alignment::CENTER)).collect()));

    let mut table = Table::new();

    table.set_format(FormatBuilder::new()
        .separator(LinePosition::Title, LineSeparator::new(' ', ' ', ' ', ' '))
        .build());

    table.set_titles(Row::new(vec![
        Cell::new_align(&("\n".to_owned() + name), Alignment::CENTER),
    ]));

    table.add_row(Row::new(vec![
        Cell::new_align(&statement.to_string(), Alignment::CENTER),
    ]));

    table.printstd();
}