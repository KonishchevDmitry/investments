use chrono::{self, Datelike, Duration};
use prettytable::{Table, Row, Cell};
use prettytable::format::{Alignment, FormatBuilder, LinePosition, LineSeparator};

use broker_statement::BrokerStatement;
use broker_statement::ib::IbStatementParser;
use core::EmptyResult;
use currency;
use currency::converter::CurrencyConverter;
use db;
use regulations;
use util;

mod parser;

pub fn generate_tax_statement(
    database: db::Connection, year: i32,
    broker_statement_path: &str, tax_statement_path: Option<&str>
) -> EmptyResult {
    let broker_statement = IbStatementParser::parse(broker_statement_path)?;
    // FIXME: HERE
    let _tax_statement = parser::TaxStatementParser::parse(tax_statement_path.unwrap())?;

    if year > chrono::Local::today().year() {
        return Err!("An attempt to generate tax statement for the future");
    }

    let tax_period_start = date!(1, 1, year);
    let tax_period_end = date!(1, 1, year + 1);

    if tax_period_start >= broker_statement.period.0 && tax_period_end <= broker_statement.period.1 {
        // Broker statement period more or equal to the tax year period
    } else if tax_period_end > broker_statement.period.0 && tax_period_start < broker_statement.period.1 {
        warn!(concat!(
            "Period of the specified broker statement ({} - {}) ",
            "doesn't fully overlap with the requested tax year ({})."),
            util::format_date(broker_statement.period.0),
            util::format_date(broker_statement.period.1 - Duration::days(1)), year);
    } else {
        return Err!(concat!(
            "Period of the specified broker statement ({} - {}) ",
            "doesn't overlap with the requested tax year ({})"),
            util::format_date(broker_statement.period.0),
            util::format_date(broker_statement.period.1 - Duration::days(1)), year);
    }

    let generator = TaxStatementGenerator {
        broker_statement: broker_statement,
        converter: CurrencyConverter::new(database),
    };

    generator.process_dividend_income().map_err(|e| format!(
        "Failed to process dividend income: {}", e))?;

    Ok(())
}

struct TaxStatementGenerator {
    broker_statement: BrokerStatement,
    converter: CurrencyConverter,
}

impl TaxStatementGenerator {
    fn process_dividend_income(&self) -> EmptyResult {
        let country = regulations::russia();
        let foreign_country = regulations::us();

        let mut table = Table::new();

        let mut total_foreign_amount = dec!(0);
        let mut total_amount = dec!(0);

        let mut total_foreign_paid_tax = dec!(0);
        let mut total_paid_tax = dec!(0);
        let mut total_tax_to_pay = dec!(0);

        let mut total_income = dec!(0);

        for dividend in &self.broker_statement.dividends {
            if dividend.amount.currency != foreign_country.currency {
                return Err!("{} dividend currency is not supported", dividend.amount.currency);
            }

            let issuer = self.broker_statement.get_instrument_name(&dividend.issuer)?;

            let foreign_amount = dividend.amount.round().amount;
            total_foreign_amount += foreign_amount;

            let currency_rate = currency::round(self.converter.currency_rate(
                dividend.date, dividend.amount.currency, country.currency)?);

            let amount = currency::round(self.converter.convert_to(
                dividend.date, dividend.amount, country.currency)?);
            total_amount += amount;

            let foreign_paid_tax = dividend.paid_tax.amount;
            total_foreign_paid_tax += foreign_paid_tax;

            let paid_tax = currency::round(self.converter.convert_to(
                dividend.date, dividend.paid_tax, country.currency)?);
            total_paid_tax += paid_tax;

            let tax_to_pay = dividend.tax_to_pay(&country, &self.converter)?;
            total_tax_to_pay += tax_to_pay;

            let income = amount - paid_tax - tax_to_pay;
            total_income += income;

            table.add_row(Row::new(vec![
                Cell::new_align(&util::format_date(dividend.date), Alignment::CENTER),
                Cell::new_align(&issuer, Alignment::LEFT),
                Cell::new_align(dividend.amount.currency, Alignment::CENTER),

                Cell::new_align(&foreign_amount.to_string(), Alignment::RIGHT),
                Cell::new_align(&currency_rate.to_string(), Alignment::RIGHT),
                Cell::new_align(&amount.to_string(), Alignment::RIGHT),

                Cell::new_align(&foreign_paid_tax.to_string(), Alignment::RIGHT),
                Cell::new_align(&paid_tax.to_string(), Alignment::RIGHT),
                Cell::new_align(&tax_to_pay.to_string(), Alignment::RIGHT),
                Cell::new_align(&income.to_string(), Alignment::RIGHT),
            ]));
        }

        if !table.is_empty() {
            table.set_titles(Row::new([
                "Дата", "Эмитент", "Валюта",
                "Сумма (USD)", "Курс руб.", "Сумма (руб)",
                "Уплачено (USD)", "Уплачено (руб)", "К доплате (руб)", "Реальный доход",
            ].iter().map(|name| Cell::new_align(*name, Alignment::CENTER)).collect()));

            table.add_row(Row::new(vec![
                Cell::new(""),
                Cell::new(""),
                Cell::new(""),

                Cell::new_align(&total_foreign_amount.to_string(), Alignment::RIGHT),
                Cell::new(""),
                Cell::new_align(&total_amount.to_string(), Alignment::RIGHT),

                Cell::new_align(&total_foreign_paid_tax.to_string(), Alignment::RIGHT),
                Cell::new_align(&total_paid_tax.to_string(), Alignment::RIGHT),
                Cell::new_align(&total_tax_to_pay.to_string(), Alignment::RIGHT),
                Cell::new_align(&total_income.to_string(), Alignment::RIGHT),
            ]));

            table.set_format(FormatBuilder::new().padding(1, 1).build());

            print_statement(
                &format!("Расчет дохода от дивидендов, полученных через {}",
                         self.broker_statement.broker.name),
                table,
            );
        }

        Ok(())
    }
}

fn print_statement(name: &str, statement: Table) {
    let mut table = Table::new();

    table.set_format(FormatBuilder::new()
        .separator(LinePosition::Title, LineSeparator::new(' ', ' ', ' ', ' '))
        .build());

    table.set_titles(Row::new(vec![
        Cell::new_align(&("\n".to_owned() + name), Alignment::CENTER),
    ]));

    table.add_row(Row::new(vec![
        Cell::new_align(&statement.to_string(), Alignment::CENTER),
    ]));

    table.printstd();
}