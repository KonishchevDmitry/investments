# Stock and forex quotes providers

The program needs stock and forex quotes for its work. It's actually a real problem, because there are a very few services that provide it for free and with reasonable API rate limits.

At this time investments uses [FCS API](https://fcsapi.com/) and [Finnhub](https://finnhub.io/), so you have to register, obtain API tokens and specify them in configuration file (see [example config](config-example.yaml)).

If you are client of [T-Bank broker](https://www.tbank.ru/invest/), it's also highly recommended to obtain [T-Bank Invest API sandbox token](https://tinkoff.github.io/investAPI/token/) and specify it in the config: T-Bank has a brilliant API with very high rate limits. When token is specified in config, investments uses T-Bank API for currency and SPB Exchange quotes + also falls back to T-Bank SPB/OTC quotes for other exchanges for which it doesn't have quotes provider yet (LSE and HKEX for example).

## Custom quotes provider

There is also an option to use your own quotes provider. Add the following configuration option:

```yaml
quotes:
  custom_provider:
    url: http://localhost/
```

When custom provider is set the program allows you to not specify any tokens for default providers and uses it first falling back to default ones if it doesn't return requested quotes.

The API is the following: investments sends `GET $url/v1/quotes?symbols=$comma_separated_symbols` HTTP request and expects that the API will return quotes for all requested symbols it has access to in the following JSON format:

```json
{
    "quotes": [{
        "symbol": "USD/RUB",
        "price": "81.7900"
    }, {
        "symbol": "IWDA",
        "price": "79.76",
        "currency": "USD"
    }]
}
```

Here is an [example](https://gist.github.com/dim0xff/7798ffa5d362215ab361bdd47f9f7391) of custom provider for [Yahoo! Finance](https://finance.yahoo.com/).

## Static quotes

And, as a simplest workaround for various possible issues, there is an option to specify static quotes in the configuration file:

```yaml
quotes:
  static:
    RSHE:  95.02 RUB
    83010: 45.26 CNY
```