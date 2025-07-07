use crate::core::GenericResult;
use crate::currency::{Cash, MultiCurrencyCashAccount};
use crate::currency::converter::CurrencyConverter;
use crate::time::Date;
use crate::types::Decimal;
use crate::util;

pub fn convert_price(
    price: Cash, quantity: Decimal, currency: &str, converter: &CurrencyConverter,
) -> GenericResult<Cash> {
    let volume = price * quantity;

    let volume_precision = util::decimal_precision(volume.amount) + 2;
    let converted_volume = util::round(
        converter.real_time_convert_to(volume, currency)?,
        volume_precision);

    Ok(calculate_price(quantity, Cash::new(currency, converted_volume)).map_err(|e| format!(
        "Unable to convert {quantity} x {price} price to {currency}: {e}"
    ))?)
}

pub fn calculate_price(quantity: Decimal, volume: Cash) -> GenericResult<Cash> {
    let volume_precision = util::decimal_precision(volume.amount);

    let mut price = volume.amount / quantity;
    let mut price_precision = volume_precision;

    loop {
        let round_price = util::round(price, price_precision);

        if util::round(round_price * quantity, volume_precision) == volume.amount {
            price = round_price.normalize();
            break;
        }

        if price_precision >= 20 {
            return Err!(
                "Unable to calculate {} / {} price with a reasonable precision",
                volume, quantity);
        }
        price_precision += 1;
    }

    Ok(Cash::new(volume.currency, price))
}

pub struct RealProfit {
    pub tax_ratio: Option<Decimal>,
    pub profit_ratio: Option<Decimal>,
    pub local_profit_ratio: Option<Decimal>,
}

pub fn calculate_real_profit<C>(
    date: Date, purchase_cost: C, purchase_local_cost: Cash, profit: C, local_profit: Cash,
    tax_to_pay: Cash, converter: &CurrencyConverter,
) -> GenericResult<RealProfit>
    where C: Into<MultiCurrencyCashAccount>
{
    let local_currency = tax_to_pay.currency;
    let purchase_cost = purchase_cost.into();
    let profit = profit.into();

    let profit_in_local = profit.total_cash_assets(date, local_currency, converter)?;
    let real_tax_ratio = if profit_in_local.is_zero() {
        None
    } else {
        Some(tax_to_pay / profit_in_local)
    };

    let real_profit_in_local = profit_in_local - tax_to_pay;
    let purchase_cost_in_local = purchase_cost.total_cash_assets(date, local_currency, converter)?;
    let real_profit_ratio = if purchase_cost_in_local.is_zero() {
        None
    } else {
        Some(real_profit_in_local / purchase_cost_in_local)
    };

    let real_local_profit = local_profit - tax_to_pay;
    let real_local_profit_ratio = if purchase_local_cost.is_zero() {
        None
    } else {
        Some(real_local_profit / purchase_local_cost)
    };

    Ok(RealProfit {
        tax_ratio: real_tax_ratio,
        profit_ratio: real_profit_ratio,
        local_profit_ratio: real_local_profit_ratio,
    })
}