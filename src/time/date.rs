use super::{Date, Time, DateTime};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct DateOptTime {
    pub date: Date,
    pub time: Option<Time>,
}

impl DateOptTime {
    pub fn new_max_time(date: Date) -> DateOptTime {
        DateOptTime {date, time: Some(Time::from_hms_nano_opt(23, 59, 59, 999_999_999).unwrap())}
    }

    pub fn or_min_time(&self) -> DateTime {
        DateTime::new(self.date, match self.time {
            Some(time) => time,
            None => time!(0, 0, 0),
        })
    }
}

impl From<Date> for DateOptTime {
    fn from(date: Date) -> Self {
        DateOptTime {date, time: None}
    }
}

impl From<DateTime> for DateOptTime {
    fn from(time: DateTime) -> Self {
        DateOptTime {
            date: time.date(),
            time: Some(time.time()),
        }
    }
}