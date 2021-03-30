use crate::core::GenericResult;
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
use crate::formatting;
use crate::localities::Country;
use crate::taxes::{IncomeType, TaxExemption};
use crate::types::{Date, Decimal};

#[derive(Debug)]
pub struct ForexTrade {
    pub from: Cash,
    pub to: Cash,
    pub commission: Cash,
    pub conclusion_date: Date,
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum StockBuyType {
    // Ordinary trade
    Trade,

    // Non-trade operation due to a corporate action that doesn't affect cash balance:
    // * Emulated buy to convert position during stock split
    // * Spinoff or stock dividend
    CorporateAction,
}

#[derive(Debug)]
pub struct StockBuy {
    pub type_: StockBuyType,
    pub symbol: String,
    pub quantity: Decimal,

    // Please note that all of the following values may be zero due to corporate actions or other
    // non-trade operations:
    pub price: Cash,
    pub volume: Cash, // May be slightly different from price * quantity due to rounding on broker side
    pub commission: Cash,

    // On stock split we generate fake sell+buy transactions for position conversion, but it gets us
    // into currency revaluation issues, so we have to keep original date+amount pairs for proper
    // calculation in other currencies.
    // FIXME(konishchev): When to fill?
    purchase_transactions: Vec<PurchaseTransaction>,

    pub conclusion_date: Date,
    pub execution_date: Date,
    pub margin: bool,

    sold: Decimal,
}

impl StockBuy {
    pub fn new(
        type_: StockBuyType, symbol: &str, quantity: Decimal,
        price: Cash, volume: Cash, commission: Cash,
        conclusion_date: Date, execution_date: Date, margin: bool,
    ) -> StockBuy {
        StockBuy {
            type_, symbol: symbol.to_owned(), quantity, price, volume, commission,
            purchase_transactions: Vec::new(), conclusion_date, execution_date, margin,
            sold: dec!(0),
        }
    }

    pub fn is_sold(&self) -> bool {
        self.sold == self.quantity
    }

    pub fn get_unsold(&self) -> Decimal {
        self.quantity - self.sold
    }

    pub fn sell(&mut self, quantity: Decimal, multiplier: Decimal) -> StockSellSource {
        assert!(self.get_unsold() >= quantity);
        self.sold += quantity;

        let (volume, commission) = if quantity == self.quantity {
            (self.volume, self.commission)
        } else {
            (
                self.price * quantity,
                self.commission / self.quantity * quantity,
            )
        };

        StockSellSource {
            quantity,
            multiplier,

            price: self.price,
            volume,
            commission,

            conclusion_date: self.conclusion_date,
            execution_date: self.execution_date,
        }
    }
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum StockSellType {
    // Ordinary trade
    Trade,

    // Non-trade operation due to a corporate action that doesn't affect cash balance and doesn't
    // lead to any taxes:
    // * Emulated sell to convert position during stock split
    CorporateAction,
}

#[derive(Clone, Debug)]
pub struct StockSell {
    pub type_: StockSellType,
    pub symbol: String,
    pub quantity: Decimal,

    pub price: Cash,
    pub volume: Cash, // May be slightly different from price * quantity due to rounding on broker side
    pub commission: Cash,

    pub conclusion_date: Date,
    pub execution_date: Date,
    pub margin: bool,

    pub emulation: bool,
    sources: Vec<StockSellSource>,
}

impl StockSell {
    pub fn new(
        type_: StockSellType, symbol: &str, quantity: Decimal, price: Cash, volume: Cash, commission: Cash,
        conclusion_date: Date, execution_date: Date, margin: bool, emulation: bool,
    ) -> StockSell {
        StockSell {
            type_, symbol: symbol.to_owned(), quantity, price, volume, commission,
            conclusion_date, execution_date, margin,
            emulation, sources: Vec::new(),
        }
    }

    pub fn is_processed(&self) -> bool {
        !self.sources.is_empty()
    }

    pub fn process(&mut self, sources: Vec<StockSellSource>) {
        assert!(!self.is_processed());
        assert_eq!(
            sources.iter()
                .map(|source| source.multiplier * source.quantity)
                .sum::<Decimal>(),
            self.quantity,
        );
        self.sources = sources;
    }

    pub fn calculate(
        &self, country: &Country, tax_year: i32, tax_exemptions: &[TaxExemption],
        converter: &CurrencyConverter,
    ) -> GenericResult<SellDetails> {
        Ok(self.calculate_impl(country, tax_year, tax_exemptions, converter).map_err(|e| format!(
            "Failed to calculate results of {} selling order from {}: {}",
            self.symbol, formatting::format_date(self.conclusion_date), e))?)
    }

    // FIXME(konishchev): Support corporate actions
    fn calculate_impl(
        &self, country: &Country, tax_year: i32, tax_exemptions: &[TaxExemption],
        converter: &CurrencyConverter,
    ) -> GenericResult<SellDetails> {
        let currency = self.price.currency;

        let local_conclusion = |value| converter.convert_to_cash_rounding(
            self.conclusion_date, value, country.currency);
        let local_execution = |value| converter.convert_to_cash_rounding(
            self.execution_date, value, country.currency);

        let mut purchase_cost = Cash::new(currency, dec!(0));
        let mut purchase_local_cost = Cash::new(country.currency, dec!(0));
        let mut deductible_purchase_local_cost = Cash::new(country.currency, dec!(0));

        let mut fifo = Vec::new();
        let mut total_quantity = dec!(0);
        let mut tax_free_quantity = dec!(0);

        for source in &self.sources {
            let source_quantity = source.quantity * source.multiplier;

            let mut source_details = FifoDetails::new(source, country, converter)?;
            let source_total_cost = source_details.total_cost(currency, converter)?;
            let source_total_local_cost = source_details.total_cost(country.currency, converter)?;

            let mut tax_exemptible = false;
            for tax_exemption in tax_exemptions {
                let (exemptible, force) = tax_exemption.is_applicable();
                tax_exemptible |= exemptible;
                if force {
                    source_details.tax_exemption_applied = true;
                    break;
                }
            }

            if tax_exemptible && !source_details.tax_exemption_applied {
                let source_local_revenue = local_execution(self.price * source_quantity)?;
                let source_local_commission = local_conclusion(
                    self.commission * source_quantity / self.quantity)?;

                let source_local_profit = source_local_revenue
                    .sub(source_local_commission).unwrap()
                    .sub(source_total_local_cost).unwrap();

                source_details.tax_exemption_applied = source_local_profit.is_positive();
            }

            total_quantity += source_quantity;
            if source_details.tax_exemption_applied {
                tax_free_quantity += source_quantity;
            }

            purchase_cost.add_assign(source_total_cost).unwrap();
            purchase_local_cost.add_assign(source_total_local_cost).unwrap();
            if !source_details.tax_exemption_applied {
                deductible_purchase_local_cost.add_assign(source_total_local_cost).unwrap();
            }

            fifo.push(source_details);
        }

        assert_eq!(total_quantity, self.quantity);
        let taxable_ratio = (total_quantity - tax_free_quantity) / total_quantity;

        let revenue = self.volume.round();
        let local_revenue = local_execution(revenue)?;
        let taxable_local_revenue = local_execution(revenue * taxable_ratio)?;

        let commission = self.commission.round();
        let local_commission = local_conclusion(commission)?;
        let deductible_local_commission = local_conclusion(commission * taxable_ratio)?;

        let total_cost = purchase_cost.add(converter.convert_to_cash_rounding(
            self.conclusion_date, commission, currency)?).unwrap();
        let total_local_cost = purchase_local_cost.add(local_commission).unwrap();
        let deductible_total_local_cost = deductible_purchase_local_cost.add(deductible_local_commission).unwrap();

        let profit = revenue.sub(total_cost).unwrap();
        let local_profit = local_revenue.sub(total_local_cost).unwrap();
        let taxable_local_profit = taxable_local_revenue.sub(deductible_total_local_cost).unwrap();

        let tax_without_deduction = Cash::new(country.currency, country.tax_to_pay(
            IncomeType::Trading, tax_year, local_profit.amount, None));
        let tax_to_pay = Cash::new(country.currency, country.tax_to_pay(
            IncomeType::Trading, tax_year, taxable_local_profit.amount, None));
        let tax_deduction = tax_without_deduction.sub(tax_to_pay).unwrap();
        assert!(!tax_deduction.is_negative());

        let real_tax_ratio = if profit.is_zero() {
            None
        } else {
            Some(converter.convert_to(self.execution_date, tax_to_pay, profit.currency)? / profit.amount)
        };

        let real_profit = profit.sub(converter.convert_to_cash_rounding(
            self.execution_date, tax_to_pay, currency)?).unwrap();

        let real_profit_ratio = if purchase_cost.is_zero() {
            None
        } else {
            Some(real_profit.div(purchase_cost).unwrap())
        };

        let real_local_profit = local_profit.sub(tax_to_pay).unwrap();
        let real_local_profit_ratio = if purchase_local_cost.is_zero() {
            None
        } else {
            Some(real_local_profit.div(purchase_local_cost).unwrap())
        };

        Ok(SellDetails {
            revenue,
            local_revenue,
            local_commission,

            purchase_local_cost,
            total_local_cost,

            profit,
            local_profit,
            taxable_local_profit,

            tax_to_pay,
            tax_deduction,

            real_tax_ratio,
            real_profit_ratio,
            real_local_profit_ratio,

            fifo,
        })
    }
}

#[derive(Clone, Debug)]
pub struct StockSellSource {
    pub quantity: Decimal,
    pub multiplier: Decimal,

    // Please note that the following values can be zero due to corporate actions or other non-trade
    // operations:
    pub price: Cash,
    pub volume: Cash, // May be slightly different from price * quantity due to rounding on broker side
    pub commission: Cash,

    pub conclusion_date: Date,
    pub execution_date: Date,
}

#[derive(Clone, Debug)]
struct PurchaseTransaction {
    date: Date,
    cost: Cash,
}

pub struct SellDetails {
    pub revenue: Cash,
    pub local_revenue: Cash,
    pub local_commission: Cash,

    // Please note that all of the following values can be zero due to corporate actions or other
    // non-trade operations:
    pub purchase_local_cost: Cash,
    pub total_local_cost: Cash,

    pub profit: Cash,
    pub local_profit: Cash,
    pub taxable_local_profit: Cash,

    pub tax_to_pay: Cash,
    pub tax_deduction: Cash,

    pub real_tax_ratio: Option<Decimal>,
    pub real_profit_ratio: Option<Decimal>,
    pub real_local_profit_ratio: Option<Decimal>,

    pub fifo: Vec<FifoDetails>,
}

impl SellDetails {
    pub fn tax_exemption_applied(&self) -> bool {
        if self.fifo.iter().any(|trade| trade.tax_exemption_applied) {
            return true;
        }

        assert_eq!(self.taxable_local_profit, self.local_profit);
        assert!(self.tax_deduction.is_zero());
        false
    }
}

pub enum StockSourceDetails {
    Trade {
        price: Cash,
        volume: Cash,

        commission: Cash,
        local_commission: Cash,

        cost: Cash,
        local_cost: Cash,
    },
    // FIXME(konishchev): Remove
    #[allow(dead_code)]
    CorporateAction,
}

pub struct FifoDetails {
    pub quantity: Decimal,
    pub multiplier: Decimal,

    pub conclusion_date: Date,
    pub execution_date: Date,

    pub type_: StockSourceDetails,
    // FIXME(konishchev): Deprecated:
    // Please note that all of the following values can be zero due to corporate actions or other
    // non-trade operations:
    pub price: Cash,
    pub commission: Cash,
    // and:
    cost: Cash,

    pub tax_exemption_applied: bool,
}

impl FifoDetails {
    fn new(source: &StockSellSource, country: &Country, converter: &CurrencyConverter) -> GenericResult<FifoDetails> {
        let cost = source.volume.round();
        let local_cost = converter.convert_to_cash_rounding(
            source.execution_date, cost, country.currency)?;

        let commission = source.commission.round();
        let local_commission = converter.convert_to_cash_rounding(
            source.conclusion_date, commission, country.currency)?;

        // FIXME(konishchev): Purchase transactions
        Ok(FifoDetails {
            quantity: source.quantity,
            multiplier: source.multiplier,

            conclusion_date: source.conclusion_date,
            execution_date: source.execution_date,

            // FIXME(konishchev): Fill
            type_: StockSourceDetails::Trade {
                price: source.price,
                volume: cost,

                commission,
                local_commission,

                cost,
                local_cost,
            },
            price: source.price,
            commission,

            cost,

            tax_exemption_applied: false,
        })
    }

    // FIXME(konishchev): Purchase transactions
    pub fn cost(&self, currency: &str, converter: &CurrencyConverter) -> GenericResult<Cash> {
        converter.convert_to_cash_rounding(self.execution_date, self.cost, currency)
    }

    // FIXME(konishchev): Purchase transactions
    pub fn total_cost(&self, currency: &str, converter: &CurrencyConverter) -> GenericResult<Cash> {
        let cost = self.cost(currency, converter)?;
        let commission = converter.convert_to_cash_rounding(self.conclusion_date, self.commission, currency)?;
        Ok(cost.add(commission).unwrap())
    }
}