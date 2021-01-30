mod builders;

use std::collections::{BTreeMap, HashMap};
use std::ops::Bound;

use chrono::Datelike;
use num_traits::{cast::ToPrimitive, Zero};

use crate::core::GenericResult;
use crate::currency::{Cash, MultiCurrencyCashAccount};
use crate::currency::converter::CurrencyConverterRc;
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

impl CommissionSpec {
    fn round(&self, amount: Decimal) -> Decimal {
        util::round_with(amount, 2, self.rounding_method)
    }

    fn round_cash(&self, mut amount: Cash) -> Cash {
        amount.amount = self.round(amount.amount);
        amount
    }
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
    fn calculate(&self, calc: &CommissionCalc, date: Date, shares: u32, volume: Cash) -> GenericResult<Cash> {
        let mut commission = dec!(0);
        let currency = volume.currency;
        let convert = |amount| calc.converter.convert(calc.spec.currency, currency, date, amount);

        if let Some(per_share) = self.per_share {
            commission += convert(per_share)? * Decimal::from(shares);
        }

        if let Some(percent) = self.percent {
            commission += volume.amount * percent / dec!(100);
        }

        if let Some(maximum_percent) = self.maximum_percent {
            let max_commission = volume.amount * maximum_percent / dec!(100);
            if commission > max_commission {
                commission = max_commission;
            }
        }

        if let Some(minimum) = self.minimum {
            let minimum = convert(minimum)?;
            if commission < minimum {
                commission = minimum
            }
        }

        Ok(Cash::new(currency, commission))
    }
}

#[derive(Default, Clone, Debug)]
pub struct CumulativeCommissionSpec {
    // Broker commissions
    percent: Option<CumulativeTieredSpec>,
    minimum_daily: Option<Decimal>,
    minimum_monthly: Option<Decimal>,

    // Additional fees (exchange, regulatory and clearing)
    fees: Vec<CumulativeFeeSpec>,

    // Depositary (tiered by portfolio net value)
    monthly_depositary: BTreeMap<Decimal, Decimal>,
}

#[derive(Clone, Copy, Debug)]
pub enum CumulativeTierType {
    Volume,
    PortfolioNetValue,
}

#[derive(Clone, Debug)]
pub struct CumulativeTieredSpec {
    _type: CumulativeTierType,
    tiers: BTreeMap<Decimal, Decimal>,
}

impl CumulativeTieredSpec {
    fn percent(&self, calc: &CommissionCalc, date: Date, volume: Decimal) -> GenericResult<Decimal> {
        let key = match self._type {
            CumulativeTierType::Volume => volume,
            CumulativeTierType::PortfolioNetValue => {
                let portfolio_net_value = calc.converter.convert_to(
                    date, calc.portfolio_net_value, calc.spec.currency)?;
                std::cmp::max(dec!(0), portfolio_net_value)
            },
        };
        Ok(*self.tiers.range((Bound::Unbounded, Bound::Included(key))).last().unwrap().1)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CumulativeFeeSpec {
    percent: Decimal,
}

pub struct CommissionCalc {
    spec: CommissionSpec,
    portfolio_net_value: Cash,
    converter: CurrencyConverterRc,
    volume: HashMap<Date, MultiCurrencyCashAccount>,
}

impl CommissionCalc {
    pub fn new(converter: CurrencyConverterRc, spec: CommissionSpec, portfolio_net_value: Cash) -> GenericResult<CommissionCalc> {
        Ok(CommissionCalc {
            spec, portfolio_net_value, converter,
            volume: HashMap::new(),
        })
    }

    pub fn add_trade(&mut self, date: Date, trade_type: TradeType, shares: Decimal, price: Cash) -> GenericResult<Cash> {
        let commission = self.add_trade_precise(date, trade_type, shares, price)?;
        Ok(self.spec.round_cash(commission))
    }

    pub fn add_trade_precise(&mut self, date: Date, trade_type: TradeType, shares: Decimal, price: Cash) -> GenericResult<Cash> {
        // Commission returned by this method must be independent from any side effects like daily
        // volume and others. Method calls with same arguments must return same results. All
        // accumulation commissions must be calculated separately.

        // We don't know how commissions are calculated for fractional shares yet, so use ceiled
        // value for now.
        let whole_shares = shares.ceil().to_u32().ok_or_else(|| format!(
            "Got an invalid number of shares: {}", shares))?;

        let volume = price * shares;
        self.volume.entry(date).or_default().deposit(volume);

        let mut commission = self.spec.trade.commission.calculate(self, date, whole_shares, volume)?;

        for (transaction_type, fee_spec) in &self.spec.trade.transaction_fees {
            if *transaction_type == trade_type {
                let fee = fee_spec.calculate(self, date, whole_shares, volume)?;
                commission.add_assign(fee)?;
            }
        }

        Ok(commission)
    }

    pub fn calculate(self) -> GenericResult<HashMap<Date, MultiCurrencyCashAccount>> {
        let mut total_by_date = HashMap::new();
        let mut monthly: HashMap<_, Decimal> = HashMap::new();

        for (&date, volume) in &self.volume {
            let (commissions, fees) = self.calculate_daily(date, volume)?;

            let mut total = MultiCurrencyCashAccount::new();
            total.add(&commissions);
            total.add(&fees);

            if !total.is_empty() {
                total_by_date.insert(date, total);
            }

            let total_commission = self.spec.round(commissions.total_assets(
                date, self.spec.currency, &self.converter)?);
            *monthly.entry((date.year(), date.month())).or_default() += total_commission;
        }

        if let Some(minimum_monthly) = self.spec.cumulative.minimum_monthly {
            for (&(year, month), &commission) in &monthly {
                if commission < minimum_monthly {
                    let date = get_monthly_commission_date(year, month);
                    let additional_commission = minimum_monthly - commission;
                    total_by_date.entry(date).or_default().deposit(
                        Cash::new(self.spec.currency, additional_commission));
                }
            }
        }

        if !self.spec.cumulative.monthly_depositary.is_empty() {
            let portfolio_net_value = self.converter.real_time_convert_to(
                self.portfolio_net_value, self.spec.currency)?;

            let monthly_depositary = *self.spec.cumulative.monthly_depositary
                .range((Bound::Unbounded, Bound::Included(std::cmp::max(dec!(0), portfolio_net_value))))
                .last().unwrap().1;

            if !monthly_depositary.is_zero() {
                for &(year, month) in monthly.keys() {
                    let date = get_monthly_commission_date(year, month);
                    total_by_date.entry(date).or_default().deposit(
                        Cash::new(self.spec.currency, monthly_depositary));
                }
            }
        }

        Ok(total_by_date)
    }

    fn calculate_daily(
        &self, date: Date, volumes: &MultiCurrencyCashAccount
    ) -> GenericResult<(MultiCurrencyCashAccount, MultiCurrencyCashAccount)> {
        let mut commissions = MultiCurrencyCashAccount::new();

        if let Some(ref tiers) = self.spec.cumulative.percent {
            let total_volume = volumes.total_assets(date, self.spec.currency, &self.converter)?;
            let percent = tiers.percent(self, date, total_volume)?;

            for volume in volumes.iter() {
                let commission = self.spec.round_cash(volume * percent / dec!(100));
                if commission.is_positive() {
                    commissions.deposit(commission);
                }
            }
        };

        if let Some(minimum) = self.spec.cumulative.minimum_daily {
            let total_commission = self.spec.round(commissions.total_assets(
                date, self.spec.currency, &self.converter)?);

            if total_commission < minimum {
                let additional_commission = minimum - total_commission;
                commissions.deposit(Cash::new(self.spec.currency, additional_commission));
            }
        }

        let mut fees = MultiCurrencyCashAccount::new();
        for fee in &self.spec.cumulative.fees {
            for volume in volumes.iter() {
                let fee = self.spec.round_cash(volume * fee.percent / dec!(100));
                if fee.is_positive() {
                    fees.deposit(fee);
                }
            }
        }

        Ok((commissions, fees))
    }
}

fn get_monthly_commission_date(year: i32, month: u32) -> Date {
    if month == 12 {
        Date::from_ymd(year + 1, 1, 1)
    } else {
        Date::from_ymd(year, month + 1, 1)
    }
}