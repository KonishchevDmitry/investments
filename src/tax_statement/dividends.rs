use chrono::Datelike;
use num_traits::Zero;

use crate::broker_statement::BrokerStatement;
use crate::config::PortfolioConfig;
use crate::core::EmptyResult;
use crate::currency::{self, Cash, MultiCurrencyCashAccount};
use crate::currency::converter::CurrencyConverter;
use crate::formatting::table::{Table, Row, Cell, Alignment, print_table};

use super::statement::TaxStatement;

pub fn process_income(
    portfolio: &PortfolioConfig, broker_statement: &BrokerStatement, year: Option<i32>,
    mut tax_statement: Option<&mut TaxStatement>, converter: &CurrencyConverter,
) -> EmptyResult {
    let mut table = Table::new();
    let country = portfolio.get_tax_country();

    let mut total_foreign_amount = MultiCurrencyCashAccount::new();
    let mut total_amount = dec!(0);

    let mut total_foreign_paid_tax = MultiCurrencyCashAccount::new();
    let mut total_paid_tax = dec!(0);
    let mut total_tax_deduction = dec!(0);
    let mut total_tax_to_pay = dec!(0);

    let mut total_income = dec!(0);

    for dividend in &broker_statement.dividends {
        if let Some(year) = year {
            if dividend.date.year() != year {
                continue;
            }
        }

        let issuer = broker_statement.get_instrument_name(&dividend.issuer)?;

        let foreign_amount = dividend.amount.round();
        total_foreign_amount.deposit(foreign_amount);

        let precise_currency_rate = converter.precise_currency_rate(
            dividend.date, foreign_amount.currency, country.currency)?;

        let amount = currency::round(converter.convert_to(
            dividend.date, dividend.amount, country.currency)?);
        total_amount += amount;

        let tax = dividend.tax(&country, converter)?;

        let foreign_paid_tax = dividend.paid_tax;
        total_foreign_paid_tax.deposit(foreign_paid_tax);

        let paid_tax = currency::round(converter.convert_to(
            dividend.date, dividend.paid_tax, country.currency)?);
        total_paid_tax += paid_tax;

        let tax_to_pay = dividend.tax_to_pay(&country, converter)?;
        total_tax_to_pay += tax_to_pay;

        let tax_deduction = country.round_tax(paid_tax);
        if !tax_to_pay.is_zero() {
            assert_eq!(tax_deduction, tax - tax_to_pay);
        }
        total_tax_deduction += tax_deduction;

        let income = amount - paid_tax - tax_to_pay;
        total_income += income;

        table.add_row(Row::new(&[
            Cell::new_date(dividend.date),
            Cell::new_align(&issuer, Alignment::LEFT),
            Cell::new_align(foreign_amount.currency, Alignment::CENTER),

            Cell::new_cash(foreign_amount),
            Cell::new_decimal(precise_currency_rate),
            Cell::new_cash(Cash::new(country.currency, amount)),

            Cell::new_cash(Cash::new(country.currency, tax)),
            Cell::new_cash(foreign_paid_tax),
            Cell::new_cash(Cash::new(country.currency, paid_tax)),
            Cell::new_cash(Cash::new(country.currency, tax_deduction)),
            Cell::new_cash(Cash::new(country.currency, tax_to_pay)),
            Cell::new_cash(Cash::new(country.currency, income)),
        ]));

        if let Some(ref mut tax_statement) = tax_statement {
            let description = format!("{}: Дивиденд от {}", broker_statement.broker.name, issuer);

            if foreign_paid_tax.currency != foreign_amount.currency {
                return Err!(
                        "{}: Tax currency is different from dividend currency: {} vs {}",
                        dividend.description(), foreign_paid_tax.currency, foreign_amount.currency);
            }

            tax_statement.add_dividend_income(
                &description, dividend.date, foreign_amount.currency, precise_currency_rate,
                foreign_amount.amount, foreign_paid_tax.amount, amount, paid_tax
            ).map_err(|e| format!(
                "Unable to add {} to the tax statement: {}", dividend.description(), e
            ))?;
        }
    }

    if !table.is_empty() {
        table.add_row(Row::new(&[
            Cell::new_empty(),
            Cell::new_empty(),
            Cell::new_empty(),

            Cell::new_multi_currency_cash(total_foreign_amount),
            Cell::new_empty(),
            Cell::new_cash(Cash::new(country.currency, total_amount)),

            Cell::new_empty(),
            Cell::new_multi_currency_cash(total_foreign_paid_tax),
            Cell::new_cash(Cash::new(country.currency, total_paid_tax)),
            Cell::new_cash(Cash::new(country.currency, total_tax_deduction)),
            Cell::new_cash(Cash::new(country.currency, total_tax_to_pay)),
            Cell::new_cash(Cash::new(country.currency, total_income)),
        ]));

        print_table(
            &format!("Расчет дохода от дивидендов, полученных через {}",
                     broker_statement.broker.name),
            &[
                "Дата", "Эмитент", "Валюта",
                "Сумма", "Курс руб.", "Сумма (руб)",
                "Налог", "Уплачено", "Уплачено (руб)", "К зачету", "К доплате",
                "Реальный доход",
            ],
            table,
        );
    }

    Ok(())
}