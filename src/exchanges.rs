use std::ops::Add;

use chrono::Duration;

use crate::time::{self, Date, DateOptTime};

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub enum Exchange {
    Moex,
    Spb,
    Us,
    Otc,
    Other,
}

impl Exchange {
    pub fn trading_mode(self) -> TradingMode {
        // History:
        // * T+2 everywhere
        // * 31.07.2023 MOEX and SPB switched to T+1
        // * 28.05.2024 US switched to T+1
        TradingMode(1)
    }

    pub fn min_last_working_day(self, today: Date) -> Date {
        // Issues due to start of SMO
        if date!(2022, 2, 25) < today && today < date!(2022, 4, 6) {
            date!(2022, 2, 25)
        } else {
            // Experimentally deduced timeout
            today - Duration::days(6)
        }
    }

    pub fn is_valid_execution_date(self, conclusion: Date, execution: Date) -> bool {
        let expected_execution = self.trading_mode().execution_date(conclusion);
        conclusion <= execution && self.min_last_working_day(execution) <= expected_execution
    }
}

pub struct Exchanges(Vec<Exchange>);

impl Exchanges {
    pub fn new(prioritized: &[Exchange]) -> Exchanges {
        Exchanges(prioritized.iter().rev().cloned().collect())
    }

    pub fn new_empty() -> Exchanges {
        Exchanges(Vec::new())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn add_prioritized(&mut self, exchange: Exchange) {
        self.0.retain(|&other| other != exchange);
        self.0.push(exchange);
    }

    pub fn merge(&mut self, other: Exchanges) {
        for exchange in other.0 {
            self.add_prioritized(exchange);
        }
    }

    pub fn get_prioritized(&self) -> Vec<Exchange> {
        self.0.iter().rev().cloned().collect()
    }
}

#[derive(Clone, Copy)]
pub struct TradingMode(u8);

impl TradingMode {
    pub fn execution_date<T: Into<DateOptTime>>(self, conclusion: T) -> Date {
        conclusion.into().date.add(Duration::days(self.0.into()))
    }
}

pub fn today_trade_conclusion_time() -> DateOptTime {
    time::now().into()
}