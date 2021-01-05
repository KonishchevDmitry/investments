use crate::core::{GenericResult, EmptyResult};
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

#[derive(Debug)]
pub enum StockSource {
    Trade,
    CorporateAction,
}

#[derive(Debug)]
pub struct StockBuy {
    pub symbol: String,
    pub quantity: Decimal,
    pub source: StockSource,

    // Please note that all of the following values can be zero due to corporate actions or other
    // non-trade operations:
    pub price: Cash,
    pub volume: Cash, // May be slightly different from price * quantity due to rounding on broker side
    pub commission: Cash,

    pub conclusion_date: Date,
    pub execution_date: Date,
    pub margin: bool,

    sold: Decimal,
}

impl StockBuy {
    pub fn new(
        symbol: &str, quantity: Decimal, source: StockSource,
        price: Cash, volume: Cash, commission: Cash,
        conclusion_date: Date, execution_date: Date, margin: bool,
    ) -> StockBuy {
        StockBuy {
            symbol: symbol.to_owned(), quantity, source, price, volume, commission,
            conclusion_date, execution_date, margin, sold: dec!(0),
        }
    }

    pub fn is_sold(&self) -> bool {
        self.sold == self.quantity
    }

    pub fn get_unsold(&self) -> Decimal {
        self.quantity - self.sold
    }

    pub fn sell(&mut self, quantity: Decimal) {
        assert!(self.get_unsold() >= quantity);
        self.sold += quantity;
    }
}

#[derive(Clone, Debug)]
pub struct StockSell {
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
        symbol: &str, quantity: Decimal, price: Cash, volume: Cash, commission: Cash,
        conclusion_date: Date, execution_date: Date, margin: bool, emulation: bool,
    ) -> StockSell {
        StockSell {
            symbol: symbol.to_owned(), quantity, price, volume, commission,
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

    pub fn convert(&mut self, currency: &str, converter: &CurrencyConverter) -> EmptyResult {
        self.price = converter.convert_to_cash_rounding(self.execution_date, self.price, currency)?;
        self.volume = converter.convert_to_cash_rounding(self.execution_date, self.volume, currency)?;
        self.commission = converter.convert_to_cash_rounding(self.conclusion_date, self.commission, currency)?;

        for source in &mut self.sources {
            source.convert(currency, converter)?;
        }

        Ok(())
    }

    pub fn calculate(
        &self, country: &Country, tax_year: i32, tax_exemptions: &[TaxExemption],
        converter: &CurrencyConverter,
    ) -> GenericResult<SellDetails> {
        Ok(self.calculate_impl(country, tax_year, tax_exemptions, converter).map_err(|e| format!(
            "Failed to calculate results of {} selling order from {}: {}",
            self.symbol, formatting::format_date(self.conclusion_date), e))?)
    }

    fn calculate_impl(
        &self, country: &Country, tax_year: i32, tax_exemptions: &[TaxExemption],
        converter: &CurrencyConverter,
    ) -> GenericResult<SellDetails> {
        let local_conclusion = |value| converter.convert_to_cash_rounding(
            self.conclusion_date, value, country.currency);
        let local_execution = |value| converter.convert_to_cash_rounding(
            self.execution_date, value, country.currency);

        let mut purchase_cost = Cash::new(self.price.currency, dec!(0));
        let mut purchase_local_cost = Cash::new(country.currency, dec!(0));
        let mut deductible_purchase_local_cost = Cash::new(country.currency, dec!(0));

        let mut fifo = Vec::new();
        let mut total_quantity = dec!(0);
        let mut tax_free_quantity = dec!(0);

        for source in &self.sources {
            let source_quantity = source.quantity * source.multiplier;
            let mut source_details = source.calculate(country, converter)?;

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
                    .sub(source_details.total_local_cost).unwrap();

                source_details.tax_exemption_applied = source_local_profit.is_positive();
            }

            total_quantity += source_quantity;
            if source_details.tax_exemption_applied {
                tax_free_quantity += source_quantity;
            }

            purchase_cost.add_assign(source_details.total_cost).map_err(|e| format!(
                "Sell and buy trades have different currency: {}", e))?;
            purchase_local_cost.add_assign(source_details.total_local_cost).unwrap();
            if !source_details.tax_exemption_applied {
                deductible_purchase_local_cost.add_assign(source_details.total_local_cost).unwrap();
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

        let total_cost = commission.add(purchase_cost).map_err(|e| format!(
            "Sell and buy trade have different currency: {}", e))?;
        let total_local_cost = local_commission.add(purchase_local_cost).unwrap();
        let deductible_total_local_cost = deductible_local_commission.add(deductible_purchase_local_cost).unwrap();

        let profit = revenue.sub(total_cost).map_err(|e| format!(
            "Sell and buy trade have different currency: {}", e))?;
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
            self.execution_date, tax_to_pay, profit.currency)?)?;
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

            purchase_cost,
            purchase_local_cost,

            total_cost,
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
    pub commission: Cash,

    pub conclusion_date: Date,
    pub execution_date: Date,
}

impl StockSellSource {
    fn convert(&mut self, currency: &str, converter: &CurrencyConverter) -> EmptyResult {
        self.price = converter.convert_to_cash_rounding(self.execution_date, self.price, currency)?;
        self.commission = converter.convert_to_cash_rounding(self.conclusion_date, self.commission, currency)?;
        Ok(())
    }

    fn calculate(&self, country: &Country, converter: &CurrencyConverter) -> GenericResult<FifoDetails> {
        let cost = (self.price * self.quantity).round();
        let local_cost = converter.convert_to_rounding(
            self.execution_date, cost, country.currency)?;

        let commission = self.commission.round();
        let local_commission = converter.convert_to_rounding(
            self.conclusion_date, commission, country.currency)?;

        let mut total_cost = cost;
        let mut total_local_cost = local_cost;

        total_cost.add_assign(self.commission.round()).map_err(|e| format!(
            "Trade and commission have different currency: {}", e))?;
        total_local_cost += local_commission;

        Ok(FifoDetails {
            quantity: self.quantity,
            multiplier: self.multiplier,
            price: self.price,

            commission: commission,
            local_commission: Cash::new(country.currency, local_commission),

            conclusion_date: self.conclusion_date,
            execution_date: self.execution_date,

            cost,
            local_cost: Cash::new(country.currency, local_cost),

            total_cost,
            total_local_cost: Cash::new(country.currency, total_local_cost),

            tax_exemption_applied: false,
        })
    }
}

pub struct SellDetails {
    pub revenue: Cash,
    pub local_revenue: Cash,
    pub local_commission: Cash,

    // Please note that all of the following values can be zero due to corporate actions or other
    // non-trade operations:
    pub purchase_cost: Cash,
    pub purchase_local_cost: Cash,
    pub total_cost: Cash,
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

pub struct FifoDetails {
    pub quantity: Decimal,
    pub multiplier: Decimal,

    pub conclusion_date: Date,
    pub execution_date: Date,

    // Please note that all of the following values can be zero due to corporate actions or other
    // non-trade operations:
    pub price: Cash,
    pub commission: Cash,
    pub local_commission: Cash,
    // and:
    pub cost: Cash,
    pub local_cost: Cash,
    pub total_cost: Cash,
    pub total_local_cost: Cash,

    pub tax_exemption_applied: bool,
}