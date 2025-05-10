pub mod config;

use chrono::Duration;
use static_table_derive::StaticTable;

use crate::analysis::deposit::{DepositEmulator, Transaction};
use crate::currency::{Cash, MultiCurrencyCashAccount};
use crate::formatting::{self, table::Style};
use crate::localities::Country;
use crate::types::{Date, Decimal};

use self::config::DepositConfig;

pub fn list(country: &Country, deposits: Vec<DepositConfig>, today: Date, cron_mode: bool, notify_days: Option<u32>) {
    let mut deposits: Vec<DepositConfig> = deposits.into_iter().filter(|deposit| {
        deposit.open_date <= today
    }).collect();

    if deposits.is_empty() {
        return
    }
    deposits.sort_by_key(|deposit| deposit.close_date);

    if cron_mode {
        print_cron_mode(country, deposits, today, notify_days)
    } else {
        print(country, deposits, today);
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

fn print(country: &Country, deposits: Vec<DepositConfig>, today: Date) {
    let mut table = Table::new();
    let mut total_amount = MultiCurrencyCashAccount::new();
    let mut total_current_amount = MultiCurrencyCashAccount::new();

    for deposit in deposits {
        let (amount, current_amount) = calculate_amounts(country, &deposit, today);
        total_amount.deposit(amount);
        total_current_amount.deposit(current_amount);

        let mut row = table.add_row(Row {
            open_date: deposit.open_date,
            close_date: deposit.close_date,
            name: deposit.name,
            amount: amount,
            interest: deposit.interest.normalize(),
            current_amount: current_amount,
        });

        if deposit.close_date <= today {
            let style = Style::new().dimmed();
            for cell in &mut row {
                cell.style(style);
            }
        }
    }

    let mut totals = table.add_empty_row();
    totals.set_amount(total_amount);
    totals.set_current_amount(total_current_amount);

    table.print("Open deposits");
}

fn print_cron_mode(country: &Country, deposits: Vec<DepositConfig>, today: Date, notify_days: Option<u32>) {
    let mut expiring_deposits = Vec::new();
    let mut closed_deposits = Vec::new();

    for deposit in deposits {
        if deposit.close_date <= today {
            closed_deposits.push(deposit);
        } else if let Some(notify_days) = notify_days {
            if today + Duration::days(i64::from(notify_days)) == deposit.close_date {
                expiring_deposits.push(deposit);
            }
        }
    }

    if !expiring_deposits.is_empty() {
        println!("The following deposits are about to close:");
        for deposit in &expiring_deposits {
            print_closed_deposit(country, deposit);
        }
    }

    if !closed_deposits.is_empty() {
        if !expiring_deposits.is_empty() {
            println!();
        }

        println!("The following deposits are closed:");
        for deposit in &closed_deposits {
            print_closed_deposit(country, deposit);
        }
    }
}

fn print_closed_deposit(country: &Country, deposit: &DepositConfig) {
    let (amount, close_amount) = calculate_amounts(country, deposit, deposit.close_date);
    println!(
        "• {date} {name}: {amount} -> {close_amount}",
        date=formatting::format_date(deposit.close_date), name=deposit.name, amount=amount,
        close_amount=close_amount);
}

fn calculate_amounts(country: &Country, deposit: &DepositConfig, today: Date) -> (Cash, Cash) {
    let currency = deposit.currency.as_ref().map_or(country.currency, String::as_str);

    let mut contributions = vec![(deposit.open_date, deposit.amount)];
    contributions.extend(&deposit.contributions);

    let transactions: Vec<_> = contributions.iter().filter_map(|&(date, amount)| {
        if date <= today {
            Some(Transaction::new(date, amount))
        } else {
            None
        }
    }).collect();

    let amount = transactions.iter().map(|transaction| transaction.amount).sum();
    let amount = Cash::new(currency, amount);

    let end_date = if today <= deposit.close_date {
        today
    } else {
        deposit.close_date
    };

    let current_amount = DepositEmulator::new(deposit.open_date, end_date, deposit.interest)
        .with_monthly_capitalization(deposit.capitalization)
        .emulate(&transactions);
    let current_amount = Cash::new(currency, current_amount).round();

    (amount, current_amount)
}