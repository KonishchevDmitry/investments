use chrono::Datelike;

use crate::core::GenericResult;
use crate::time::Date;
use crate::types::Decimal;

pub struct InflationCalc {
    today: Date,
    get_inflation: fn(year: i32) -> Option<Decimal>
}

impl InflationCalc {
    pub fn new(currency: &str, today: Date) -> GenericResult<InflationCalc> {
        Ok(InflationCalc {
            today,
            get_inflation: match currency {
                "RUB" => russia_inflation,
                "USD" => us_inflation,
                #[cfg(test)] "test" => tests::test_inflation,
                _ => return Err!("{} currency is not supported by inflation calculator", currency),
            },
        })
    }

    pub fn adjust(&self, mut date: Date, mut amount: Decimal) -> Decimal {
        while date < self.today {
            let year = date.year();

            let period = if year == self.today.year() {
                self.today - date
            } else {
                Date::from_ymd_opt(date.year() + 1, 1, 1).unwrap() - date
            };

            if let Some(inflation) = (self.get_inflation)(year) {
                let days_in_year = (
                    Date::from_ymd_opt(year + 1, 1, 1).unwrap() - Date::from_ymd_opt(year, 1, 1).unwrap()
                ).num_days();

                amount += amount * inflation / dec!(100) * Decimal::from(period.num_days()) / Decimal::from(days_in_year);
            }

            date += period;
        }

        amount
    }
}

fn russia_inflation(year: i32) -> Option<Decimal> {
    // https://www.statbureau.org/ru/russia/inflation-tables
    Some(match year {
        1991 => dec!(160.40),
        1992 => dec!(2508.85),
        1993 => dec!(839.87),
        1994 => dec!(215.02),
        1995 => dec!(131.33),
        1996 => dec!(21.81),
        1997 => dec!(11.03),
        1998 => dec!(84.44),
        1999 => dec!(36.56),
        2000 => dec!(20.20),
        2001 => dec!(18.58),
        2002 => dec!(15.06),
        2003 => dec!(11.99),
        2004 => dec!(11.74),
        2005 => dec!(10.91),
        2006 => dec!(9.00),
        2007 => dec!(11.87),
        2008 => dec!(13.28),
        2009 => dec!(8.80),
        2010 => dec!(8.78),
        2011 => dec!(6.10),
        2012 => dec!(6.58),
        2013 => dec!(6.45),
        2014 => dec!(11.36),
        2015 => dec!(12.91),
        2016 => dec!(5.38),
        2017 => dec!(2.52),
        2018 => dec!(4.27),
        2019 => dec!(3.05),
        2020 => dec!(4.91),
        2021 => dec!(8.39),
        2022 => dec!(11.92),
        2023 => dec!(7.42),
        2024 => dec!(9.51),
        _ => return None,
    })
}

fn us_inflation(year: i32) -> Option<Decimal> {
    // https://fred.stlouisfed.org/series/FPCPITOTLZGUSA
    // https://www.usinflationcalculator.com/inflation/historical-inflation-rates/
    Some(match year {
        1960 => dec!(1.45797598627786),
        1961 => dec!(1.07072414764723),
        1962 => dec!(1.19877334820185),
        1963 => dec!(1.2396694214876),
        1964 => dec!(1.27891156462583),
        1965 => dec!(1.58516926383669),
        1966 => dec!(3.01507537688439),
        1967 => dec!(2.77278562259307),
        1968 => dec!(4.27179615288534),
        1969 => dec!(5.4623862002875),
        1970 => dec!(5.83825533848253),
        1971 => dec!(4.29276668813045),
        1972 => dec!(3.27227824655283),
        1973 => dec!(6.17776006377041),
        1974 => dec!(11.0548048048048),
        1975 => dec!(9.14314686496534),
        1976 => dec!(5.74481263549085),
        1977 => dec!(6.50168399472839),
        1978 => dec!(7.63096383885602),
        1979 => dec!(11.2544711292795),
        1980 => dec!(13.5492019749684),
        1981 => dec!(10.3347153402771),
        1982 => dec!(6.13142700027494),
        1983 => dec!(3.21243523316063),
        1984 => dec!(4.30053547523427),
        1985 => dec!(3.54564415209369),
        1986 => dec!(1.89804772234275),
        1987 => dec!(3.66456321751691),
        1988 => dec!(4.07774110744408),
        1989 => dec!(4.82700303008949),
        1990 => dec!(5.39795643990322),
        1991 => dec!(4.23496396453853),
        1992 => dec!(3.0288196781497),
        1993 => dec!(2.95165696638554),
        1994 => dec!(2.6074415921546),
        1995 => dec!(2.80541968853655),
        1996 => dec!(2.9312041999344),
        1997 => dec!(2.33768993730741),
        1998 => dec!(1.55227909874362),
        1999 => dec!(2.18802719697358),
        2000 => dec!(3.37685727149935),
        2001 => dec!(2.82617111885402),
        2002 => dec!(1.58603162650603),
        2003 => dec!(2.27009497336113),
        2004 => dec!(2.67723669309173),
        2005 => dec!(3.39274684549547),
        2006 => dec!(3.22594410070407),
        2007 => dec!(2.85267248150136),
        2008 => dec!(3.83910029665101),
        2009 => dec!(-0.35554626629975),
        2010 => dec!(1.64004344238989),
        2011 => dec!(3.15684156862206),
        2012 => dec!(2.06933726526059),
        2013 => dec!(1.46483265562714),
        2014 => dec!(1.62222297740821),
        2015 => dec!(0.118627135552435),
        2016 => dec!(1.26158320570537),
        2017 => dec!(2.13011000365963),
        2018 => dec!(2.44258329692818),
        2019 => dec!(1.81221007526015),
        2020 => dec!(1.23358439630637),
        2021 => dec!(4.69785886363739),
        2022 => dec!(8.00279982052117),
        2023 => dec!(4.11633838374488),
        2024 => dec!(2.9),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use crate::util;
    use super::*;

    #[test]
    fn calculator() {
        let check = |result, expected| {
            assert_eq!(
                util::round(result, 20),
                util::round(expected, 20),
            )
        };

        let calc = InflationCalc::new("test", date!(1962, 1, 5)).unwrap();
        assert_eq!((date!(1963, 1, 1) - date!(1962, 1, 1)).num_days(), 365);
        check(
            calc.adjust(date!(1958, 3, 4), dec!(123)),
            dec!(123)
                * (dec!(1) + dec!(1.45797598627786) / dec!(100)) // 1960
                * (dec!(1) + dec!(1.07072414764723) / dec!(100)) // 1961
                * (dec!(1) + dec!(1.19877334820185) / dec!(100) * dec!(4) / dec!(365))
        );

        let calc = InflationCalc::new("test", date!(2010, 4, 6)).unwrap();
        assert_eq!((date!(2008, 1, 1) - date!(2007, 1, 1)).num_days(), 365);
        assert_eq!((date!(2008, 1, 1) - date!(2007, 7, 3)).num_days(), 182);
        assert_eq!((date!(2010, 4, 6) - date!(2010, 1, 1)).num_days(), 95);
        assert_eq!((date!(2011, 1, 1) - date!(2010, 1, 1)).num_days(), 365);
        check(
            calc.adjust(date!(2007, 7, 3), dec!(123)),
            dec!(123)
                * (dec!(1) + dec!(2.85267248150136) / dec!(100) * dec!(182) / dec!(365))  // 2007
                * (dec!(1) + dec!(3.83910029665101) / dec!(100))                          // 2008
                * (dec!(1) + dec!(-0.35554626629975) / dec!(100))                         // 2009
                * (dec!(1) + dec!(1.64004344238989) / dec!(100) * dec!(95) / dec!(365))   // 2010
        );

        let calc = InflationCalc::new("test", date!(2023, 10, 7)).unwrap();
        assert_eq!((date!(2021, 1, 1) - date!(2020, 1, 1)).num_days(), 366);
        assert_eq!((date!(2021, 1, 1) - date!(2020, 7, 3)).num_days(), 182);
        check(
            calc.adjust(date!(2020, 7, 3), dec!(123)),
            dec!(123)
                * (dec!(1) + dec!(1.23358439630637) / dec!(100) * dec!(182) / dec!(366))  // 2020
                * (dec!(1) + dec!(4.69785886363739) / dec!(100))                          // 2021
                * (dec!(1) + dec!(8.00279982052117) / dec!(100))                          // 2022
        );
    }

    pub fn test_inflation(year: i32) -> Option<Decimal> {
        if year < 2023 {
            us_inflation(year)
        } else {
            None
        }
    }
}