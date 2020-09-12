use std::fmt::Write;

use ansi_term::{Style, Color, ANSIString};
use num_traits::Zero;

use crate::currency::Cash;
use crate::types::Decimal;
use crate::util;

use super::asset_allocation::{Portfolio, AssetAllocation, Holding};

pub fn print_portfolio(portfolio: Portfolio, flat: bool) {
    let mut assets = portfolio.assets;
    if flat {
        assets = flatify(assets, dec!(1));
    }

    print_assets(assets, portfolio.total_value - portfolio.min_cash_assets, &portfolio.currency, 0);

    println!("\n{} {}", colorify_title("Total value:"),
             format_cash(&portfolio.currency, portfolio.total_value));

    print!("{} {}", colorify_title("Cash assets:"),
           format_cash(&portfolio.currency, portfolio.current_cash_assets));
    if portfolio.target_cash_assets != portfolio.current_cash_assets {
        print!(" -> {}", format_cash(&portfolio.currency, portfolio.target_cash_assets));
    }
    println!();

    if !portfolio.commissions.is_zero() {
        println!("{} {}", colorify_title("Commissions:"),
                 colorify_commission(&format_cash(&portfolio.currency, portfolio.commissions)));
    }
}

fn flatify(assets: Vec<AssetAllocation>, expected_weight: Decimal) -> Vec<AssetAllocation> {
    let mut flat_assets = Vec::new();

    for mut asset in assets {
        asset.expected_weight *= expected_weight;

        match asset.holding {
            Holding::Stock(_) => {
                flat_assets.push(asset);
            },
            Holding::Group(holdings) => {
                flat_assets.extend(flatify(holdings, asset.expected_weight));
            },
        };
    }

    flat_assets
}

fn print_assets(mut assets: Vec<AssetAllocation>, expected_total_value: Decimal, currency: &str, depth: usize) {
    assets.sort_by_key(|asset: &AssetAllocation| -asset.target_value);

    for asset in assets {
        print_asset(asset, expected_total_value, currency, depth);
    }
}

fn print_asset(asset: AssetAllocation, expected_total_value: Decimal, currency: &str, depth: usize) {
    let expected_value = expected_total_value * asset.expected_weight;

    let mut buffer = String::new();

    write!(&mut buffer, "{bullet:>indent$} {name}",
           bullet='•', indent=depth * 2 + 1, name= colorify_title(&asset.full_name())).unwrap();

    if asset.buy_blocked {
        write!(&mut buffer, " {}", colorify_restriction("[buy blocked]")).unwrap();
    }
    if asset.sell_blocked {
        write!(&mut buffer, " {}", colorify_restriction("[sell blocked]")).unwrap();
    }

    write!(&mut buffer, " -").unwrap();

    if let Holding::Stock(ref holding) = asset.holding {
        write!(&mut buffer, " {}", format_shares(holding.current_shares, false)).unwrap();
    }

    write!(&mut buffer, " {current_weight} ({current_value})",
           current_weight=format_weight(get_weight(asset.current_value, expected_total_value)),
           current_value=format_cash(currency, asset.current_value)).unwrap();

    if asset.target_value != asset.current_value {
        if let Holding::Stock(ref holding) = asset.holding {
            let colorify_func = if holding.target_shares > holding.current_shares {
                colorify_buy
            } else {
                colorify_sell
            };

            let shares_change = holding.target_shares - holding.current_shares;
            let value_change = asset.target_value - asset.current_value;

            let changes = format!(
                "{shares_change} ({value_change})",
                shares_change=format_shares(shares_change, true),
                value_change=format_cash(currency, value_change.abs()));

            write!(&mut buffer, " {}", colorify_func(&changes)).unwrap();
        }

        write!(&mut buffer, " → {target_weight} ({target_value})",
               target_weight=format_weight(get_weight(asset.target_value, expected_total_value)),
               target_value=format_cash(currency, asset.target_value)).unwrap();
    }

    write!(&mut buffer, " / {expected_weight} ({expected_value})",
           expected_weight=format_weight(asset.expected_weight),
           expected_value=format_cash(currency, expected_value)).unwrap();

    if let Holding::Group(holdings) = asset.holding {
        println!("{}:", buffer);
        print_assets(holdings, expected_value, currency, depth + 1);
    } else {
        println!("{}", buffer);
    }
}

fn format_cash(currency: &str, amount: Decimal) -> String {
    Cash::new(currency, amount).format_rounded()
}

fn format_shares(shares: Decimal, with_sign: bool) -> String {
    let shares = shares.normalize();
    let symbol = 's';

    if with_sign {
        format!("{:+}{}", shares, symbol)
    } else {
        format!("{}{}", shares, symbol)
    }
}

fn get_weight(asset_value: Decimal, expected_total_value: Decimal) -> Decimal {
    if expected_total_value.is_zero() {
        Decimal::max_value()
    } else {
        asset_value / expected_total_value
    }
}

fn format_weight(weight: Decimal) -> String {
    if weight == Decimal::max_value() {
        "∞".to_owned()
    } else {
        format!("{}%", util::round(weight * dec!(100), 1))
    }
}

fn colorify_title(name: &str) -> ANSIString {
    Style::new().bold().paint(name)
}

fn colorify_restriction(message: &str) -> ANSIString {
    Color::Blue.paint(message)
}

fn colorify_buy(message: &str) -> ANSIString {
    Color::Green.paint(message)
}

fn colorify_sell(message: &str) -> ANSIString {
    Color::Red.paint(message)
}

fn colorify_commission(message: &str) -> ANSIString {
    Color::Yellow.paint(message)
}