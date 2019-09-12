use static_table_derive::StaticTable;

use crate::config::DepositConfig;
use crate::currency::{Cash, MultiCurrencyCashAccount};
use crate::localities;
use crate::types::{Date, Decimal};
use crate::util;

// FIXME: Regression tests
pub fn list(mut deposits: Vec<DepositConfig>) {
    let today = util::today();

    let mut deposits: Vec<DepositConfig> = deposits.drain(..).filter(|deposit| {
        deposit.open_date <= today
    }).collect();
    deposits.sort_by_key(|deposit| deposit.close_date);

    if !deposits.is_empty() {
        print(deposits);
    }
}

#[derive(StaticTable)]
struct Row {
    #[column(name="Open date")]
    open_date: Date,
    #[column(name="Close date")]
    close_date: Date,
    #[column(name="Name")]
    name: String,
    #[column(name="Amount")]
    amount: Cash,
    #[column(name="Interest")]
    interest: Decimal,
}

fn print(deposits: Vec<DepositConfig>) {
    let mut table = Table::new();
    let country = localities::russia();
    let mut total_amount = MultiCurrencyCashAccount::new();

    for deposit in deposits {
        // FIXME
//        pub capitalization: Option<u32>,
//        pub contributions: Vec<(Date, Decimal)>,
        let currency = deposit.currency.as_ref().map_or(country.currency, String::as_str);

        let amount = Cash::new(currency, deposit.amount);
        total_amount.deposit(amount);

        table.add_row(Row {
            open_date: deposit.open_date,
            close_date: deposit.close_date,
            name: deposit.name,
            amount: amount,
            interest: deposit.interest.normalize(),
        });
    }

    let mut totals = table.add_empty_row();
    totals.set_amount(total_amount);

    table.print("Open deposits");
}