use crate::core::GenericResult;
use crate::currency::Cash;
use crate::currency::converter::CurrencyConverter;
use crate::localities::Country;
use crate::types::{Date, Decimal};

#[derive(Debug)]
pub struct StockBuy {
    pub symbol: String,
    pub quantity: u32,
    pub price: Cash,
    pub commission: Cash,

    pub conclusion_date: Date,
    pub execution_date: Date,

    sold: u32,
}

impl StockBuy {
    pub fn new(
        symbol: &str, quantity: u32, price: Cash, commission: Cash,
        conclusion_date: Date, execution_date: Date,
    ) -> StockBuy {
        StockBuy {
            symbol: symbol.to_owned(), quantity, price, commission,
            conclusion_date, execution_date, sold: 0,
        }
    }

    pub fn is_sold(&self) -> bool {
        self.sold == self.quantity
    }

    pub fn get_unsold(&self) -> u32 {
        self.quantity - self.sold
    }

    pub fn sell(&mut self, quantity: u32) {
        assert!(self.get_unsold() >= quantity);
        self.sold += quantity;
    }
}

#[derive(Debug)]
pub struct StockSell {
    pub symbol: String,
    pub quantity: u32,
    pub price: Cash,
    pub commission: Cash,

    pub conclusion_date: Date,
    pub execution_date: Date,

    sources: Vec<StockSellSource>,
}

impl StockSell {
    pub fn new(
        symbol: &str, quantity: u32, price: Cash, commission: Cash,
        conclusion_date: Date, execution_date: Date,
    ) -> StockSell {
        StockSell {
            symbol: symbol.to_owned(), quantity, price, commission,
            conclusion_date, execution_date, sources: Vec::new(),
        }
    }

    pub fn is_processed(&self) -> bool {
        !self.sources.is_empty()
    }

    pub fn process(&mut self, sources: Vec<StockSellSource>) {
        assert!(!self.is_processed());
        assert_eq!(sources.iter().map(|source| source.quantity).sum::<u32>(), self.quantity);
        self.sources = sources;
    }
}

#[derive(Debug)]
pub struct StockSellSource {
    pub quantity: u32,
    pub price: Cash,
    pub commission: Cash,

    pub conclusion_date: Date,
    pub execution_date: Date,
}

impl StockSell {
    pub fn tax_to_pay(&self, country: &Country, converter: &CurrencyConverter) -> GenericResult<Decimal> {
        // TODO: We need to use exactly the same rounding logic as in tax statement

        let mut purchase_cost = dec!(0);

        for source in &self.sources {
            purchase_cost += converter.convert_to(
                source.execution_date, source.price * source.quantity, country.currency)?;

            purchase_cost += converter.convert_to(
                source.conclusion_date, source.commission, country.currency)?;
        }

        let mut sell_revenue = converter.convert_to(
            self.execution_date, self.price * self.quantity, country.currency)?;

        sell_revenue -= converter.convert_to(
            self.conclusion_date, self.commission, country.currency)?;

        let income = sell_revenue - purchase_cost;
        if income.is_sign_negative() {
            return Ok(dec!(0));
        }

        Ok(country.tax_to_pay(income, None))
    }
}