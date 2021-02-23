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

    let mut converted_price = converted_volume / quantity;
    let mut price_precision = volume_precision;

    loop {
        let round_price = util::round(converted_price, price_precision);

        if util::round(round_price * quantity, volume_precision) == converted_volume {
            converted_price = round_price.normalize();
            break;
        }

        if price_precision >= 20 {
            return Err!(
                "Unable to convert {} x {} price to {} with reasonable precision",
                quantity, price, currency);
        }
        price_precision += 1;
    }

    Ok(Cash::new(currency, converted_price))
}