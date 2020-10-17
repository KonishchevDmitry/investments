mod builders;

use std::collections::{BTreeMap, HashMap};
use std::ops::Bound;

use chrono::Datelike;
use num_traits::{cast::ToPrimitive, Zero};

use crate::core::GenericResult;
use crate::currency::{Cash, converter::CurrencyConverter};
use crate::types::{Date, Decimal, TradeType};
use crate::util::{self, RoundingMethod};

pub use builders::*;

#[derive(Clone, Debug)]
pub struct CommissionSpec {
    currency: &'static str,
    rounding_method: RoundingMethod,

    trade: TradeCommissionSpec,
    cumulative: CumulativeCommissionSpec,
}

#[derive(Default, Clone, Debug)]
pub struct TradeCommissionSpec {
    commission: TransactionCommissionSpec,
    transaction_fees: Vec<(TradeType, TransactionCommissionSpec)>,
}

#[derive(Default, Clone, Copy, Debug)]
pub struct TransactionCommissionSpec {
    percent: Option<Decimal>,
    per_share: Option<Decimal>,

    minimum: Option<Decimal>,
    maximum_percent: Option<Decimal>,
}

impl TransactionCommissionSpec {
    fn calculate(&self, shares: u32, volume: Decimal) -> Decimal {
        let mut commission = dec!(0);

        if let Some(per_share) = self.per_share {
            commission += per_share * Decimal::from(shares);
        }

        if let Some(percent) = self.percent {
            commission += volume * percent / dec!(100);
        }

        if let Some(maximum_percent) = self.maximum_percent {
            let max_commission = volume * maximum_percent / dec!(100);
            if commission > max_commission {
                commission = max_commission;
            }
        }

        if let Some(minimum) = self.minimum {
            if commission < minimum {
                commission = minimum
            }
        }

        commission
    }
}

#[derive(Default, Clone, Debug)]
pub struct CumulativeCommissionSpec {
    // Broker commissions
    tiers: Option<BTreeMap<Decimal, Decimal>>,
    minimum_daily: Option<Decimal>,
    minimum_monthly: Option<Decimal>,

    // Additional fees (exchange, regulatory and clearing)
    fees: Vec<CumulativeFeeSpec>,

    // Depositary (tiered by portfolio size)
    monthly_depositary: BTreeMap<Decimal, Decimal>,
}

#[derive(Clone, Copy, Debug)]
pub struct CumulativeFeeSpec {
    percent: Decimal,
}

pub struct CommissionCalc {
    spec: CommissionSpec,
    volume: HashMap<Date, Decimal>,
    portfolio_net_value: Decimal,
}

impl CommissionCalc {
    pub fn new(converter: &CurrencyConverter, spec: CommissionSpec, portfolio_net_value: Cash) -> GenericResult<CommissionCalc> {
        let portfolio_net_value = converter.real_time_convert_to(portfolio_net_value, &spec.currency)?;
        Ok(CommissionCalc {
            spec,
            volume: HashMap::new(),
            portfolio_net_value,
        })
    }

    pub fn add_trade(&mut self, date: Date, trade_type: TradeType, shares: Decimal, price: Cash) -> GenericResult<Cash> {
        let mut commission = self.add_trade_precise(date, trade_type, shares, price)?;
        commission.amount = util::round_with(commission.amount, 2, self.spec.rounding_method);
        Ok(commission)
    }

    pub fn add_trade_precise(&mut self, date: Date, trade_type: TradeType, shares: Decimal, price: Cash) -> GenericResult<Cash> {
        // Commission returned by this method must be independent from any side effects like daily
        // volume and others. Method calls with same arguments must return same results. All
        // accumulation commissions must be calculated separately.

        // We don't know how commissions are calculated for fractional shares yet, so use ceiled
        // value for now.
        let whole_shares = shares.ceil().to_u32().ok_or_else(|| format!(
            "Got an invalid number of shares: {}", shares))?;

        let volume = get_trade_volume(self.spec.currency, price * shares)?;
        *self.volume.entry(date).or_default() += volume;

        let mut commission = self.spec.trade.commission.calculate(whole_shares, volume);

        for (transaction_type, fee_spec) in &self.spec.trade.transaction_fees {
            if *transaction_type == trade_type {
                commission += fee_spec.calculate(whole_shares, volume);
            }
        }

        Ok(Cash::new(self.spec.currency, commission))
    }

    pub fn calculate(self) -> HashMap<Date, Cash> {
        let mut total_by_date = HashMap::new();
        let mut monthly = HashMap::new();

        for (&date, &volume) in &self.volume {
            let (commission, fee) = self.calculate_daily(volume);

            let total = commission + fee;
            if !total.is_zero() {
                total_by_date.insert(date, total);
            }

            monthly.entry((date.year(), date.month()))
                .and_modify(|total| *total += commission)
                .or_insert(commission);
        }

        if let Some(minimum_monthly) = self.spec.cumulative.minimum_monthly {
            for (&(year, month), &commission) in &monthly {
                if commission < minimum_monthly {
                    let additional_commission = minimum_monthly - commission;
                    total_by_date.entry(get_monthly_commission_date(year, month))
                        .and_modify(|total| *total += additional_commission)
                        .or_insert(additional_commission);
                }
            }
        }

        if !self.spec.cumulative.monthly_depositary.is_empty() {
            let monthly_depositary = *self.spec.cumulative.monthly_depositary
                .range((Bound::Unbounded, Bound::Included(std::cmp::max(dec!(0), self.portfolio_net_value))))
                .last().unwrap().1;

            if !monthly_depositary.is_zero() {
                for &(year, month) in monthly.keys() {
                    total_by_date.entry(get_monthly_commission_date(year, month))
                        .and_modify(|total| *total += monthly_depositary)
                        .or_insert(monthly_depositary);
                }
            }
        }

        total_by_date.iter().map(|(&date, &commission)| {
            (date, Cash::new(self.spec.currency, commission))
        }).collect()
    }

    fn calculate_daily(&self, volume: Decimal) -> (Decimal, Decimal) {
        let mut commission = if let Some(ref tiers) = self.spec.cumulative.tiers {
            let percent = *tiers.range((Bound::Unbounded, Bound::Included(volume)))
                .last().unwrap().1;

            util::round_with(volume * percent / dec!(100), 2, self.spec.rounding_method)
        } else {
            dec!(0)
        };

        if let Some(minimum) = self.spec.cumulative.minimum_daily {
            if commission < minimum {
                commission = minimum;
            }
        }

        let mut fees = dec!(0);
        for fee in &self.spec.cumulative.fees {
            fees += util::round_with(volume * fee.percent / dec!(100), 2, self.spec.rounding_method);
        }

        (commission, fees)
    }
}

fn get_trade_volume(commission_currency: &str, volume: Cash) -> GenericResult<Decimal> {
    if volume.currency != commission_currency {
        return Err!(concat!(
            "Unable to calculate trade commission: ",
            "Commission currency doesn't match trade currency: {} vs {}"),
            commission_currency, volume.currency
        );
    }

    Ok(volume.amount)
}

fn get_monthly_commission_date(year: i32, month: u32) -> Date {
    if month == 12 {
        Date::from_ymd(year + 1, 1, 1)
    } else {
        Date::from_ymd(year, month + 1, 1)
    }
}