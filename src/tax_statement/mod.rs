use chrono::Datelike;
use log::warn;
use prettytable::{Table, Row, Cell};
use prettytable::format::Alignment;

use crate::broker_statement::BrokerStatement;
use crate::config::Config;
use crate::core::EmptyResult;
use crate::currency::{self, Cash, MultiCurrencyCashAccount};
use crate::currency::converter::CurrencyConverter;
use crate::db;
use crate::formatting;
use crate::localities;
use crate::util;

use self::statement::TaxStatement;

mod statement;

pub fn generate_tax_statement(
    config: &Config, portfolio_name: &str, year: i32, tax_statement_path: Option<&str>
) -> EmptyResult {
    if year > util::today().year() {
        return Err!("An attempt to generate tax statement for the future");
    }

    let portfolio = config.get_portfolio(portfolio_name)?;
    let broker_statement = BrokerStatement::read(config, portfolio.broker, &portfolio.statements)?;

    let tax_period_start = date!(1, 1, year);
    let tax_period_end = date!(1, 1, year + 1);

    if tax_period_start >= broker_statement.period.0 && tax_period_end <= broker_statement.period.1 {
        // Broker statement period more or equal to the tax year period
    } else if tax_period_end > broker_statement.period.0 && tax_period_start < broker_statement.period.1 {
        warn!(concat!(
            "Period of the specified broker statement ({}) ",
            "doesn't fully overlap with the requested tax year ({})."),
            formatting::format_period(broker_statement.period.0, broker_statement.period.1), year);
    } else {
        return Err!(concat!(
            "Period of the specified broker statement ({}) ",
            "doesn't overlap with the requested tax year ({})"),
            formatting::format_period(broker_statement.period.0, broker_statement.period.1), year);
    }

    let mut tax_statement = match tax_statement_path {
        Some(path) => {
            let statement = TaxStatement::read(path)?;

            if statement.year != year {
                return Err!("Tax statement year ({}) doesn't match the requested year {}",
                    statement.year, year);
            }

            Some(statement)
        },
        None => None,
    };

    {
        let database = db::connect(&config.db_path)?;

        let mut generator = TaxStatementGenerator {
            broker_statement: broker_statement,
            year: year,
            tax_statement: tax_statement.as_mut(),
            converter: CurrencyConverter::new(database, true),
        };

        // FIXME: Stock selling support
        // FIXME: Free cash interest support
        generator.process_dividend_income().map_err(|e| format!(
            "Failed to process dividend income: {}", e))?;
    }

    if let Some(ref tax_statement) = tax_statement {
        tax_statement.save()?;
    }

    Ok(())
}

struct TaxStatementGenerator<'a> {
    broker_statement: BrokerStatement,
    year: i32,
    tax_statement: Option<&'a mut TaxStatement>,
    converter: CurrencyConverter,
}

impl<'a> TaxStatementGenerator<'a> {
    fn process_dividend_income(&mut self) -> EmptyResult {
        let country = localities::russia();

        let mut table = Table::new();

        let mut total_foreign_amount = MultiCurrencyCashAccount::new();
        let mut total_amount = dec!(0);

        let mut total_foreign_paid_tax = MultiCurrencyCashAccount::new();
        let mut total_paid_tax = dec!(0);
        let mut total_tax_to_pay = dec!(0);

        let mut total_income = dec!(0);

        for dividend in &self.broker_statement.dividends {
            if dividend.date.year() != self.year {
                continue;
            }

            let issuer = self.broker_statement.get_instrument_name(&dividend.issuer)?;

            let foreign_amount = dividend.amount.round();
            total_foreign_amount.deposit(foreign_amount);

            // Don't round currency rate. CBR provides currency rates with high precision like
            // 56.3438 and tax statement uses currency rate value for 100 units like 5634.38.
            let precise_currency_rate = self.converter.currency_rate(
                dividend.date, foreign_amount.currency, country.currency)?;

            let amount = currency::round(self.converter.convert_to(
                dividend.date, dividend.amount, country.currency)?);
            total_amount += amount;

            let foreign_paid_tax = dividend.paid_tax;
            total_foreign_paid_tax.deposit(foreign_paid_tax);

            let paid_tax = currency::round(self.converter.convert_to(
                dividend.date, dividend.paid_tax, country.currency)?);
            total_paid_tax += paid_tax;

            let tax_to_pay = dividend.tax_to_pay(&country, &self.converter)?;
            total_tax_to_pay += tax_to_pay;

            let income = amount - paid_tax - tax_to_pay;
            total_income += income;

            table.add_row(Row::new(vec![
                formatting::date_cell(dividend.date),
                Cell::new_align(&issuer, Alignment::LEFT),
                Cell::new_align(foreign_amount.currency, Alignment::CENTER),

                formatting::cash_cell(foreign_amount),
                formatting::decimal_cell(precise_currency_rate),
                formatting::cash_cell(Cash::new(country.currency, amount)),

                formatting::cash_cell(foreign_paid_tax),
                formatting::cash_cell(Cash::new(country.currency, paid_tax)),
                formatting::cash_cell(Cash::new(country.currency, tax_to_pay)),
                formatting::cash_cell(Cash::new(country.currency, income)),
            ]));

            if let Some(ref mut tax_statement) = self.tax_statement {
                let description = format!(
                    "{}: Дивиденд от {}", self.broker_statement.broker.name, issuer);

                if foreign_paid_tax.currency != foreign_amount.currency {
                    return Err!(
                        "{}: Tax currency is different from dividend currency: {} vs {}",
                        dividend.description(), foreign_paid_tax.currency, foreign_amount.currency);
                }

                tax_statement.add_dividend(
                    &description, dividend.date, foreign_amount.currency, precise_currency_rate,
                    foreign_amount.amount, foreign_paid_tax.amount, amount, paid_tax
                ).map_err(|e| format!(
                    "Unable to add {} to the tax statement: {}", dividend.description(), e
                ))?;
            }
        }

        if !table.is_empty() {
            table.add_row(Row::new(vec![
                formatting::empty_cell(),
                formatting::empty_cell(),
                formatting::empty_cell(),

                formatting::multi_currency_cash_cell(total_foreign_amount),
                formatting::empty_cell(),
                formatting::cash_cell(Cash::new(country.currency, total_amount)),

                formatting::multi_currency_cash_cell(total_foreign_paid_tax),
                formatting::cash_cell(Cash::new(country.currency, total_paid_tax)),
                formatting::cash_cell(Cash::new(country.currency, total_tax_to_pay)),
                formatting::cash_cell(Cash::new(country.currency, total_income)),
            ]));

            formatting::print_statement(
                &format!("Расчет дохода от дивидендов, полученных через {}",
                         self.broker_statement.broker.name),
                &[
                    "Дата", "Эмитент", "Валюта",
                    "Сумма", "Курс руб.", "Сумма (руб)",
                    "Уплачено", "Уплачено (руб)", "К доплате", "Реальный доход",
                ],
                table,
            );
        }

        Ok(())
    }
}