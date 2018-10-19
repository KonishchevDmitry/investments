use chrono::{self, Datelike, Duration};
use prettytable::{Table, Row, Cell};
use prettytable::format::{Alignment, FormatBuilder};

use broker_statement::BrokerStatement;
use broker_statement::ib::IbStatementParser;
use core::EmptyResult;
use currency::CashAssets;
use currency::converter::CurrencyConverter;
use db;
use types::Date;
use util;

pub fn generate_tax_statement(
    database: db::Connection, year: i32,
    broker_statement_path: &str, tax_statement_path: Option<&str>
) -> EmptyResult {
    let broker_statement = IbStatementParser::parse(broker_statement_path)?;

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
        let foreign_currency = "USD";
        let local_currency = "RUB";

        let mut table = Table::new();

        table.set_format(FormatBuilder::new()
            .padding(1, 1)
            .build());

        table.set_titles(Row::new([
            "Дата", "Эмитент", "Валюта",
            "Сумма (USD)", "Курс руб.", "Сумма (руб)",
            "Уплачено (USD)", "Уплачено (руб)", "К доплате (руб)", "Реальный доход",
        ].iter().map(|name| Cell::new_align(*name, Alignment::CENTER)).collect()));

        for dividend in &self.broker_statement.dividends {
            if dividend.amount.currency != foreign_currency {
                return Err!("{} dividend currency is not supported", dividend.amount.currency);
            }

            let issuer = self.broker_statement.get_instrument_name(&dividend.issuer)?;

            table.add_row(Row::new(vec![
                Cell::new_align(&util::format_date(dividend.date), Alignment::CENTER),
                Cell::new_align(&issuer, Alignment::LEFT),
                Cell::new_align(dividend.amount.currency, Alignment::CENTER),

                Cell::new_align(&dividend.amount.amount.to_string(), Alignment::RIGHT)
            ]));
        }

        if !table.is_empty() {
            let mut wrapper = Table::new();
            wrapper.set_format(FormatBuilder::new().indent(1).build());
            wrapper.add_row(Row::new(vec![Cell::new_align(
                &format!("Расчет дохода от дивидендов, полученных через {}",
                        self.broker_statement.broker.name),
                Alignment::CENTER,
            )]));
            wrapper.add_row(Row::new(vec![Cell::new_align(&table.to_string(), Alignment::CENTER)]));
            wrapper.printstd();
        }

        Ok(())
    }
}

/*
    def print(self):

        table.align["Эмитент"] = "l"
        for column_name in currency_columns:
            table.align[column_name] = "r"

        total_value = Decimal()
        total_value_in_local_currency = Decimal()

        total_paid_taxes = Decimal()
        total_paid_taxes_in_local_currency = Decimal()

        total_to_pay = Decimal()
        total_real_income = Decimal()

        for dividend in self.dividends:
            expected_taxes_in_local_currency = _round_currency(dividend.value_in_local_currency * Decimal("0.13"))
            to_pay = max(Decimal(), expected_taxes_in_local_currency - dividend.paid_taxes_in_local_currency)
            real_income = dividend.value_in_local_currency - dividend.paid_taxes_in_local_currency - to_pay

            total_value += dividend.value
            total_value_in_local_currency += dividend.value_in_local_currency

            total_paid_taxes += dividend.paid_taxes
            total_paid_taxes_in_local_currency += dividend.paid_taxes_in_local_currency

            total_to_pay += to_pay
            total_real_income += real_income

            table.add_row((
                dividend.date.strftime("%d.%m.%Y"), dividend.issuer, "USD",
                dividend.value, dividend.local_currency_rate, dividend.value_in_local_currency,
                dividend.paid_taxes, dividend.paid_taxes_in_local_currency, to_pay, real_income,
            ))

        table.add_row((
            "", "", "",
            total_value, "", total_value_in_local_currency,
            total_paid_taxes, total_paid_taxes_in_local_currency, total_to_pay, total_real_income,
        ))

        print(table.get_string())
*/