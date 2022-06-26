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
    broker_statement: &BrokerStatement, year: Option<i32>, calculated_tax: Cash,
) -> EmptyResult {
    let mut table = Table::new();
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
        withheld_tax.deposit(Cash::zero(calculated_tax.currency));
    }

    table.add_row(Row {calculated_tax, withheld_tax});
    table.print(&format!("Налог, удержанный {}", broker_statement.broker.name));

    Ok(())
}