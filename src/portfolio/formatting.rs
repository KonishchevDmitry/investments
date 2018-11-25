use std::fmt::Write;

use ansi_term::{Style, ANSIString};
use num_traits::ToPrimitive;
use separator::Separatable;

use types::Decimal;
use util;

use super::asset_allocation::{Portfolio, AssetAllocation, Holding};

// FIXME: flat mode
pub fn print_portfolio(portfolio: &Portfolio) {
    for assets in &portfolio.assets {
        print_assets(assets, &portfolio.currency, 0);
    }

    println!();
    println!("{}: {}", colorify_name("Total value"), format_cash(&portfolio.currency, portfolio.total_value));
    println!("{}: {}", colorify_name("Free assets"), format_cash(&portfolio.currency, portfolio.free_assets));
}

fn print_assets(asset: &AssetAllocation, currency: &str, depth: usize) {
    let mut buffer = String::new();

    write!(&mut buffer, "{bullet:>indent$} {name}",
           bullet='•', indent=depth * 2 + 1, name=colorify_name(&asset.full_name())).unwrap();

    // FIXME: target value, expected value
    write!(&mut buffer, " {target_value} / {expected_weight} ({current_value})",
           target_value=format_cash(currency, asset.target_value),
           expected_weight=format_weight(asset.expected_weight),
           current_value=format_cash(currency, asset.current_value)).unwrap();

    if let Holding::Group(ref assets) = asset.holding {
        write!(&mut buffer, ":").unwrap();
        println!("{}", buffer);

        for asset in assets {
            print_assets(asset, currency, depth + 1);
        }
    } else {
        println!("{}", buffer);
    }
}

fn format_cash(currency: &str, amount: Decimal) -> String {
    let mut buffer = String::new();

    if currency == "USD" {
        write!(&mut buffer, "$").unwrap();
    }

    write!(&mut buffer, "{}", util::round_to(amount, 0).to_i64().unwrap().separated_string()).unwrap();

    match currency {
        "USD" => (),
        "RUB" => write!(&mut buffer, "₽").unwrap(),
        _ => write!(&mut buffer, " {}", currency).unwrap(),
    };

    buffer
}

fn format_weight(weight: Decimal) -> String {
    format!("{}%", util::round_to(weight * dec!(100), 1))
}

fn colorify_name(name: &str) -> ANSIString {
    Style::new().bold().paint(name)
}