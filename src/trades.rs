use crate::core::GenericResult;
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
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
        "Unable to convert {} x {} price to {}: {}",
        quantity, price, currency, e
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