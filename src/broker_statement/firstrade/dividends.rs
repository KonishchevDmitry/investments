use chrono::Datelike;
use lazy_static::lazy_static;
use log::warn;
use regex::Regex;

use crate::core::EmptyResult;
use crate::currency::Cash;
use crate::localities;
use crate::taxes::IncomeType;
use crate::types::Date;
use crate::util;

use super::StatementParser;

pub fn parse_dividend(
    parser: &mut StatementParser, date: Date, issuer: &str, income: Cash, description: &str,
) -> EmptyResult {
    let currency = income.currency;
    let income = income.amount;

    if parser.reader.warn_on_missing_dividend_details {
        warn!(concat!(
            "Firstrade statements don't provide information about real dividend amount, so it ",
            "will be deduced from paid amount and expected tax rate.",
        ));
        parser.reader.warn_on_missing_dividend_details = false;
    }

    let stripped_description = util::fold_spaces(description.trim());
    if !stripped_description.ends_with(" NON-QUALIFIED DIVIDEND NON-RES TAX WITHHELD") &&
        !stripped_description.ends_with(" IN LIEU OF DIVIDEND NON-RES TAX WITHHELD") {
        return Err!("Unexpected dividend description: {:?}", description);
    }

    let foreign_country = localities::us();
    let amount = foreign_country.deduce_income(IncomeType::Dividends, date.year(), income);
    let paid_tax = amount - income;
    debug_assert_eq!(paid_tax, foreign_country.tax_to_pay(IncomeType::Dividends, date.year(), amount, None));

    parser.statement.dividend_accruals(date, issuer, true)
        .add(date, Cash::new(currency, amount));

    parser.statement.tax_accruals(date, issuer, false)
        .add(date, Cash::new(currency, paid_tax));

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
        case("ISHARES TR                     CORE INTL AGGREGATE BD ETF     Rev NRA W/H AS/OF 10/07/20 ROC", date!( 7, 10, 2020)),
        case("VANGUARD                       TOTAL BOND MARKET ETF          Rev NRA W/H AS/OF 12/29/20 LCG", date!(29, 12, 2020)),
    )]
    fn tax_reversal_parsing(description: &str, date: Date) {
        assert_eq!(parse_tax_reversal_description(description), Some(date));
    }
}