## Investments

A work in progress project to organize my investments.

### Available functionality

```
$ investments tax-statement --help

Reads Interactive Brokers statement and alters *.dcX file (Russian tax program named Декларация)
by adding all required information about income from paid dividends.

If tax statement file is not specified only outputs the data which is going to be declared.

USAGE:
    investments tax-statement <YEAR> <BROKER_STATEMENT> [TAX_STATEMENT]

ARGS:
    <YEAR>
            Year to generate the statement for

    <BROKER_STATEMENT>
            Path to Interactive Brokers statement *.csv file

    <TAX_STATEMENT>
            Path to tax statement *.dcX file
```
