## Investments

Helps you with managing your investments.

Targeted for Russian investors who use [Interactive Brokers](http://interactivebrokers.com) or
[Open Broker](https://open-broker.ru). Considers taxes, commissions, dividends and tax deductions when calculates
portfolio performance.

### Available functionality

```
$ investments analyse --help

Calculates average rate of return from cash investments by comparing portfolio performance
to performance of a bank deposit with exactly the same investments and monthly capitalization.

USAGE:
    investments analyse <PORTFOLIO>

ARGS:
    <PORTFOLIO>    Portfolio name
```

```
$ investments sync --help

Sync portfolio with broker statement

USAGE:
    investments sync <PORTFOLIO>

ARGS:
    <PORTFOLIO>    Portfolio name
```

```
$ investments buy --help

Add the specified stock shares to the portfolio

USAGE:
    investments buy <PORTFOLIO> <SHARES> <SYMBOL> <CASH_ASSETS>

ARGS:
    <PORTFOLIO>      Portfolio name
    <SHARES>         Shares
    <SYMBOL>         Symbol
    <CASH_ASSETS>    Current cash assets
```

```
$ investments sell --help

Remove the specified stock shares from the portfolio

USAGE:
    investments sell <PORTFOLIO> <SHARES> <SYMBOL> <CASH_ASSETS>

ARGS:
    <PORTFOLIO>      Portfolio name
    <SHARES>         Shares
    <SYMBOL>         Symbol
    <CASH_ASSETS>    Current cash assets
```

```
$ investments cash --help

Set current cash assets

USAGE:
    investments cash <PORTFOLIO> <CASH_ASSETS>

ARGS:
    <PORTFOLIO>      Portfolio name
    <CASH_ASSETS>    Current cash assets
```

```
$ investments show --help

Show portfolio's asset allocation

USAGE:
    investments show [FLAGS] <PORTFOLIO>

FLAGS:
    -f, --flat    Flat view

ARGS:
    <PORTFOLIO>    Portfolio name
```

```
$ investments rebalance --help

Rebalance the portfolio according to the asset allocation configuration

USAGE:
    investments rebalance [FLAGS] <PORTFOLIO>

FLAGS:
    -f, --flat    Flat view

ARGS:
    <PORTFOLIO>    Portfolio name
```

```
$ investments tax-statement --help

Reads broker statement and alters *.dcX file (created by Russian tax program named Декларация) by adding
all required information about income from paid dividends.

If tax statement file is not specified only outputs the data which is going to be declared.

USAGE:
    investments tax-statement <PORTFOLIO> <YEAR> [TAX_STATEMENT]

ARGS:
    <PORTFOLIO>
            Portfolio name

    <YEAR>
            Year to generate the statement for

    <TAX_STATEMENT>
            Path to tax statement *.dcX file
```
