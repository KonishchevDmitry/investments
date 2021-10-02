use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use matches::matches;
use xls_table_derive::XlsTableRow;

use crate::broker_statement::dividends::{DividendId, DividendAccruals};
use crate::broker_statement::taxes::TaxAccruals;
use crate::core::{GenericResult, EmptyResult};
use crate::currency::Cash;
use crate::formatting;
use crate::instruments::InstrumentId;
use crate::time::Date;
use crate::types::Decimal;
use crate::util::{self, DecimalRestrictions, RoundingMethod};
use crate::xls::{self, XlsStatementParser, SheetParser, SheetReader, Section, SectionParser,
                 TableReader, Cell, SkipCell};

use super::common::{parse_date_cell, parse_decimal_cell, parse_quantity_cell};

const SHEET_NAME: &str = "Отчет";
const TITLE_PREFIX: &str = "Отчет о выплате доходов по ценным бумагам иностранных эмитентов";

pub struct ForeignIncomeStatementReader {
}

impl ForeignIncomeStatementReader {
    #[allow(dead_code)] // FIXME(konishchev): Remove
    pub fn is_statement(path: &str) -> GenericResult<bool> {
        if !path.ends_with(".xlsx") {
            return Ok(false);
        }

        let sheet = match xls::open_sheet(path, SHEET_NAME)? {
            Some(sheet) => sheet,
            None => return Ok(false),
        };

        for mut row in sheet.rows() {
            row = xls::trim_row_right(row);
            if row.len() == 1 && matches!(&row[0], Cell::String(value) if value.starts_with(TITLE_PREFIX)) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    #[allow(dead_code)] // FIXME(konishchev): Remove
    fn read(path: &str) -> GenericResult<HashMap<DividendId, (DividendAccruals, TaxAccruals)>> {
        let parser = Box::new(ForeignIncomeSheetParser {});
        let foreign_income = Rc::new(RefCell::new(HashMap::new()));

        XlsStatementParser::read(path, parser, vec![
            Section::new(TITLE_PREFIX).by_prefix().required()
                .parser(Box::new(ForeignIncomeParser {income: foreign_income.clone()})),
        ])?;

        Ok(Rc::try_unwrap(foreign_income).ok().unwrap().into_inner())
    }
}

struct ForeignIncomeSheetParser {
}

impl SheetParser for ForeignIncomeSheetParser {
    fn sheet_name(&self) -> &str {
        SHEET_NAME
    }

    fn repeatable_table_column_titles(&self) -> bool {
        true
    }
}

struct ForeignIncomeParser {
    income: Rc<RefCell<HashMap<DividendId, (DividendAccruals, TaxAccruals)>>>,
}

impl SectionParser for ForeignIncomeParser {
    fn parse(&mut self, parser: &mut XlsStatementParser) -> EmptyResult {
        let mut all_income = self.income.borrow_mut();
        parser.sheet.next_row_checked()?;

        for income in xls::read_table::<ForeignIncomeRow>(&mut parser.sheet)? {
            let (dividend_id, amount, tax_withheld) = income.parse().map_err(|e| format!(
                "Error while parsing {:?} dividend income from {}: {}",
                income.name, formatting::format_date(income.date), e))?;

            let (dividends, taxes) = all_income.entry(dividend_id).or_insert_with(|| (
                DividendAccruals::new(true),
                TaxAccruals::new(true),
            ));

            dividends.add(income.date, amount);
            if !tax_withheld.is_zero() {
                taxes.add(income.date, tax_withheld);
            }
        }

        Ok(())
    }
}

#[derive(XlsTableRow)]
struct ForeignIncomeRow {
    #[column(name="Дата фиксации реестра")]
    _0: SkipCell,
    #[column(name="Дата выплаты", parse_with="parse_date_cell")]
    date: Date,
    #[column(name="Тип выплаты*")]
    type_: String,
    #[column(name="Наименование ценной бумаги")]
    name: String,
    #[column(name="ISIN")]
    isin: String,
    #[column(name="Страна эмитента")]
    _5: SkipCell,
    #[column(name="Количество ценных бумаг", parse_with="parse_quantity_cell")]
    quantity: u32,
    #[column(name="Выплата на одну бумагу", parse_with="parse_decimal_cell")]
    amount_per_stock: Decimal,
    #[column(name="Комиссия внешних платежных агентов**", parse_with="parse_decimal_cell")]
    commission: Decimal,
    #[column(name="Сумма налога, удержанного эмитентом", parse_with="parse_decimal_cell")]
    tax_withheld: Decimal,
    #[column(name="Итоговая сумма выплаты", parse_with="parse_decimal_cell")]
    paid_amount: Decimal,
    #[column(name="Валюта")]
    currency: String,
}

impl TableReader for ForeignIncomeRow {
    fn next_row(sheet: &mut SheetReader) -> Option<&[Cell]> {
        loop {
            let row = match sheet.next_row() {
                Some(row) => row,
                None => return None,
            };

            if let Some(Cell::String(value)) = row.iter().next() {
                if value.starts_with(TITLE_PREFIX) || value.starts_with("Депонент: ") {
                    continue;
                } else if value.starts_with("*Типы выплат: ") {
                    return None;
                }
            }

            return Some(unsafe {
                // Loop confuses the borrow checker
                std::slice::from_raw_parts(row.as_ptr(), row.len())
            })
        }
    }
}

impl ForeignIncomeRow {
    fn parse(&self) -> GenericResult<(DividendId, Cash, Cash)> {
        if self.type_.trim() != "1" {
            return Err!("Unsupported payment type: {:?}", self.type_);
        }

        let dividend_id = DividendId::new(
            self.date, InstrumentId::Isin(self.isin.clone()));

        let stock_quantity = util::validate_named_decimal(
            "stock quantity", self.quantity.into(),
            DecimalRestrictions::StrictlyPositive)?;

        let amount_per_stock = util::validate_named_decimal(
            "dividend amount per stock", self.amount_per_stock,
            DecimalRestrictions::StrictlyPositive)?;

        let commission = util::validate_named_decimal(
            "commission", self.commission, DecimalRestrictions::PositiveOrZero)?;

        let tax_withheld = util::validate_named_decimal(
            "withheld tax amount", self.tax_withheld,
            DecimalRestrictions::PositiveOrZero)?;

        let paid_amount = util::validate_named_decimal(
            "dividend paid amount", self.paid_amount,
            DecimalRestrictions::StrictlyPositive)?;

        let expected_paid_amount = amount_per_stock * stock_quantity - commission - tax_withheld;

        if
            util::round_with(expected_paid_amount, 2, RoundingMethod::Round) != paid_amount &&
            util::round_with(expected_paid_amount, 2, RoundingMethod::Truncate) != paid_amount
        {
            return Err!(
                "Got an unexpected dividend paid amount: {} vs {}",
                paid_amount, expected_paid_amount);
        }

        let dividend_amount = Cash::new(&self.currency, paid_amount + tax_withheld);
        let tax_withheld = Cash::new(&self.currency, tax_withheld);

        Ok((dividend_id, dividend_amount, tax_withheld))
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(name => ["dividends-report-example.xlsx"])]
    fn parse_real(name: &str) {
        let path = format!("testdata/tinkoff/{}", name);

        let is_statement = ForeignIncomeStatementReader::is_statement(&path).unwrap();
        assert!(is_statement);

        let income = ForeignIncomeStatementReader::read(&path).unwrap();
        assert!(!income.is_empty());
    }
}