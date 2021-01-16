use static_table_derive::StaticTable;

use crate::broker_statement::BrokerStatement;
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
) {
    let mut table = Table::new();
    let mut withheld_tax = MultiCurrencyCashAccount::new();

    for withholding in &broker_statement.tax_agent_withholdings {
        if let Some(year) = year {
            if withholding.year != year {
                continue;
            }
        }

        withheld_tax.deposit(withholding.amount);
    }

    if withheld_tax.is_empty() {
        withheld_tax.deposit(Cash::new(calculated_tax.currency, dec!(0)));
    }

    table.add_row(Row {calculated_tax, withheld_tax});
    table.print(&format!("Налог, удержанный {}", broker_statement.broker.name));
}