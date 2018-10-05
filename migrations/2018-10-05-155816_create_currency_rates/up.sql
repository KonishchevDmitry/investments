CREATE TABLE currency_rates (
  currency TEXT NOT NULL,
  date DATE NOT NULL,
  price TEXT NOT NULL,
  PRIMARY KEY (currency, date)
) WITHOUT ROWID