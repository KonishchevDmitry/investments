CREATE TABLE currency_rates (
  currency TEXT NOT NULL,
  date DATE NOT NULL,
  price TEXT,
  PRIMARY KEY (currency, date)
) WITHOUT ROWID