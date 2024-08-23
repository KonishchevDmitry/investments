// XXX(konishchev): Rewrite
use scraper::ElementRef;

use crate::core::GenericResult;
use crate::formats::xls::{self, Cell, TableRow};

use super::util;

pub fn read_table<T: TableRow>(element: ElementRef) -> GenericResult<Vec<T>> {
    let mut table = Vec::new();
    let columns = T::columns();

    // trace!("Reading {} table starting from #{} row...", std::any::type_name::<T>(), sheet.next_human_row_id());

    let element = util::select_one(element, "tbody")?;
    let mut rows = element.child_elements().peekable();

    // XXX(konishchev): HERE
    let mut header = rows.next().unwrap();
    if let Some(row) = rows.peek() {
        if row.value().has_class("table-header", scraper::CaseSensitivity::CaseSensitive) {
            header = rows.next().unwrap();
        }
    }

    let header: Vec<Cell> = util::select_multiple(header, "td")?.into_iter().map(|cell| {
        Cell::String(util::textify(cell).trim_end_matches(&['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹']).into())
    }).collect();
    println!("header: {header:?}");

    let mut columns_mapping = match xls::map_columns(&header, &columns, T::trim_column_title) {
        Ok(mapping) => mapping,
        Err(err) => {
            // if T::next_row(sheet).is_none() && !sheet.parse_empty_tables() {
            //     trace!("Skip empty {} table.", std::any::type_name::<T>());
            //     return Ok(table);
            // }
            return Err(err);
        },
    };

    while let Some(row) = rows.next() {
        // if repeatable_table_column_titles {
        //     if let Ok(new_mapping) = map_columns(row, &columns, T::trim_column_title) {
        //         columns_mapping = new_mapping;
        //         continue;
        //     }
        // }

        if row.value().has_class("summary-row", scraper::CaseSensitivity::CaseSensitive) {
            continue;
        }
        if row.value().has_class("rn", scraper::CaseSensitivity::CaseSensitive) {
            continue;
        }
        if util::select_multiple(row, "td")?.len() == 1 {
            continue;
        }

        let row: Vec<Cell> = util::select_multiple(row, "td")?.into_iter().map(|cell| {
            Cell::String(util::textify(cell))
        }).collect();
        println!("row: {row:?}");

        let mapped_row = columns_mapping.map(&row)?;
        // if T::skip_row(&mapped_row)? {
        //     continue;
        // }

        table.push(TableRow::parse(&mapped_row)?);
    }

    Ok(table)
}