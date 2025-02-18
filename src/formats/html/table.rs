use std::borrow::Cow;

use log::trace;
use scraper::ElementRef;

use crate::core::GenericResult;
use crate::formats::xls::{self, Cell, TableColumn};

use super::util;

pub type RawRowType<'a> = ElementRef<'a>;

pub trait TableRow: Sized {
    fn columns() -> Vec<TableColumn>;
    fn skip_row(row: RawRowType) -> bool;
    fn trim_column_title(title: &str) -> Cow<str>;
    fn parse(row: &[Option<&Cell>]) -> GenericResult<Self>;
}

pub fn read_table<T: TableRow>(element: ElementRef) -> GenericResult<Vec<T>> {
    let (header, rows) = get_table_boundaries(element).map_err(|e| format!(
        "{e}:\n{}", element.html()))?;

    let columns = T::columns();
    let header_cells = read_table_row(header)?;

    let columns_mapping = xls::map_columns(&header_cells, &columns, T::trim_column_title).map_err(|e| format!(
        "Unable to map {} on the following table header ({e}):\n{}", std::any::type_name::<T>(), header.html()))?;

    let mut table = Vec::new();

    for row in rows {
        if T::skip_row(row) {
            trace!("Skipping the following row:\n{}", row.html());
            continue;
        }

        let row_cells = read_table_row(row)?;
        let parsed_row = columns_mapping.map(&row_cells)
            .and_then(|mapped_cells| TableRow::parse(&mapped_cells))
            .map_err(|e| format!(
                "Unable to map {} on the following row ({e}):\n{}",
                std::any::type_name::<T>(), row.html(),
            ))?;


        table.push(parsed_row);
    }

    Ok(table)
}

fn get_table_boundaries(element: ElementRef) -> GenericResult<(ElementRef, impl Iterator<Item=ElementRef>)> {
    let element = util::select_one(element, "tbody")?;
    let mut rows = element.child_elements();

    loop {
        let header = rows.next().ok_or("Unable to find the table header")?;
        let columns = util::select_multiple(header, "td")?;

        if columns.iter().any(|column| column.attr("colspan").unwrap_or("1") != "1") {
            trace!("Nested header detected. Ignoring it:\n{}", header.html());
            continue;
        }

        return Ok((header, rows))
    }
}

fn read_table_row(row: ElementRef) -> GenericResult<Vec<Cell>> {
    Ok(util::select_multiple(row, "td")?.into_iter().map(|cell| {
        Cell::String(util::textify(cell))
    }).collect())
}