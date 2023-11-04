use std::fs;
use std::path::Path;

use bitflags::bitflags;
use log::debug;

use crate::core::{GenericResult, EmptyResult};
use crate::brokers::Broker;
use crate::taxes::TaxRemapping;

use super::{bcs, firstrade, ib, open, tinkoff};
use super::PartialBrokerStatement;

bitflags! {
    #[derive(Clone, Copy)]
    pub struct ReadingStrictness: u32 {
        const TRADE_SETTLE_DATE = 1 << 0;
        const CASH_FLOW_DATES   = 1 << 1;
        const TAX_EXEMPTIONS    = 1 << 2;
        const REPO_TRADES       = 1 << 3;
        const GRANTS            = 1 << 4;
    }
}

pub trait BrokerStatementReader {
    fn check(&mut self, path: &str) -> GenericResult<bool>;
    fn read(&mut self, path: &str, is_last: bool) -> GenericResult<PartialBrokerStatement>;
    #[allow(clippy::boxed_local)]
    fn close(self: Box<Self>) -> EmptyResult { Ok(()) }
}

pub fn read(
    broker: Broker, statement_dir_path: &str, tax_remapping: TaxRemapping,
    strictness: ReadingStrictness,
) -> GenericResult<Vec<PartialBrokerStatement>> {
    let mut tax_remapping = Some(tax_remapping);
    let mut statement_reader = match broker {
        Broker::Bcs => bcs::StatementReader::new(),
        Broker::Firstrade => firstrade::StatementReader::new(),
        Broker::InteractiveBrokers => ib::StatementReader::new(
            tax_remapping.take().unwrap(), strictness),
        Broker::Open => open::StatementReader::new(),
        Broker::Tinkoff => tinkoff::StatementReader::new(),
    }?;

    let mut file_names = preprocess_statement_directory(statement_dir_path, statement_reader.as_mut())
        .map_err(|e| format!("Error while reading {:?}: {}", statement_dir_path, e))?;

    if file_names.is_empty() {
        return Err!("{:?} doesn't contain any broker statement", statement_dir_path);
    }
    file_names.sort_unstable();

    let mut statements = Vec::new();

    for (id, file_name) in file_names.iter().enumerate() {
        let is_last = id == file_names.len() - 1;

        let path = Path::new(statement_dir_path).join(file_name);
        let path = path.to_str().unwrap();

        debug!("Reading {:?}...", path);
        let statement = statement_reader.read(path, is_last).map_err(|e| format!(
            "Error while reading {:?} broker statement: {}", path, e))?;

        statements.push(statement);
    }

    if let Some(tax_remapping) = tax_remapping {
        tax_remapping.ensure_all_mapped().map_err(|e| format!(
            "{}. Tax remapping is not supported for {} yet", e, broker.brief_name()))?;
    }
    statement_reader.close()?;

    Ok(statements)
}

fn preprocess_statement_directory(
    statement_dir_path: &str, statement_reader: &mut dyn BrokerStatementReader
) -> GenericResult<Vec<String>> {
    let mut file_names = Vec::new();

    for entry in fs::read_dir(statement_dir_path)? {
        let entry = entry?;

        let path = entry.path();
        let path = path.to_str().ok_or_else(|| format!(
            "Got an invalid path: {:?}", path.to_string_lossy()))?;

        if !statement_reader.check(path)? {
            continue;
        }

        let file_name = entry.file_name().into_string().map_err(|file_name| format!(
            "Got an invalid file name: {:?}", file_name.to_string_lossy()))?;
        file_names.push(file_name);
    }

    Ok(file_names)
}