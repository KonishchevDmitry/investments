use static_table_derive::StaticTable;

use crate::broker_statement::BrokerStatement;
use crate::core::EmptyResult;
use crate::currency::{Cash, MultiCurrencyCashAccount};

#[derive(StaticTable)]
struct Row {
    #[column(name="Посчитанный")]
    calculated_tax: Cash,
    #[column(name="Удержанный брокером")]
    withheld_tax: MultiCurrencyCashAccount,
}

pub fn process_tax_agent_withholdings(
    broker_statement: &BrokerStatement, year: Option<i32>, has_income: bool, calculated_tax: Cash,
) -> EmptyResult {
    let mut withheld_tax = MultiCurrencyCashAccount::new();

    for (withholding_year, withholding) in broker_statement.tax_agent_withholdings.calculate()? {
        if let Some(year) = year {
            if withholding_year != year {
                continue;
            }
        }

        withheld_tax.deposit(withholding);
    }

    if withheld_tax.is_empty() {
        if !has_income {
            assert!(calculated_tax.is_zero());
            return Ok(());
        }
        withheld_tax.deposit(Cash::zero(calculated_tax.currency));
    }

    let mut table = Table::new();
    table.add_row(Row {calculated_tax, withheld_tax});
    table.print(&format!("Налог, удержанный {}", broker_statement.broker.name));

    Ok(())
}