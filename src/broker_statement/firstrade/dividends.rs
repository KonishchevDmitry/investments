use lazy_static::lazy_static;
use log::warn;
use regex::Regex;

use crate::core::EmptyResult;
use crate::currency::Cash;
use crate::instruments::InstrumentId;
use crate::localities::{self, Jurisdiction};
use crate::taxes::{FixedTaxRate, IncomeType, TaxRate};
use crate::types::Date;
use crate::util;

use super::StatementParser;

pub fn parse_dividend(
    parser: &mut StatementParser, date: Date, issuer: &str, income: Cash, description: &str,
) -> EmptyResult {
    if parser.reader.warn_on_missing_dividend_details {
        warn!(concat!(
            "Firstrade statements don't provide information about real dividend amount, so it ",
            "will be deduced from received amount and expected tax rate.",
        ));
        parser.reader.warn_on_missing_dividend_details = false;
    }

    let mut non_res_tax_withheld = false;
    let mut stripped_description = util::fold_spaces(description).to_string();

    #[allow(clippy::assigning_clones)]
    if let Some(stripped) = stripped_description.strip_suffix(" NON-RES TAX WITHHELD") {
        stripped_description = stripped.to_owned();
        non_res_tax_withheld = true;
    }

    if !stripped_description.ends_with(" NON-QUALIFIED DIVIDEND") &&
        !stripped_description.ends_with(" IN LIEU OF DIVIDEND") {
        return Err!("Unexpected dividend description: {:?}", description);
    }

    let us = Jurisdiction::Usa.traits();
    let mut tax_rate = FixedTaxRate::new(localities::us_dividend_tax_rate(date), us.tax_precision);

    if income.currency != us.currency {
        return Err!("Got a dividend from {} in an unexpected currency: {}", issuer, income.currency)
    }

    let (amount, paid_tax) = if non_res_tax_withheld {
        let amount = localities::deduce_us_dividend_amount(date, income);
        let paid_tax = amount - income;
        debug_assert_eq!(paid_tax.amount, tax_rate.tax(IncomeType::Dividends, amount.amount));
        (amount, paid_tax)
    } else {
        let amount = income;
        let paid_tax = tax_rate.tax(IncomeType::Dividends, amount.amount);
        (amount, Cash::new(amount.currency, paid_tax))
    };

    let issuer_id = InstrumentId::Symbol(issuer.to_owned());
    parser.statement.dividend_accruals(date, issuer_id.clone(), true).add(date, amount);
    parser.statement.tax_accruals(date, issuer_id, false).add(date, paid_tax);

    Ok(())
}

pub fn parse_tax_reversal_description(description: &str) -> Option<Date> {
    lazy_static! {
        static ref REVERSAL_REGEX: Regex = Regex::new(
            r" Rev NRA W/H AS/OF (?P<date>\d{2}/\d{2}/\d{2}) ").unwrap();
    }

    REVERSAL_REGEX.captures(description).and_then(|captures| {
        let date = captures.name("date").unwrap().as_str();
        Date::parse_from_str(date, "%m/%d/%y").ok()
    })
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use super::*;

    #[rstest(description, date,
        case("ISHARES TR                     CORE INTL AGGREGATE BD ETF     Rev NRA W/H AS/OF 10/07/20 ROC", date!(2020, 10,  7)),
        case("VANGUARD                       TOTAL BOND MARKET ETF          Rev NRA W/H AS/OF 12/29/20 LCG", date!(2020, 12, 29)),
    )]
    fn tax_reversal_parsing(description: &str, date: Date) {
        assert_eq!(parse_tax_reversal_description(description), Some(date));
    }
}