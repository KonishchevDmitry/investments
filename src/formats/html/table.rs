use log::trace;
use scraper::ElementRef;

use crate::core::GenericResult;
use crate::formats::xls::{self, Cell, TableRow};

use super::util;

pub fn read_table<T: TableRow>(element: ElementRef) -> GenericResult<Vec<T>> {
    let (header, mut rows) = get_table_boundaries(element).map_err(|e| format!(
        "{e}:\n{}", element.html()))?;

    let columns = T::columns();
    let header_cells = read_table_row(header)?;

    let columns_mapping = xls::map_columns(&header_cells, &columns, T::trim_column_title).map_err(|e| format!(
        "Unable to map {} on the following table header ({e}):\n{}", std::any::type_name::<T>(), header.html()))?;

    let mut table = Vec::new();

    while let Some(row) = rows.next() {
        // XXX(konishchev): HERE
        // if row.value().has_class("summary-row", scraper::CaseSensitivity::CaseSensitive) {
        //     continue;
        // }
        if row.value().has_class("rn", scraper::CaseSensitivity::CaseSensitive) {
            continue;
        }
        if util::select_multiple(row, "td")?.len() == 1 {
            continue;
        }

        let row_cells = read_table_row(row)?;
        // println!("row: {row:?}");

        let mapped_row = columns_mapping.map(&row_cells).map_err(|e| format!(
            "Unable to map {} on the following row ({e}):\n{}", std::any::type_name::<T>(), row.html()))?;
        // if T::skip_row(&mapped_row)? {
        //     continue;
        // }

        table.push(TableRow::parse(&mapped_row)?);
    }

    Ok(table)
}

fn get_table_boundaries(element: ElementRef) -> GenericResult<(ElementRef, impl Iterator<Item=ElementRef>)> {
    let element = util::select_one(element, "tbody")?;
    let mut rows = element.child_elements();

    loop {
        let header = rows.next().ok_or_else(|| "Unable to find the table header")?;
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