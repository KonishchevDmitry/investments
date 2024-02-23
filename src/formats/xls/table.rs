use std::borrow::Cow;

use itertools::Itertools;
use lazy_static::lazy_static;
use log::trace;
use regex::{self, Regex, RegexBuilder};

use crate::core::GenericResult;

use super::{SheetReader, Cell, is_empty_row};

pub trait TableRow: Sized {
    fn columns() -> Vec<TableColumn>;
    fn trim_column_title(title: &str) -> Cow<str>;
    fn parse(row: &[Option<&Cell>]) -> GenericResult<Self>;
}

pub trait TableReader {
    fn next_row(sheet: &mut SheetReader) -> Option<&[Cell]> {
        sheet.next_row().and_then(|row| {
            if is_empty_row(row) {
                None
            } else {
                Some(row)
            }
        })
    }

    fn skip_row(_row: &[Option<&Cell>]) -> GenericResult<bool> {
        Ok(false)
    }
}

pub fn read_table<T: TableRow + TableReader>(sheet: &mut SheetReader) -> GenericResult<Vec<T>> {
    let mut table = Vec::new();
    let columns = T::columns();
    let repeatable_table_column_titles = sheet.repeatable_table_column_titles();

    trace!("Reading {} table starting from #{} row...", std::any::type_name::<T>(), sheet.next_human_row_id());

    let mut columns_mapping = match map_columns(sheet.next_row_checked()?, &columns, T::trim_column_title) {
        Ok(mapping) => mapping,
        Err(err) => {
            if T::next_row(sheet).is_none() && !sheet.parse_empty_tables() {
                trace!("Skip empty {} table.", std::any::type_name::<T>());
                return Ok(table);
            }
            return Err(err);
        },
    };

    while let Some(row) = T::next_row(sheet) {
        if repeatable_table_column_titles {
            if let Ok(new_mapping) = map_columns(row, &columns, T::trim_column_title) {
                columns_mapping = new_mapping;
                continue;
            }
        }

        let mapped_row = columns_mapping.map(row)?;
        if T::skip_row(&mapped_row)? {
            continue;
        }

        table.push(TableRow::parse(&mapped_row)?);
    }

    Ok(table)
}

pub struct TableColumn {
    name: &'static str,
    regex: bool,
    aliases: &'static [&'static str],
    case_insensitive: bool,
    space_insensitive: bool,
    optional: bool,
}

impl TableColumn {
    pub fn new(
        name: &'static str, regex: bool, aliases: &'static [&'static str],
        case_insensitive: bool, space_insensitive: bool, optional: bool
    ) -> TableColumn {
        TableColumn {name, regex, aliases, case_insensitive, space_insensitive, optional}
    }

    fn find(&self, row: &[Cell], trim_title: fn(&str) -> Cow<str>) -> GenericResult<Option<usize>> {
        for (cell_id, cell) in row.iter().enumerate() {
            match cell {
                Cell::String(value) => {
                    return if self.matches(&trim_title(value))? {
                        Ok(Some(cell_id))
                    } else if self.optional {
                        Ok(None)
                    } else {
                        Err!("Unable to find {:?} column - got {:?} instead", self.name, value)
                    };
                },
                Cell::Empty => {}
                _ => return Err!(
                    "Unable to find {:?} column - got an unexpected {:?} cell", self.name, cell),
            };
        }

        if self.optional {
            Ok(None)
        } else {
            Err!("The table has no {:?} column", self.name)
        }
    }

    fn matches(&self, value: &str) -> GenericResult<bool> {
        lazy_static! {
            static ref EXTRA_SPACES_REGEX: Regex = Regex::new(r" {2,}").unwrap();
        }

        let value = self.transform_for_matching(EXTRA_SPACES_REGEX.replace_all(value.trim(), " "));
        let mut names = Vec::from(self.aliases);

        if self.regex {
            let regex = RegexBuilder::new(self.name)
                .case_insensitive(self.case_insensitive)
                .ignore_whitespace(self.space_insensitive)
                .build().map_err(|_| format!("Invalid column name regex: {:?}", self.name))?;

            if regex.is_match(&value) {
                return Ok(true);
            }
        } else {
            names.push(self.name);
        }

        if self.space_insensitive {
            for name in names {
                if value == self.transform_for_matching(name) {
                    return Ok(true);
                }
            }
        } else {
            let value_regex = RegexBuilder::new(
                &format!("^{}$", regex::escape(&value).replace('\r', "").replace('\n', " ?")))
                .case_insensitive(self.case_insensitive)
                .build().unwrap();

            for name in names {
                if value_regex.is_match(name) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    fn transform_for_matching<'a, V: Into<Cow<'a, str>>>(&self, value: V) -> Cow<'a, str> {
        let mut value = value.into();

        if self.space_insensitive {
            lazy_static! {
                static ref SPACE_REGEX: Regex = Regex::new(r"\s+").unwrap();
            }
            value = SPACE_REGEX.replace_all(&value, "").into_owned().into();
        }

        if self.case_insensitive {
            value = value.to_lowercase().into();
        }

        value
    }
}

pub struct ColumnsMapping {
    mapping: Vec<Option<usize>>,
}

impl ColumnsMapping {
    pub fn get<'a>(&self, row: &'a[Cell], column_id: usize) -> GenericResult<Option<&'a Cell>> {
        if column_id >= self.mapping.len() {
            return Err!("Invalid column ID: {}", column_id);
        }

        Ok(self.map_id(row, column_id)?.map(|cell_id| &row[cell_id]))
    }

    pub fn map<'a>(&self, row: &'a[Cell]) -> GenericResult<Vec<Option<&'a Cell>>> {
        let mut mapped_row = Vec::with_capacity(self.mapping.len());
        let mut current_cell_id = 0;

        for column_id in 0..self.mapping.len() {
            let mapped_cell_id = match self.map_id(row, column_id)? {
                Some(cell_id) => cell_id,
                None => {
                    mapped_row.push(None);
                    continue;
                },
            };

            if current_cell_id < mapped_cell_id {
                let spare_cells = &row[current_cell_id..mapped_cell_id];
                if !is_empty_row(spare_cells) {
                    return Err!(
                        "The row contains non-empty cells between column cells: {:?}", spare_cells);
                }
            } else {
                assert_eq!(current_cell_id, mapped_cell_id);
            }

            mapped_row.push(Some(&row[mapped_cell_id]));
            current_cell_id = mapped_cell_id + 1;
        }

        let spare_cells = &row[current_cell_id..];
        if !is_empty_row(spare_cells) {
            return Err!("The row contains non-empty cells after column cells: {:?}", spare_cells);
        }

        Ok(mapped_row)
    }

    fn map_id(&self, row: &[Cell], column_id: usize) -> GenericResult<Option<usize>> {
        let cell_id = match self.mapping[column_id] {
            Some(cell_id) => cell_id,
            None => return Ok(None),
        };

        if cell_id >= row.len() {
            return Err!("The row have no {}:{} column", column_id, cell_id);
        }

        Ok(Some(cell_id))
    }
}

pub fn map_columns(mut row: &[Cell], columns: &[TableColumn], trim_title: fn(&str) -> Cow<str>) -> GenericResult<ColumnsMapping> {
    let mut mapping = Vec::with_capacity(columns.len());
    let mut offset = 0;

    for column in columns {
        let cell_id = match column.find(row, trim_title)? {
            Some(index) => {
                row = &row[index + 1..];
                let cell_id = offset + index;
                offset += index + 1;
                Some(cell_id)
            }
            None => None,
        };
        mapping.push(cell_id);
    }

    if !is_empty_row(row) {
        return Err!(
            "The table has more columns than expected: {:?}",
            row.iter().filter_map(|cell| {
                match cell {
                    Cell::Empty => None,
                    _ => Some(cell.to_string()),
                }
            }).format(", "))
    }

    Ok(ColumnsMapping { mapping })
}