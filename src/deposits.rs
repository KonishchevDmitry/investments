use static_table_derive::StaticTable;

use crate::analyse::deposit_emulator::{DepositEmulator, Transaction};
use crate::config::DepositConfig;
use crate::currency::{Cash, MultiCurrencyCashAccount};
use crate::localities;
use crate::types::{Date, Decimal};

// FIXME: Regression tests, cron mode, coloring
pub fn list(mut deposits: Vec<DepositConfig>, today: Date) {
    let mut deposits: Vec<DepositConfig> = deposits.drain(..).filter(|deposit| {
        deposit.open_date <= today
    }).collect();
    deposits.sort_by_key(|deposit| deposit.close_date);

    if !deposits.is_empty() {
        print(deposits, today);
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
    #[column(name="Current amount")]
    current_amount: Cash,
}

fn print(deposits: Vec<DepositConfig>, today: Date) {
    let country = localities::russia();

    let mut table = Table::new();
    let mut total_amount = MultiCurrencyCashAccount::new();
    let mut total_current_amount = MultiCurrencyCashAccount::new();

    for deposit in deposits {
        let currency = deposit.currency.as_ref().map_or(country.currency, String::as_str);

        let mut contributions = deposit.contributions;
        contributions.insert(0, (deposit.open_date, deposit.amount));

        let transactions: Vec<_> = contributions.iter().filter_map(|&(date, amount)| {
            if date <= today {
                Some(Transaction::new(date, amount))
            } else {
                None
            }
        }).collect();

        let amount = transactions.iter().map(|transaction| transaction.amount).sum();
        let amount = Cash::new(currency, amount);
        total_amount.deposit(amount);

        let end_date = if today < deposit.close_date {
            today
        } else {
            deposit.close_date
        };

        let current_amount = DepositEmulator::new(deposit.open_date, end_date, deposit.interest)
            .with_monthly_capitalization(deposit.capitalization)
            .emulate(&transactions);
        let current_amount = Cash::new(currency, current_amount).round();
        total_current_amount.deposit(current_amount);

        table.add_row(Row {
            open_date: deposit.open_date,
            close_date: deposit.close_date,
            name: deposit.name,
            amount: amount,
            interest: deposit.interest.normalize(),
            current_amount: current_amount,
        });
    }

    let mut totals = table.add_empty_row();
    totals.set_amount(total_amount);
    totals.set_current_amount(total_current_amount);

    table.print("Open deposits");
}