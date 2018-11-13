CREATE TABLE assets (
  portfolio TEXT NOT NULL,
  asset_type TEXT CHECK(asset_type IN ('stock', 'cash')) NOT NULL,
  symbol TEXT NOT NULL,
  quantity TEXT NOT NULL,
  PRIMARY KEY (portfolio, asset_type, symbol)
) WITHOUT ROWID