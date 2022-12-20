/// Денежная сумма в определенной валюте
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MoneyValue {
    /// строковый ISO-код валюты
    #[prost(string, tag = "1")]
    pub currency: ::prost::alloc::string::String,
    /// целая часть суммы, может быть отрицательным числом
    #[prost(int64, tag = "2")]
    pub units: i64,
    /// дробная часть суммы, может быть отрицательным числом
    #[prost(int32, tag = "3")]
    pub nano: i32,
}
/// Котировка - денежная сумма без указания валюты
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Quotation {
    /// целая часть суммы, может быть отрицательным числом
    #[prost(int64, tag = "1")]
    pub units: i64,
    /// дробная часть суммы, может быть отрицательным числом
    #[prost(int32, tag = "2")]
    pub nano: i32,
}
/// Проверка активности стрима.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Ping {
    /// Время проверки.
    #[prost(message, optional, tag = "1")]
    pub time: ::core::option::Option<::prost_types::Timestamp>,
}
/// Режим торгов инструмента
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum SecurityTradingStatus {
    /// Торговый статус не определён
    Unspecified = 0,
    /// Недоступен для торгов
    NotAvailableForTrading = 1,
    /// Период открытия торгов
    OpeningPeriod = 2,
    /// Период закрытия торгов
    ClosingPeriod = 3,
    /// Перерыв в торговле
    BreakInTrading = 4,
    /// Нормальная торговля
    NormalTrading = 5,
    /// Аукцион закрытия
    ClosingAuction = 6,
    /// Аукцион крупных пакетов
    DarkPoolAuction = 7,
    /// Дискретный аукцион
    DiscreteAuction = 8,
    /// Аукцион открытия
    OpeningAuctionPeriod = 9,
    /// Период торгов по цене аукциона закрытия
    TradingAtClosingAuctionPrice = 10,
    /// Сессия назначена
    SessionAssigned = 11,
    /// Сессия закрыта
    SessionClose = 12,
    /// Сессия открыта
    SessionOpen = 13,
    /// Доступна торговля в режиме внутренней ликвидности брокера
    DealerNormalTrading = 14,
    /// Перерыв торговли в режиме внутренней ликвидности брокера
    DealerBreakInTrading = 15,
    /// Недоступна торговля в режиме внутренней ликвидности брокера
    DealerNotAvailableForTrading = 16,
}
impl SecurityTradingStatus {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            SecurityTradingStatus::Unspecified => "SECURITY_TRADING_STATUS_UNSPECIFIED",
            SecurityTradingStatus::NotAvailableForTrading => {
                "SECURITY_TRADING_STATUS_NOT_AVAILABLE_FOR_TRADING"
            }
            SecurityTradingStatus::OpeningPeriod => {
                "SECURITY_TRADING_STATUS_OPENING_PERIOD"
            }
            SecurityTradingStatus::ClosingPeriod => {
                "SECURITY_TRADING_STATUS_CLOSING_PERIOD"
            }
            SecurityTradingStatus::BreakInTrading => {
                "SECURITY_TRADING_STATUS_BREAK_IN_TRADING"
            }
            SecurityTradingStatus::NormalTrading => {
                "SECURITY_TRADING_STATUS_NORMAL_TRADING"
            }
            SecurityTradingStatus::ClosingAuction => {
                "SECURITY_TRADING_STATUS_CLOSING_AUCTION"
            }
            SecurityTradingStatus::DarkPoolAuction => {
                "SECURITY_TRADING_STATUS_DARK_POOL_AUCTION"
            }
            SecurityTradingStatus::DiscreteAuction => {
                "SECURITY_TRADING_STATUS_DISCRETE_AUCTION"
            }
            SecurityTradingStatus::OpeningAuctionPeriod => {
                "SECURITY_TRADING_STATUS_OPENING_AUCTION_PERIOD"
            }
            SecurityTradingStatus::TradingAtClosingAuctionPrice => {
                "SECURITY_TRADING_STATUS_TRADING_AT_CLOSING_AUCTION_PRICE"
            }
            SecurityTradingStatus::SessionAssigned => {
                "SECURITY_TRADING_STATUS_SESSION_ASSIGNED"
            }
            SecurityTradingStatus::SessionClose => {
                "SECURITY_TRADING_STATUS_SESSION_CLOSE"
            }
            SecurityTradingStatus::SessionOpen => "SECURITY_TRADING_STATUS_SESSION_OPEN",
            SecurityTradingStatus::DealerNormalTrading => {
                "SECURITY_TRADING_STATUS_DEALER_NORMAL_TRADING"
            }
            SecurityTradingStatus::DealerBreakInTrading => {
                "SECURITY_TRADING_STATUS_DEALER_BREAK_IN_TRADING"
            }
            SecurityTradingStatus::DealerNotAvailableForTrading => {
                "SECURITY_TRADING_STATUS_DEALER_NOT_AVAILABLE_FOR_TRADING"
            }
        }
    }
}
/// Запрос расписания торгов.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TradingSchedulesRequest {
    /// Наименование биржи или расчетного календаря. </br>Если не передаётся, возвращается информация по всем доступным торговым площадкам.
    #[prost(string, tag = "1")]
    pub exchange: ::prost::alloc::string::String,
    /// Начало периода по часовому поясу UTC.
    #[prost(message, optional, tag = "2")]
    pub from: ::core::option::Option<::prost_types::Timestamp>,
    /// Окончание периода по часовому поясу UTC.
    #[prost(message, optional, tag = "3")]
    pub to: ::core::option::Option<::prost_types::Timestamp>,
}
/// Список торговых площадок.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TradingSchedulesResponse {
    /// Список торговых площадок и режимов торгов.
    #[prost(message, repeated, tag = "1")]
    pub exchanges: ::prost::alloc::vec::Vec<TradingSchedule>,
}
/// Данные по торговой площадке.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TradingSchedule {
    /// Наименование торговой площадки.
    #[prost(string, tag = "1")]
    pub exchange: ::prost::alloc::string::String,
    /// Массив с торговыми и неторговыми днями.
    #[prost(message, repeated, tag = "2")]
    pub days: ::prost::alloc::vec::Vec<TradingDay>,
}
/// Информация о времени торгов.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TradingDay {
    /// Дата.
    #[prost(message, optional, tag = "1")]
    pub date: ::core::option::Option<::prost_types::Timestamp>,
    /// Признак торгового дня на бирже.
    #[prost(bool, tag = "2")]
    pub is_trading_day: bool,
    /// Время начала торгов по часовому поясу UTC.
    #[prost(message, optional, tag = "3")]
    pub start_time: ::core::option::Option<::prost_types::Timestamp>,
    /// Время окончания торгов по часовому поясу UTC.
    #[prost(message, optional, tag = "4")]
    pub end_time: ::core::option::Option<::prost_types::Timestamp>,
    /// Время начала аукциона открытия в часовом поясе UTC.
    #[prost(message, optional, tag = "7")]
    pub opening_auction_start_time: ::core::option::Option<::prost_types::Timestamp>,
    /// Время окончания аукциона закрытия в часовом поясе UTC.
    #[prost(message, optional, tag = "8")]
    pub closing_auction_end_time: ::core::option::Option<::prost_types::Timestamp>,
    /// Время начала аукциона открытия вечерней сессии в часовом поясе UTC.
    #[prost(message, optional, tag = "9")]
    pub evening_opening_auction_start_time: ::core::option::Option<
        ::prost_types::Timestamp,
    >,
    /// Время начала вечерней сессии в часовом поясе UTC.
    #[prost(message, optional, tag = "10")]
    pub evening_start_time: ::core::option::Option<::prost_types::Timestamp>,
    /// Время окончания вечерней сессии в часовом поясе UTC.
    #[prost(message, optional, tag = "11")]
    pub evening_end_time: ::core::option::Option<::prost_types::Timestamp>,
    /// Время начала основного клиринга в часовом поясе UTC.
    #[prost(message, optional, tag = "12")]
    pub clearing_start_time: ::core::option::Option<::prost_types::Timestamp>,
    /// Время окончания основного клиринга в часовом поясе UTC.
    #[prost(message, optional, tag = "13")]
    pub clearing_end_time: ::core::option::Option<::prost_types::Timestamp>,
    /// Время начала премаркета в часовом поясе UTC.
    #[prost(message, optional, tag = "14")]
    pub premarket_start_time: ::core::option::Option<::prost_types::Timestamp>,
    /// Время окончания премаркета в часовом поясе UTC.
    #[prost(message, optional, tag = "15")]
    pub premarket_end_time: ::core::option::Option<::prost_types::Timestamp>,
}
/// Запрос получения инструмента по идентификатору.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InstrumentRequest {
    /// Тип идентификатора инструмента. Возможные значения: figi, ticker. Подробнее об идентификации инструментов: [Идентификация инструментов](<https://tinkoff.github.io/investAPI/faq_identification/>)
    #[prost(enumeration = "InstrumentIdType", tag = "1")]
    pub id_type: i32,
    /// Идентификатор class_code. Обязателен при id_type = ticker.
    #[prost(string, tag = "2")]
    pub class_code: ::prost::alloc::string::String,
    /// Идентификатор запрашиваемого инструмента.
    #[prost(string, tag = "3")]
    pub id: ::prost::alloc::string::String,
}
/// Запрос получения инструментов.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InstrumentsRequest {
    /// Статус запрашиваемых инструментов. Возможные значения: \[InstrumentStatus\](#instrumentstatus)
    #[prost(enumeration = "InstrumentStatus", tag = "1")]
    pub instrument_status: i32,
}
/// Информация об облигации.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BondResponse {
    /// Информация об облигации.
    #[prost(message, optional, tag = "1")]
    pub instrument: ::core::option::Option<Bond>,
}
/// Список облигаций.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct BondsResponse {
    /// Массив облигаций.
    #[prost(message, repeated, tag = "1")]
    pub instruments: ::prost::alloc::vec::Vec<Bond>,
}
/// Запрос купонов по облигации.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetBondCouponsRequest {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Начало запрашиваемого периода в часовом поясе UTC. Фильтрация по coupon_date (дата выплаты купона)
    #[prost(message, optional, tag = "2")]
    pub from: ::core::option::Option<::prost_types::Timestamp>,
    /// Окончание запрашиваемого периода в часовом поясе UTC. Фильтрация по coupon_date (дата выплаты купона)
    #[prost(message, optional, tag = "3")]
    pub to: ::core::option::Option<::prost_types::Timestamp>,
}
/// Купоны по облигации.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetBondCouponsResponse {
    #[prost(message, repeated, tag = "1")]
    pub events: ::prost::alloc::vec::Vec<Coupon>,
}
/// Объект передачи информации о купоне облигации.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Coupon {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Дата выплаты купона.
    #[prost(message, optional, tag = "2")]
    pub coupon_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Номер купона.
    #[prost(int64, tag = "3")]
    pub coupon_number: i64,
    /// (Опционально) Дата фиксации реестра для выплаты купона.
    #[prost(message, optional, tag = "4")]
    pub fix_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Выплата на одну облигацию.
    #[prost(message, optional, tag = "5")]
    pub pay_one_bond: ::core::option::Option<MoneyValue>,
    /// Тип купона.
    #[prost(enumeration = "CouponType", tag = "6")]
    pub coupon_type: i32,
    /// Начало купонного периода.
    #[prost(message, optional, tag = "7")]
    pub coupon_start_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Окончание купонного периода.
    #[prost(message, optional, tag = "8")]
    pub coupon_end_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Купонный период в днях.
    #[prost(int32, tag = "9")]
    pub coupon_period: i32,
}
/// Данные по валюте.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CurrencyResponse {
    /// Информация о валюте.
    #[prost(message, optional, tag = "1")]
    pub instrument: ::core::option::Option<Currency>,
}
/// Данные по валютам.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CurrenciesResponse {
    /// Массив валют.
    #[prost(message, repeated, tag = "1")]
    pub instruments: ::prost::alloc::vec::Vec<Currency>,
}
/// Данные по фонду.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EtfResponse {
    /// Информация о фонде.
    #[prost(message, optional, tag = "1")]
    pub instrument: ::core::option::Option<Etf>,
}
/// Данные по фондам.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EtfsResponse {
    /// Массив фондов.
    #[prost(message, repeated, tag = "1")]
    pub instruments: ::prost::alloc::vec::Vec<Etf>,
}
/// Данные по фьючерсу.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FutureResponse {
    /// Информация о фьючерсу.
    #[prost(message, optional, tag = "1")]
    pub instrument: ::core::option::Option<Future>,
}
/// Данные по фьючерсам.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FuturesResponse {
    /// Массив фьючерсов.
    #[prost(message, repeated, tag = "1")]
    pub instruments: ::prost::alloc::vec::Vec<Future>,
}
/// Данные по опциону.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OptionResponse {
    /// Информация по опциону.
    #[prost(message, optional, tag = "1")]
    pub instrument: ::core::option::Option<Option>,
}
/// Данные по опционам.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OptionsResponse {
    /// Массив данных по опциону.
    #[prost(message, repeated, tag = "1")]
    pub instruments: ::prost::alloc::vec::Vec<Option>,
}
/// Опцион.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Option {
    /// Уникальный идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub uid: ::prost::alloc::string::String,
    /// Уникальный идентификатор позиции.
    #[prost(string, tag = "2")]
    pub position_uid: ::prost::alloc::string::String,
    /// Тикер инструмента.
    #[prost(string, tag = "3")]
    pub ticker: ::prost::alloc::string::String,
    /// Класс-код.
    #[prost(string, tag = "4")]
    pub class_code: ::prost::alloc::string::String,
    /// Уникальный идентификатор позиции основного инструмента.
    #[prost(string, tag = "5")]
    pub basic_asset_position_uid: ::prost::alloc::string::String,
    /// Текущий режим торгов инструмента.
    #[prost(enumeration = "SecurityTradingStatus", tag = "21")]
    pub trading_status: i32,
    /// Реальная площадка исполнения расчётов. Допустимые значения: [REAL_EXCHANGE_MOEX, REAL_EXCHANGE_RTS]
    #[prost(enumeration = "RealExchange", tag = "31")]
    pub real_exchange: i32,
    /// Направление опциона.
    #[prost(enumeration = "OptionDirection", tag = "41")]
    pub direction: i32,
    /// Тип расчетов по опциону.
    #[prost(enumeration = "OptionPaymentType", tag = "42")]
    pub payment_type: i32,
    /// Стиль опциона.
    #[prost(enumeration = "OptionStyle", tag = "43")]
    pub style: i32,
    /// Способ исполнения опциона.
    #[prost(enumeration = "OptionSettlementType", tag = "44")]
    pub settlement_type: i32,
    /// Название инструмента.
    #[prost(string, tag = "101")]
    pub name: ::prost::alloc::string::String,
    /// Валюта.
    #[prost(string, tag = "111")]
    pub currency: ::prost::alloc::string::String,
    /// Валюта, в которой оценивается контракт.
    #[prost(string, tag = "112")]
    pub settlement_currency: ::prost::alloc::string::String,
    /// Тип актива.
    #[prost(string, tag = "131")]
    pub asset_type: ::prost::alloc::string::String,
    /// Основной актив.
    #[prost(string, tag = "132")]
    pub basic_asset: ::prost::alloc::string::String,
    /// Биржа.
    #[prost(string, tag = "141")]
    pub exchange: ::prost::alloc::string::String,
    /// Код страны рисков.
    #[prost(string, tag = "151")]
    pub country_of_risk: ::prost::alloc::string::String,
    /// Наименование страны рисков.
    #[prost(string, tag = "152")]
    pub country_of_risk_name: ::prost::alloc::string::String,
    /// Сектор экономики.
    #[prost(string, tag = "161")]
    pub sector: ::prost::alloc::string::String,
    /// Количество бумаг в лоте.
    #[prost(int32, tag = "201")]
    pub lot: i32,
    /// Размер основного актива.
    #[prost(message, optional, tag = "211")]
    pub basic_asset_size: ::core::option::Option<Quotation>,
    /// Коэффициент ставки риска длинной позиции по клиенту.
    #[prost(message, optional, tag = "221")]
    pub klong: ::core::option::Option<Quotation>,
    /// Коэффициент ставки риска короткой позиции по клиенту.
    #[prost(message, optional, tag = "222")]
    pub kshort: ::core::option::Option<Quotation>,
    /// Ставка риска минимальной маржи лонг.
    #[prost(message, optional, tag = "223")]
    pub dlong: ::core::option::Option<Quotation>,
    /// Ставка риска минимальной маржи шорт.
    #[prost(message, optional, tag = "224")]
    pub dshort: ::core::option::Option<Quotation>,
    /// Ставка риска начальной маржи лонг.
    #[prost(message, optional, tag = "225")]
    pub dlong_min: ::core::option::Option<Quotation>,
    /// Ставка риска начальной маржи шорт.
    #[prost(message, optional, tag = "226")]
    pub dshort_min: ::core::option::Option<Quotation>,
    /// Минимальный шаг цены.
    #[prost(message, optional, tag = "231")]
    pub min_price_increment: ::core::option::Option<Quotation>,
    /// Цена страйка.
    #[prost(message, optional, tag = "241")]
    pub strike_price: ::core::option::Option<MoneyValue>,
    /// Дата истечения срока в формате UTC.
    #[prost(message, optional, tag = "301")]
    pub expiration_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Дата начала обращения контракта в формате UTC.
    #[prost(message, optional, tag = "311")]
    pub first_trade_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Дата исполнения в формате UTC.
    #[prost(message, optional, tag = "312")]
    pub last_trade_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Дата первой минутной свечи в формате UTC.
    #[prost(message, optional, tag = "321")]
    pub first_1min_candle_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Дата первой дневной свечи в формате UTC.
    #[prost(message, optional, tag = "322")]
    pub first_1day_candle_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Признак доступности для операций шорт.
    #[prost(bool, tag = "401")]
    pub short_enabled_flag: bool,
    /// Возможность покупки/продажи на ИИС.
    #[prost(bool, tag = "402")]
    pub for_iis_flag: bool,
    /// Признак внебиржевой ценной бумаги.
    #[prost(bool, tag = "403")]
    pub otc_flag: bool,
    /// Признак доступности для покупки.
    #[prost(bool, tag = "404")]
    pub buy_available_flag: bool,
    /// Признак доступности для продажи.
    #[prost(bool, tag = "405")]
    pub sell_available_flag: bool,
    /// Флаг отображающий доступность торговли инструментом только для квалифицированных инвесторов.
    #[prost(bool, tag = "406")]
    pub for_qual_investor_flag: bool,
    /// Флаг отображающий доступность торговли инструментом по выходным
    #[prost(bool, tag = "407")]
    pub weekend_flag: bool,
    /// Флаг заблокированного ТКС
    #[prost(bool, tag = "408")]
    pub blocked_tca_flag: bool,
}
/// Данные по акции.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ShareResponse {
    /// Информация об акции.
    #[prost(message, optional, tag = "1")]
    pub instrument: ::core::option::Option<Share>,
}
/// Данные по акциям.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SharesResponse {
    /// Массив акций.
    #[prost(message, repeated, tag = "1")]
    pub instruments: ::prost::alloc::vec::Vec<Share>,
}
/// Объект передачи информации об облигации.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Bond {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Тикер инструмента.
    #[prost(string, tag = "2")]
    pub ticker: ::prost::alloc::string::String,
    /// Класс-код (секция торгов).
    #[prost(string, tag = "3")]
    pub class_code: ::prost::alloc::string::String,
    /// Isin-идентификатор инструмента.
    #[prost(string, tag = "4")]
    pub isin: ::prost::alloc::string::String,
    /// Лотность инструмента. Возможно совершение операций только на количества ценной бумаги, кратные параметру *lot*. Подробнее: \[лот\](<https://tinkoff.github.io/investAPI/glossary#lot>)
    #[prost(int32, tag = "5")]
    pub lot: i32,
    /// Валюта расчётов.
    #[prost(string, tag = "6")]
    pub currency: ::prost::alloc::string::String,
    /// Коэффициент ставки риска длинной позиции по инструменту.
    #[prost(message, optional, tag = "7")]
    pub klong: ::core::option::Option<Quotation>,
    /// Коэффициент ставки риска короткой позиции по инструменту.
    #[prost(message, optional, tag = "8")]
    pub kshort: ::core::option::Option<Quotation>,
    /// Ставка риска минимальной маржи в лонг. Подробнее: [ставка риска в лонг](<https://help.tinkoff.ru/margin-trade/long/risk-rate/>)
    #[prost(message, optional, tag = "9")]
    pub dlong: ::core::option::Option<Quotation>,
    /// Ставка риска минимальной маржи в шорт. Подробнее: [ставка риска в шорт](<https://help.tinkoff.ru/margin-trade/short/risk-rate/>)
    #[prost(message, optional, tag = "10")]
    pub dshort: ::core::option::Option<Quotation>,
    /// Ставка риска начальной маржи в лонг. Подробнее: [ставка риска в лонг](<https://help.tinkoff.ru/margin-trade/long/risk-rate/>)
    #[prost(message, optional, tag = "11")]
    pub dlong_min: ::core::option::Option<Quotation>,
    /// Ставка риска начальной маржи в шорт. Подробнее: [ставка риска в шорт](<https://help.tinkoff.ru/margin-trade/short/risk-rate/>)
    #[prost(message, optional, tag = "12")]
    pub dshort_min: ::core::option::Option<Quotation>,
    /// Признак доступности для операций в шорт.
    #[prost(bool, tag = "13")]
    pub short_enabled_flag: bool,
    /// Название инструмента.
    #[prost(string, tag = "15")]
    pub name: ::prost::alloc::string::String,
    /// Торговая площадка.
    #[prost(string, tag = "16")]
    pub exchange: ::prost::alloc::string::String,
    /// Количество выплат по купонам в год.
    #[prost(int32, tag = "17")]
    pub coupon_quantity_per_year: i32,
    /// Дата погашения облигации в часовом поясе UTC.
    #[prost(message, optional, tag = "18")]
    pub maturity_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Номинал облигации.
    #[prost(message, optional, tag = "19")]
    pub nominal: ::core::option::Option<MoneyValue>,
    /// Первоначальный номинал облигации.
    #[prost(message, optional, tag = "20")]
    pub initial_nominal: ::core::option::Option<MoneyValue>,
    /// Дата выпуска облигации в часовом поясе UTC.
    #[prost(message, optional, tag = "21")]
    pub state_reg_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Дата размещения в часовом поясе UTC.
    #[prost(message, optional, tag = "22")]
    pub placement_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Цена размещения.
    #[prost(message, optional, tag = "23")]
    pub placement_price: ::core::option::Option<MoneyValue>,
    /// Значение НКД (накопленного купонного дохода) на дату.
    #[prost(message, optional, tag = "24")]
    pub aci_value: ::core::option::Option<MoneyValue>,
    /// Код страны риска, т.е. страны, в которой компания ведёт основной бизнес.
    #[prost(string, tag = "25")]
    pub country_of_risk: ::prost::alloc::string::String,
    /// Наименование страны риска, т.е. страны, в которой компания ведёт основной бизнес.
    #[prost(string, tag = "26")]
    pub country_of_risk_name: ::prost::alloc::string::String,
    /// Сектор экономики.
    #[prost(string, tag = "27")]
    pub sector: ::prost::alloc::string::String,
    /// Форма выпуска. Возможные значения: </br>**documentary** — документарная; </br>**non_documentary** — бездокументарная.
    #[prost(string, tag = "28")]
    pub issue_kind: ::prost::alloc::string::String,
    /// Размер выпуска.
    #[prost(int64, tag = "29")]
    pub issue_size: i64,
    /// Плановый размер выпуска.
    #[prost(int64, tag = "30")]
    pub issue_size_plan: i64,
    /// Текущий режим торгов инструмента.
    #[prost(enumeration = "SecurityTradingStatus", tag = "31")]
    pub trading_status: i32,
    /// Признак внебиржевой ценной бумаги.
    #[prost(bool, tag = "32")]
    pub otc_flag: bool,
    /// Признак доступности для покупки.
    #[prost(bool, tag = "33")]
    pub buy_available_flag: bool,
    /// Признак доступности для продажи.
    #[prost(bool, tag = "34")]
    pub sell_available_flag: bool,
    /// Признак облигации с плавающим купоном.
    #[prost(bool, tag = "35")]
    pub floating_coupon_flag: bool,
    /// Признак бессрочной облигации.
    #[prost(bool, tag = "36")]
    pub perpetual_flag: bool,
    /// Признак облигации с амортизацией долга.
    #[prost(bool, tag = "37")]
    pub amortization_flag: bool,
    /// Шаг цены.
    #[prost(message, optional, tag = "38")]
    pub min_price_increment: ::core::option::Option<Quotation>,
    /// Параметр указывает на возможность торговать инструментом через API.
    #[prost(bool, tag = "39")]
    pub api_trade_available_flag: bool,
    /// Уникальный идентификатор инструмента.
    #[prost(string, tag = "40")]
    pub uid: ::prost::alloc::string::String,
    /// Реальная площадка исполнения расчётов.
    #[prost(enumeration = "RealExchange", tag = "41")]
    pub real_exchange: i32,
    /// Уникальный идентификатор позиции инструмента.
    #[prost(string, tag = "42")]
    pub position_uid: ::prost::alloc::string::String,
    /// Признак доступности для ИИС.
    #[prost(bool, tag = "51")]
    pub for_iis_flag: bool,
    /// Флаг отображающий доступность торговли инструментом только для квалифицированных инвесторов.
    #[prost(bool, tag = "52")]
    pub for_qual_investor_flag: bool,
    /// Флаг отображающий доступность торговли инструментом по выходным
    #[prost(bool, tag = "53")]
    pub weekend_flag: bool,
    /// Флаг заблокированного ТКС
    #[prost(bool, tag = "54")]
    pub blocked_tca_flag: bool,
    /// Дата первой минутной свечи.
    #[prost(message, optional, tag = "61")]
    pub first_1min_candle_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Дата первой дневной свечи.
    #[prost(message, optional, tag = "62")]
    pub first_1day_candle_date: ::core::option::Option<::prost_types::Timestamp>,
}
/// Объект передачи информации о валюте.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Currency {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Тикер инструмента.
    #[prost(string, tag = "2")]
    pub ticker: ::prost::alloc::string::String,
    /// Класс-код (секция торгов).
    #[prost(string, tag = "3")]
    pub class_code: ::prost::alloc::string::String,
    /// Isin-идентификатор инструмента.
    #[prost(string, tag = "4")]
    pub isin: ::prost::alloc::string::String,
    /// Лотность инструмента. Возможно совершение операций только на количества ценной бумаги, кратные параметру *lot*. Подробнее: \[лот\](<https://tinkoff.github.io/investAPI/glossary#lot>)
    #[prost(int32, tag = "5")]
    pub lot: i32,
    /// Валюта расчётов.
    #[prost(string, tag = "6")]
    pub currency: ::prost::alloc::string::String,
    /// Коэффициент ставки риска длинной позиции по инструменту.
    #[prost(message, optional, tag = "7")]
    pub klong: ::core::option::Option<Quotation>,
    /// Коэффициент ставки риска короткой позиции по инструменту.
    #[prost(message, optional, tag = "8")]
    pub kshort: ::core::option::Option<Quotation>,
    /// Ставка риска минимальной маржи в лонг. Подробнее: [ставка риска в лонг](<https://help.tinkoff.ru/margin-trade/long/risk-rate/>)
    #[prost(message, optional, tag = "9")]
    pub dlong: ::core::option::Option<Quotation>,
    /// Ставка риска минимальной маржи в шорт. Подробнее: [ставка риска в шорт](<https://help.tinkoff.ru/margin-trade/short/risk-rate/>)
    #[prost(message, optional, tag = "10")]
    pub dshort: ::core::option::Option<Quotation>,
    /// Ставка риска начальной маржи в лонг. Подробнее: [ставка риска в лонг](<https://help.tinkoff.ru/margin-trade/long/risk-rate/>)
    #[prost(message, optional, tag = "11")]
    pub dlong_min: ::core::option::Option<Quotation>,
    /// Ставка риска начальной маржи в шорт. Подробнее: [ставка риска в шорт](<https://help.tinkoff.ru/margin-trade/short/risk-rate/>)
    #[prost(message, optional, tag = "12")]
    pub dshort_min: ::core::option::Option<Quotation>,
    /// Признак доступности для операций в шорт.
    #[prost(bool, tag = "13")]
    pub short_enabled_flag: bool,
    /// Название инструмента.
    #[prost(string, tag = "15")]
    pub name: ::prost::alloc::string::String,
    /// Торговая площадка.
    #[prost(string, tag = "16")]
    pub exchange: ::prost::alloc::string::String,
    /// Номинал.
    #[prost(message, optional, tag = "17")]
    pub nominal: ::core::option::Option<MoneyValue>,
    /// Код страны риска, т.е. страны, в которой компания ведёт основной бизнес.
    #[prost(string, tag = "18")]
    pub country_of_risk: ::prost::alloc::string::String,
    /// Наименование страны риска, т.е. страны, в которой компания ведёт основной бизнес.
    #[prost(string, tag = "19")]
    pub country_of_risk_name: ::prost::alloc::string::String,
    /// Текущий режим торгов инструмента.
    #[prost(enumeration = "SecurityTradingStatus", tag = "20")]
    pub trading_status: i32,
    /// Признак внебиржевой ценной бумаги.
    #[prost(bool, tag = "21")]
    pub otc_flag: bool,
    /// Признак доступности для покупки.
    #[prost(bool, tag = "22")]
    pub buy_available_flag: bool,
    /// Признак доступности для продажи.
    #[prost(bool, tag = "23")]
    pub sell_available_flag: bool,
    /// Строковый ISO-код валюты.
    #[prost(string, tag = "24")]
    pub iso_currency_name: ::prost::alloc::string::String,
    /// Шаг цены.
    #[prost(message, optional, tag = "25")]
    pub min_price_increment: ::core::option::Option<Quotation>,
    /// Параметр указывает на возможность торговать инструментом через API.
    #[prost(bool, tag = "26")]
    pub api_trade_available_flag: bool,
    /// Уникальный идентификатор инструмента.
    #[prost(string, tag = "27")]
    pub uid: ::prost::alloc::string::String,
    /// Реальная площадка исполнения расчётов.
    #[prost(enumeration = "RealExchange", tag = "28")]
    pub real_exchange: i32,
    /// Уникальный идентификатор позиции инструмента.
    #[prost(string, tag = "29")]
    pub position_uid: ::prost::alloc::string::String,
    /// Признак доступности для ИИС.
    #[prost(bool, tag = "41")]
    pub for_iis_flag: bool,
    /// Флаг отображающий доступность торговли инструментом только для квалифицированных инвесторов.
    #[prost(bool, tag = "52")]
    pub for_qual_investor_flag: bool,
    /// Флаг отображающий доступность торговли инструментом по выходным
    #[prost(bool, tag = "53")]
    pub weekend_flag: bool,
    /// Флаг заблокированного ТКС
    #[prost(bool, tag = "54")]
    pub blocked_tca_flag: bool,
    /// Дата первой минутной свечи.
    #[prost(message, optional, tag = "56")]
    pub first_1min_candle_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Дата первой дневной свечи.
    #[prost(message, optional, tag = "57")]
    pub first_1day_candle_date: ::core::option::Option<::prost_types::Timestamp>,
}
/// Объект передачи информации об инвестиционном фонде.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Etf {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Тикер инструмента.
    #[prost(string, tag = "2")]
    pub ticker: ::prost::alloc::string::String,
    /// Класс-код (секция торгов).
    #[prost(string, tag = "3")]
    pub class_code: ::prost::alloc::string::String,
    /// Isin-идентификатор инструмента.
    #[prost(string, tag = "4")]
    pub isin: ::prost::alloc::string::String,
    /// Лотность инструмента. Возможно совершение операций только на количества ценной бумаги, кратные параметру *lot*. Подробнее: \[лот\](<https://tinkoff.github.io/investAPI/glossary#lot>)
    #[prost(int32, tag = "5")]
    pub lot: i32,
    /// Валюта расчётов.
    #[prost(string, tag = "6")]
    pub currency: ::prost::alloc::string::String,
    /// Коэффициент ставки риска длинной позиции по инструменту.
    #[prost(message, optional, tag = "7")]
    pub klong: ::core::option::Option<Quotation>,
    /// Коэффициент ставки риска короткой позиции по инструменту.
    #[prost(message, optional, tag = "8")]
    pub kshort: ::core::option::Option<Quotation>,
    /// Ставка риска минимальной маржи в лонг. Подробнее: [ставка риска в лонг](<https://help.tinkoff.ru/margin-trade/long/risk-rate/>)
    #[prost(message, optional, tag = "9")]
    pub dlong: ::core::option::Option<Quotation>,
    /// Ставка риска минимальной маржи в шорт. Подробнее: [ставка риска в шорт](<https://help.tinkoff.ru/margin-trade/short/risk-rate/>)
    #[prost(message, optional, tag = "10")]
    pub dshort: ::core::option::Option<Quotation>,
    /// Ставка риска начальной маржи в лонг. Подробнее: [ставка риска в лонг](<https://help.tinkoff.ru/margin-trade/long/risk-rate/>)
    #[prost(message, optional, tag = "11")]
    pub dlong_min: ::core::option::Option<Quotation>,
    /// Ставка риска начальной маржи в шорт. Подробнее: [ставка риска в шорт](<https://help.tinkoff.ru/margin-trade/short/risk-rate/>)
    #[prost(message, optional, tag = "12")]
    pub dshort_min: ::core::option::Option<Quotation>,
    /// Признак доступности для операций в шорт.
    #[prost(bool, tag = "13")]
    pub short_enabled_flag: bool,
    /// Название инструмента.
    #[prost(string, tag = "15")]
    pub name: ::prost::alloc::string::String,
    /// Торговая площадка.
    #[prost(string, tag = "16")]
    pub exchange: ::prost::alloc::string::String,
    /// Размер фиксированной комиссии фонда.
    #[prost(message, optional, tag = "17")]
    pub fixed_commission: ::core::option::Option<Quotation>,
    /// Возможные значения: </br>**equity** — акции;</br>**fixed_income** — облигации;</br>**mixed_allocation** — смешанный;</br>**money_market** — денежный рынок;</br>**real_estate** — недвижимость;</br>**commodity** — товары;</br>**specialty** — специальный;</br>**private_equity** — private equity;</br>**alternative_investment** — альтернативные инвестиции.
    #[prost(string, tag = "18")]
    pub focus_type: ::prost::alloc::string::String,
    /// Дата выпуска в часовом поясе UTC.
    #[prost(message, optional, tag = "19")]
    pub released_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Количество акций фонда в обращении.
    #[prost(message, optional, tag = "20")]
    pub num_shares: ::core::option::Option<Quotation>,
    /// Код страны риска, т.е. страны, в которой компания ведёт основной бизнес.
    #[prost(string, tag = "21")]
    pub country_of_risk: ::prost::alloc::string::String,
    /// Наименование страны риска, т.е. страны, в которой компания ведёт основной бизнес.
    #[prost(string, tag = "22")]
    pub country_of_risk_name: ::prost::alloc::string::String,
    /// Сектор экономики.
    #[prost(string, tag = "23")]
    pub sector: ::prost::alloc::string::String,
    /// Частота ребалансировки.
    #[prost(string, tag = "24")]
    pub rebalancing_freq: ::prost::alloc::string::String,
    /// Текущий режим торгов инструмента.
    #[prost(enumeration = "SecurityTradingStatus", tag = "25")]
    pub trading_status: i32,
    /// Признак внебиржевой ценной бумаги.
    #[prost(bool, tag = "26")]
    pub otc_flag: bool,
    /// Признак доступности для покупки.
    #[prost(bool, tag = "27")]
    pub buy_available_flag: bool,
    /// Признак доступности для продажи.
    #[prost(bool, tag = "28")]
    pub sell_available_flag: bool,
    /// Шаг цены.
    #[prost(message, optional, tag = "29")]
    pub min_price_increment: ::core::option::Option<Quotation>,
    /// Параметр указывает на возможность торговать инструментом через API.
    #[prost(bool, tag = "30")]
    pub api_trade_available_flag: bool,
    /// Уникальный идентификатор инструмента.
    #[prost(string, tag = "31")]
    pub uid: ::prost::alloc::string::String,
    /// Реальная площадка исполнения расчётов.
    #[prost(enumeration = "RealExchange", tag = "32")]
    pub real_exchange: i32,
    /// Уникальный идентификатор позиции инструмента.
    #[prost(string, tag = "33")]
    pub position_uid: ::prost::alloc::string::String,
    /// Признак доступности для ИИС.
    #[prost(bool, tag = "41")]
    pub for_iis_flag: bool,
    /// Флаг отображающий доступность торговли инструментом только для квалифицированных инвесторов.
    #[prost(bool, tag = "42")]
    pub for_qual_investor_flag: bool,
    /// Флаг отображающий доступность торговли инструментом по выходным
    #[prost(bool, tag = "43")]
    pub weekend_flag: bool,
    /// Флаг заблокированного ТКС
    #[prost(bool, tag = "44")]
    pub blocked_tca_flag: bool,
    /// Дата первой минутной свечи.
    #[prost(message, optional, tag = "56")]
    pub first_1min_candle_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Дата первой дневной свечи.
    #[prost(message, optional, tag = "57")]
    pub first_1day_candle_date: ::core::option::Option<::prost_types::Timestamp>,
}
/// Объект передачи информации о фьючерсе.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Future {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Тикер инструмента.
    #[prost(string, tag = "2")]
    pub ticker: ::prost::alloc::string::String,
    /// Класс-код (секция торгов).
    #[prost(string, tag = "3")]
    pub class_code: ::prost::alloc::string::String,
    /// Лотность инструмента. Возможно совершение операций только на количества ценной бумаги, кратные параметру *lot*. Подробнее: \[лот\](<https://tinkoff.github.io/investAPI/glossary#lot>)
    #[prost(int32, tag = "4")]
    pub lot: i32,
    /// Валюта расчётов.
    #[prost(string, tag = "5")]
    pub currency: ::prost::alloc::string::String,
    /// Коэффициент ставки риска длинной позиции по клиенту.
    #[prost(message, optional, tag = "6")]
    pub klong: ::core::option::Option<Quotation>,
    /// Коэффициент ставки риска короткой позиции по клиенту.
    #[prost(message, optional, tag = "7")]
    pub kshort: ::core::option::Option<Quotation>,
    /// Ставка риска минимальной маржи в лонг. Подробнее: [ставка риска в лонг](<https://help.tinkoff.ru/margin-trade/long/risk-rate/>)
    #[prost(message, optional, tag = "8")]
    pub dlong: ::core::option::Option<Quotation>,
    /// Ставка риска минимальной маржи в шорт. Подробнее: [ставка риска в шорт](<https://help.tinkoff.ru/margin-trade/short/risk-rate/>)
    #[prost(message, optional, tag = "9")]
    pub dshort: ::core::option::Option<Quotation>,
    /// Ставка риска начальной маржи в лонг. Подробнее: [ставка риска в лонг](<https://help.tinkoff.ru/margin-trade/long/risk-rate/>)
    #[prost(message, optional, tag = "10")]
    pub dlong_min: ::core::option::Option<Quotation>,
    /// Ставка риска начальной маржи в шорт. Подробнее: [ставка риска в шорт](<https://help.tinkoff.ru/margin-trade/short/risk-rate/>)
    #[prost(message, optional, tag = "11")]
    pub dshort_min: ::core::option::Option<Quotation>,
    /// Признак доступности для операций шорт.
    #[prost(bool, tag = "12")]
    pub short_enabled_flag: bool,
    /// Название инструмента.
    #[prost(string, tag = "13")]
    pub name: ::prost::alloc::string::String,
    /// Торговая площадка.
    #[prost(string, tag = "14")]
    pub exchange: ::prost::alloc::string::String,
    /// Дата начала обращения контракта в часовом поясе UTC.
    #[prost(message, optional, tag = "15")]
    pub first_trade_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Дата в часовом поясе UTC, до которой возможно проведение операций с фьючерсом.
    #[prost(message, optional, tag = "16")]
    pub last_trade_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Тип фьючерса. Возможные значения: </br>**physical_delivery** — физические поставки; </br>**cash_settlement** — денежный эквивалент.
    #[prost(string, tag = "17")]
    pub futures_type: ::prost::alloc::string::String,
    /// Тип актива. Возможные значения: </br>**commodity** — товар; </br>**currency** — валюта; </br>**security** — ценная бумага; </br>**index** — индекс.
    #[prost(string, tag = "18")]
    pub asset_type: ::prost::alloc::string::String,
    /// Основной актив.
    #[prost(string, tag = "19")]
    pub basic_asset: ::prost::alloc::string::String,
    /// Размер основного актива.
    #[prost(message, optional, tag = "20")]
    pub basic_asset_size: ::core::option::Option<Quotation>,
    /// Код страны риска, т.е. страны, в которой компания ведёт основной бизнес.
    #[prost(string, tag = "21")]
    pub country_of_risk: ::prost::alloc::string::String,
    /// Наименование страны риска, т.е. страны, в которой компания ведёт основной бизнес.
    #[prost(string, tag = "22")]
    pub country_of_risk_name: ::prost::alloc::string::String,
    /// Сектор экономики.
    #[prost(string, tag = "23")]
    pub sector: ::prost::alloc::string::String,
    /// Дата истечения срока в часов поясе UTC.
    #[prost(message, optional, tag = "24")]
    pub expiration_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Текущий режим торгов инструмента.
    #[prost(enumeration = "SecurityTradingStatus", tag = "25")]
    pub trading_status: i32,
    /// Признак внебиржевой ценной бумаги.
    #[prost(bool, tag = "26")]
    pub otc_flag: bool,
    /// Признак доступности для покупки.
    #[prost(bool, tag = "27")]
    pub buy_available_flag: bool,
    /// Признак доступности для продажи.
    #[prost(bool, tag = "28")]
    pub sell_available_flag: bool,
    /// Шаг цены.
    #[prost(message, optional, tag = "29")]
    pub min_price_increment: ::core::option::Option<Quotation>,
    /// Параметр указывает на возможность торговать инструментом через API.
    #[prost(bool, tag = "30")]
    pub api_trade_available_flag: bool,
    /// Уникальный идентификатор инструмента.
    #[prost(string, tag = "31")]
    pub uid: ::prost::alloc::string::String,
    /// Реальная площадка исполнения расчётов.
    #[prost(enumeration = "RealExchange", tag = "32")]
    pub real_exchange: i32,
    /// Уникальный идентификатор позиции инструмента.
    #[prost(string, tag = "33")]
    pub position_uid: ::prost::alloc::string::String,
    /// Уникальный идентификатор позиции основного инструмента.
    #[prost(string, tag = "34")]
    pub basic_asset_position_uid: ::prost::alloc::string::String,
    /// Признак доступности для ИИС.
    #[prost(bool, tag = "41")]
    pub for_iis_flag: bool,
    /// Флаг отображающий доступность торговли инструментом только для квалифицированных инвесторов.
    #[prost(bool, tag = "42")]
    pub for_qual_investor_flag: bool,
    /// Флаг отображающий доступность торговли инструментом по выходным
    #[prost(bool, tag = "43")]
    pub weekend_flag: bool,
    /// Флаг заблокированного ТКС
    #[prost(bool, tag = "44")]
    pub blocked_tca_flag: bool,
    /// Дата первой минутной свечи.
    #[prost(message, optional, tag = "56")]
    pub first_1min_candle_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Дата первой дневной свечи.
    #[prost(message, optional, tag = "57")]
    pub first_1day_candle_date: ::core::option::Option<::prost_types::Timestamp>,
}
/// Объект передачи информации об акции.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Share {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Тикер инструмента.
    #[prost(string, tag = "2")]
    pub ticker: ::prost::alloc::string::String,
    /// Класс-код (секция торгов).
    #[prost(string, tag = "3")]
    pub class_code: ::prost::alloc::string::String,
    /// Isin-идентификатор инструмента.
    #[prost(string, tag = "4")]
    pub isin: ::prost::alloc::string::String,
    /// Лотность инструмента. Возможно совершение операций только на количества ценной бумаги, кратные параметру *lot*. Подробнее: \[лот\](<https://tinkoff.github.io/investAPI/glossary#lot>)
    #[prost(int32, tag = "5")]
    pub lot: i32,
    /// Валюта расчётов.
    #[prost(string, tag = "6")]
    pub currency: ::prost::alloc::string::String,
    /// Коэффициент ставки риска длинной позиции по инструменту.
    #[prost(message, optional, tag = "7")]
    pub klong: ::core::option::Option<Quotation>,
    /// Коэффициент ставки риска короткой позиции по инструменту.
    #[prost(message, optional, tag = "8")]
    pub kshort: ::core::option::Option<Quotation>,
    /// Ставка риска минимальной маржи в лонг. Подробнее: [ставка риска в лонг](<https://help.tinkoff.ru/margin-trade/long/risk-rate/>)
    #[prost(message, optional, tag = "9")]
    pub dlong: ::core::option::Option<Quotation>,
    /// Ставка риска минимальной маржи в шорт. Подробнее: [ставка риска в шорт](<https://help.tinkoff.ru/margin-trade/short/risk-rate/>)
    #[prost(message, optional, tag = "10")]
    pub dshort: ::core::option::Option<Quotation>,
    /// Ставка риска начальной маржи в лонг. Подробнее: [ставка риска в лонг](<https://help.tinkoff.ru/margin-trade/long/risk-rate/>)
    #[prost(message, optional, tag = "11")]
    pub dlong_min: ::core::option::Option<Quotation>,
    /// Ставка риска начальной маржи в шорт. Подробнее: [ставка риска в шорт](<https://help.tinkoff.ru/margin-trade/short/risk-rate/>)
    #[prost(message, optional, tag = "12")]
    pub dshort_min: ::core::option::Option<Quotation>,
    /// Признак доступности для операций в шорт.
    #[prost(bool, tag = "13")]
    pub short_enabled_flag: bool,
    /// Название инструмента.
    #[prost(string, tag = "15")]
    pub name: ::prost::alloc::string::String,
    /// Торговая площадка.
    #[prost(string, tag = "16")]
    pub exchange: ::prost::alloc::string::String,
    /// Дата IPO акции в часовом поясе UTC.
    #[prost(message, optional, tag = "17")]
    pub ipo_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Размер выпуска.
    #[prost(int64, tag = "18")]
    pub issue_size: i64,
    /// Код страны риска, т.е. страны, в которой компания ведёт основной бизнес.
    #[prost(string, tag = "19")]
    pub country_of_risk: ::prost::alloc::string::String,
    /// Наименование страны риска, т.е. страны, в которой компания ведёт основной бизнес.
    #[prost(string, tag = "20")]
    pub country_of_risk_name: ::prost::alloc::string::String,
    /// Сектор экономики.
    #[prost(string, tag = "21")]
    pub sector: ::prost::alloc::string::String,
    /// Плановый размер выпуска.
    #[prost(int64, tag = "22")]
    pub issue_size_plan: i64,
    /// Номинал.
    #[prost(message, optional, tag = "23")]
    pub nominal: ::core::option::Option<MoneyValue>,
    /// Текущий режим торгов инструмента.
    #[prost(enumeration = "SecurityTradingStatus", tag = "25")]
    pub trading_status: i32,
    /// Признак внебиржевой ценной бумаги.
    #[prost(bool, tag = "26")]
    pub otc_flag: bool,
    /// Признак доступности для покупки.
    #[prost(bool, tag = "27")]
    pub buy_available_flag: bool,
    /// Признак доступности для продажи.
    #[prost(bool, tag = "28")]
    pub sell_available_flag: bool,
    /// Признак наличия дивидендной доходности.
    #[prost(bool, tag = "29")]
    pub div_yield_flag: bool,
    /// Тип акции. Возможные значения: \[ShareType\](<https://tinkoff.github.io/investAPI/instruments#sharetype>)
    #[prost(enumeration = "ShareType", tag = "30")]
    pub share_type: i32,
    /// Шаг цены.
    #[prost(message, optional, tag = "31")]
    pub min_price_increment: ::core::option::Option<Quotation>,
    /// Параметр указывает на возможность торговать инструментом через API.
    #[prost(bool, tag = "32")]
    pub api_trade_available_flag: bool,
    /// Уникальный идентификатор инструмента.
    #[prost(string, tag = "33")]
    pub uid: ::prost::alloc::string::String,
    /// Реальная площадка исполнения расчётов.
    #[prost(enumeration = "RealExchange", tag = "34")]
    pub real_exchange: i32,
    /// Уникальный идентификатор позиции инструмента.
    #[prost(string, tag = "35")]
    pub position_uid: ::prost::alloc::string::String,
    /// Признак доступности для ИИС.
    #[prost(bool, tag = "46")]
    pub for_iis_flag: bool,
    /// Флаг отображающий доступность торговли инструментом только для квалифицированных инвесторов.
    #[prost(bool, tag = "47")]
    pub for_qual_investor_flag: bool,
    /// Флаг отображающий доступность торговли инструментом по выходным
    #[prost(bool, tag = "48")]
    pub weekend_flag: bool,
    /// Флаг заблокированного ТКС
    #[prost(bool, tag = "49")]
    pub blocked_tca_flag: bool,
    /// Дата первой минутной свечи.
    #[prost(message, optional, tag = "56")]
    pub first_1min_candle_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Дата первой дневной свечи.
    #[prost(message, optional, tag = "57")]
    pub first_1day_candle_date: ::core::option::Option<::prost_types::Timestamp>,
}
/// Запрос НКД по облигации
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetAccruedInterestsRequest {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Начало запрашиваемого периода в часовом поясе UTC.
    #[prost(message, optional, tag = "2")]
    pub from: ::core::option::Option<::prost_types::Timestamp>,
    /// Окончание запрашиваемого периода в часовом поясе UTC.
    #[prost(message, optional, tag = "3")]
    pub to: ::core::option::Option<::prost_types::Timestamp>,
}
/// НКД облигации
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetAccruedInterestsResponse {
    /// Массив операций начисления купонов.
    #[prost(message, repeated, tag = "1")]
    pub accrued_interests: ::prost::alloc::vec::Vec<AccruedInterest>,
}
/// Операция начисления купонов.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AccruedInterest {
    /// Дата и время выплаты в часовом поясе UTC.
    #[prost(message, optional, tag = "1")]
    pub date: ::core::option::Option<::prost_types::Timestamp>,
    /// Величина выплаты.
    #[prost(message, optional, tag = "2")]
    pub value: ::core::option::Option<Quotation>,
    /// Величина выплаты в процентах от номинала.
    #[prost(message, optional, tag = "3")]
    pub value_percent: ::core::option::Option<Quotation>,
    /// Номинал облигации.
    #[prost(message, optional, tag = "4")]
    pub nominal: ::core::option::Option<Quotation>,
}
/// Запрос информации о фьючерсе
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetFuturesMarginRequest {
    /// Идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
}
/// Данные по фьючерсу
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetFuturesMarginResponse {
    /// Гарантийное обеспечение при покупке.
    #[prost(message, optional, tag = "1")]
    pub initial_margin_on_buy: ::core::option::Option<MoneyValue>,
    /// Гарантийное обеспечение при продаже.
    #[prost(message, optional, tag = "2")]
    pub initial_margin_on_sell: ::core::option::Option<MoneyValue>,
    /// Шаг цены.
    #[prost(message, optional, tag = "3")]
    pub min_price_increment: ::core::option::Option<Quotation>,
    /// Стоимость шага цены.
    #[prost(message, optional, tag = "4")]
    pub min_price_increment_amount: ::core::option::Option<Quotation>,
}
/// Данные по инструменту.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InstrumentResponse {
    /// Основная информация об инструменте.
    #[prost(message, optional, tag = "1")]
    pub instrument: ::core::option::Option<Instrument>,
}
/// Объект передачи основной информации об инструменте.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Instrument {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Тикер инструмента.
    #[prost(string, tag = "2")]
    pub ticker: ::prost::alloc::string::String,
    /// Класс-код инструмента.
    #[prost(string, tag = "3")]
    pub class_code: ::prost::alloc::string::String,
    /// Isin-идентификатор инструмента.
    #[prost(string, tag = "4")]
    pub isin: ::prost::alloc::string::String,
    /// Лотность инструмента. Возможно совершение операций только на количества ценной бумаги, кратные параметру *lot*. Подробнее: \[лот\](<https://tinkoff.github.io/investAPI/glossary#lot>)
    #[prost(int32, tag = "5")]
    pub lot: i32,
    /// Валюта расчётов.
    #[prost(string, tag = "6")]
    pub currency: ::prost::alloc::string::String,
    /// Коэффициент ставки риска длинной позиции по инструменту.
    #[prost(message, optional, tag = "7")]
    pub klong: ::core::option::Option<Quotation>,
    /// Коэффициент ставки риска короткой позиции по инструменту.
    #[prost(message, optional, tag = "8")]
    pub kshort: ::core::option::Option<Quotation>,
    /// Ставка риска минимальной маржи в лонг. Подробнее: [ставка риска в лонг](<https://help.tinkoff.ru/margin-trade/long/risk-rate/>)
    #[prost(message, optional, tag = "9")]
    pub dlong: ::core::option::Option<Quotation>,
    /// Ставка риска минимальной маржи в шорт. Подробнее: [ставка риска в шорт](<https://help.tinkoff.ru/margin-trade/short/risk-rate/>)
    #[prost(message, optional, tag = "10")]
    pub dshort: ::core::option::Option<Quotation>,
    /// Ставка риска начальной маржи в лонг. Подробнее: [ставка риска в лонг](<https://help.tinkoff.ru/margin-trade/long/risk-rate/>)
    #[prost(message, optional, tag = "11")]
    pub dlong_min: ::core::option::Option<Quotation>,
    /// Ставка риска начальной маржи в шорт. Подробнее: [ставка риска в шорт](<https://help.tinkoff.ru/margin-trade/short/risk-rate/>)
    #[prost(message, optional, tag = "12")]
    pub dshort_min: ::core::option::Option<Quotation>,
    /// Признак доступности для операций в шорт.
    #[prost(bool, tag = "13")]
    pub short_enabled_flag: bool,
    /// Название инструмента.
    #[prost(string, tag = "14")]
    pub name: ::prost::alloc::string::String,
    /// Торговая площадка.
    #[prost(string, tag = "15")]
    pub exchange: ::prost::alloc::string::String,
    /// Код страны риска, т.е. страны, в которой компания ведёт основной бизнес.
    #[prost(string, tag = "16")]
    pub country_of_risk: ::prost::alloc::string::String,
    /// Наименование страны риска, т.е. страны, в которой компания ведёт основной бизнес.
    #[prost(string, tag = "17")]
    pub country_of_risk_name: ::prost::alloc::string::String,
    /// Тип инструмента.
    #[prost(string, tag = "18")]
    pub instrument_type: ::prost::alloc::string::String,
    /// Текущий режим торгов инструмента.
    #[prost(enumeration = "SecurityTradingStatus", tag = "19")]
    pub trading_status: i32,
    /// Признак внебиржевой ценной бумаги.
    #[prost(bool, tag = "20")]
    pub otc_flag: bool,
    /// Признак доступности для покупки.
    #[prost(bool, tag = "21")]
    pub buy_available_flag: bool,
    /// Признак доступности для продажи.
    #[prost(bool, tag = "22")]
    pub sell_available_flag: bool,
    /// Шаг цены.
    #[prost(message, optional, tag = "23")]
    pub min_price_increment: ::core::option::Option<Quotation>,
    /// Параметр указывает на возможность торговать инструментом через API.
    #[prost(bool, tag = "24")]
    pub api_trade_available_flag: bool,
    /// Уникальный идентификатор инструмента.
    #[prost(string, tag = "25")]
    pub uid: ::prost::alloc::string::String,
    /// Реальная площадка исполнения расчётов.
    #[prost(enumeration = "RealExchange", tag = "26")]
    pub real_exchange: i32,
    /// Уникальный идентификатор позиции инструмента.
    #[prost(string, tag = "27")]
    pub position_uid: ::prost::alloc::string::String,
    /// Признак доступности для ИИС.
    #[prost(bool, tag = "36")]
    pub for_iis_flag: bool,
    /// Флаг отображающий доступность торговли инструментом только для квалифицированных инвесторов.
    #[prost(bool, tag = "37")]
    pub for_qual_investor_flag: bool,
    /// Флаг отображающий доступность торговли инструментом по выходным
    #[prost(bool, tag = "38")]
    pub weekend_flag: bool,
    /// Флаг заблокированного ТКС
    #[prost(bool, tag = "39")]
    pub blocked_tca_flag: bool,
    /// Дата первой минутной свечи.
    #[prost(message, optional, tag = "56")]
    pub first_1min_candle_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Дата первой дневной свечи.
    #[prost(message, optional, tag = "57")]
    pub first_1day_candle_date: ::core::option::Option<::prost_types::Timestamp>,
}
/// Запрос дивидендов.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetDividendsRequest {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Начало запрашиваемого периода в часовом поясе UTC. Фильтрация происходит по параметру *record_date* (дата фиксации реестра).
    #[prost(message, optional, tag = "2")]
    pub from: ::core::option::Option<::prost_types::Timestamp>,
    /// Окончание запрашиваемого периода в часовом поясе UTC. Фильтрация происходит по параметру *record_date* (дата фиксации реестра).
    #[prost(message, optional, tag = "3")]
    pub to: ::core::option::Option<::prost_types::Timestamp>,
}
/// Дивиденды.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetDividendsResponse {
    #[prost(message, repeated, tag = "1")]
    pub dividends: ::prost::alloc::vec::Vec<Dividend>,
}
/// Информация о выплате.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Dividend {
    /// Величина дивиденда на 1 ценную бумагу (включая валюту).
    #[prost(message, optional, tag = "1")]
    pub dividend_net: ::core::option::Option<MoneyValue>,
    /// Дата фактических выплат в часовом поясе UTC.
    #[prost(message, optional, tag = "2")]
    pub payment_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Дата объявления дивидендов в часовом поясе UTC.
    #[prost(message, optional, tag = "3")]
    pub declared_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Последний день (включительно) покупки для получения выплаты в часовом поясе UTC.
    #[prost(message, optional, tag = "4")]
    pub last_buy_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Тип выплаты. Возможные значения: Regular Cash – регулярные выплаты, Cancelled – выплата отменена, Daily Accrual – ежедневное начисление, Return of Capital – возврат капитала, прочие типы выплат.
    #[prost(string, tag = "5")]
    pub dividend_type: ::prost::alloc::string::String,
    /// Дата фиксации реестра в часовом поясе UTC.
    #[prost(message, optional, tag = "6")]
    pub record_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Регулярность выплаты. Возможные значения: Annual – ежегодная, Semi-Anl – каждые полгода, прочие типы выплат.
    #[prost(string, tag = "7")]
    pub regularity: ::prost::alloc::string::String,
    /// Цена закрытия инструмента на момент ex_dividend_date.
    #[prost(message, optional, tag = "8")]
    pub close_price: ::core::option::Option<MoneyValue>,
    /// Величина доходности.
    #[prost(message, optional, tag = "9")]
    pub yield_value: ::core::option::Option<Quotation>,
    /// Дата и время создания записи в часовом поясе UTC.
    #[prost(message, optional, tag = "10")]
    pub created_at: ::core::option::Option<::prost_types::Timestamp>,
}
/// Запрос актива по идентификатору.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AssetRequest {
    /// uid-идентификатор актива.
    #[prost(string, tag = "1")]
    pub id: ::prost::alloc::string::String,
}
/// Данные по активу.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AssetResponse {
    /// Актив.
    #[prost(message, optional, tag = "1")]
    pub asset: ::core::option::Option<AssetFull>,
}
/// Запрос списка активов.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AssetsRequest {}
/// Список активов.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AssetsResponse {
    /// Активы.
    #[prost(message, repeated, tag = "1")]
    pub assets: ::prost::alloc::vec::Vec<Asset>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AssetFull {
    /// Уникальный идентификатор актива.
    #[prost(string, tag = "1")]
    pub uid: ::prost::alloc::string::String,
    /// Тип актива.
    #[prost(enumeration = "AssetType", tag = "2")]
    pub r#type: i32,
    /// Наименование актива.
    #[prost(string, tag = "3")]
    pub name: ::prost::alloc::string::String,
    /// Короткое наименование актива.
    #[prost(string, tag = "4")]
    pub name_brief: ::prost::alloc::string::String,
    /// Описание актива.
    #[prost(string, tag = "5")]
    pub description: ::prost::alloc::string::String,
    /// Дата и время удаления актива.
    #[prost(message, optional, tag = "6")]
    pub deleted_at: ::core::option::Option<::prost_types::Timestamp>,
    /// Тестирование клиентов.
    #[prost(string, repeated, tag = "7")]
    pub required_tests: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    /// Номер государственной регистрации.
    #[prost(string, tag = "10")]
    pub gos_reg_code: ::prost::alloc::string::String,
    /// Код CFI.
    #[prost(string, tag = "11")]
    pub cfi: ::prost::alloc::string::String,
    /// Код НРД инструмента.
    #[prost(string, tag = "12")]
    pub code_nsd: ::prost::alloc::string::String,
    /// Статус актива.
    #[prost(string, tag = "13")]
    pub status: ::prost::alloc::string::String,
    /// Бренд.
    #[prost(message, optional, tag = "14")]
    pub brand: ::core::option::Option<Brand>,
    /// Дата и время последнего обновления записи.
    #[prost(message, optional, tag = "15")]
    pub updated_at: ::core::option::Option<::prost_types::Timestamp>,
    /// Код типа ц.б. по классификации Банка России.
    #[prost(string, tag = "16")]
    pub br_code: ::prost::alloc::string::String,
    /// Наименование кода типа ц.б. по классификации Банка России.
    #[prost(string, tag = "17")]
    pub br_code_name: ::prost::alloc::string::String,
    /// Массив идентификаторов инструментов.
    #[prost(message, repeated, tag = "18")]
    pub instruments: ::prost::alloc::vec::Vec<AssetInstrument>,
    #[prost(oneof = "asset_full::Ext", tags = "8, 9")]
    pub ext: ::core::option::Option<asset_full::Ext>,
}
/// Nested message and enum types in `AssetFull`.
pub mod asset_full {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Ext {
        /// Валюта. Обязательно и заполняется только для type = "ASSET_TYPE_CURRENCY".
        #[prost(message, tag = "8")]
        Currency(super::AssetCurrency),
        /// Ценная бумага. Обязательно и заполняется только для type = "ASSET_TYPE_SECURITY".
        #[prost(message, tag = "9")]
        Security(super::AssetSecurity),
    }
}
/// Информация об активе.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Asset {
    /// Уникальный идентификатор актива.
    #[prost(string, tag = "1")]
    pub uid: ::prost::alloc::string::String,
    /// Тип актива.
    #[prost(enumeration = "AssetType", tag = "2")]
    pub r#type: i32,
    /// Наименование актива.
    #[prost(string, tag = "3")]
    pub name: ::prost::alloc::string::String,
    /// Массив идентификаторов инструментов.
    #[prost(message, repeated, tag = "4")]
    pub instruments: ::prost::alloc::vec::Vec<AssetInstrument>,
}
/// Валюта.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AssetCurrency {
    /// ISO-код валюты.
    #[prost(string, tag = "1")]
    pub base_currency: ::prost::alloc::string::String,
}
/// Ценная бумага.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AssetSecurity {
    /// ISIN-идентификатор ценной бумаги.
    #[prost(string, tag = "1")]
    pub isin: ::prost::alloc::string::String,
    /// Тип ценной бумаги.
    #[prost(string, tag = "2")]
    pub r#type: ::prost::alloc::string::String,
    #[prost(oneof = "asset_security::Ext", tags = "3, 4, 5, 6, 7")]
    pub ext: ::core::option::Option<asset_security::Ext>,
}
/// Nested message and enum types in `AssetSecurity`.
pub mod asset_security {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Ext {
        /// Акция. Заполняется только для акций (тип актива asset.type = "ASSET_TYPE_SECURITY" и security.type = share).
        #[prost(message, tag = "3")]
        Share(super::AssetShare),
        /// Облигация. Заполняется только для облигаций (тип актива asset.type = "ASSET_TYPE_SECURITY" и security.type = bond).
        #[prost(message, tag = "4")]
        Bond(super::AssetBond),
        /// Структурная нота. Заполняется только для структурных продуктов (тип актива asset.type = "ASSET_TYPE_SECURITY" и security.type = sp).
        #[prost(message, tag = "5")]
        Sp(super::AssetStructuredProduct),
        /// Фонд. Заполняется только для фондов (тип актива asset.type = "ASSET_TYPE_SECURITY" и security.type = etf).
        #[prost(message, tag = "6")]
        Etf(super::AssetEtf),
        /// Клиринговый сертификат участия. Заполняется только для клиринговых сертификатов (тип актива asset.type = "ASSET_TYPE_SECURITY" и security.type = clearing_certificate).
        #[prost(message, tag = "7")]
        ClearingCertificate(super::AssetClearingCertificate),
    }
}
/// Акция.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AssetShare {
    /// Тип акции.
    #[prost(enumeration = "ShareType", tag = "1")]
    pub r#type: i32,
    /// Объем выпуска (шт.).
    #[prost(message, optional, tag = "2")]
    pub issue_size: ::core::option::Option<Quotation>,
    /// Номинал.
    #[prost(message, optional, tag = "3")]
    pub nominal: ::core::option::Option<Quotation>,
    /// Валюта номинала.
    #[prost(string, tag = "4")]
    pub nominal_currency: ::prost::alloc::string::String,
    /// Индекс (Bloomberg).
    #[prost(string, tag = "5")]
    pub primary_index: ::prost::alloc::string::String,
    /// Ставка дивиденда (для привилегированных акций).
    #[prost(message, optional, tag = "6")]
    pub dividend_rate: ::core::option::Option<Quotation>,
    /// Тип привилегированных акций.
    #[prost(string, tag = "7")]
    pub preferred_share_type: ::prost::alloc::string::String,
    /// Дата IPO.
    #[prost(message, optional, tag = "8")]
    pub ipo_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Дата регистрации.
    #[prost(message, optional, tag = "9")]
    pub registry_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Признак наличия дивидендной доходности.
    #[prost(bool, tag = "10")]
    pub div_yield_flag: bool,
    /// Форма выпуска ФИ.
    #[prost(string, tag = "11")]
    pub issue_kind: ::prost::alloc::string::String,
    /// Дата размещения акции.
    #[prost(message, optional, tag = "12")]
    pub placement_date: ::core::option::Option<::prost_types::Timestamp>,
    /// ISIN базового актива.
    #[prost(string, tag = "13")]
    pub repres_isin: ::prost::alloc::string::String,
    /// Объявленное количество шт.
    #[prost(message, optional, tag = "14")]
    pub issue_size_plan: ::core::option::Option<Quotation>,
    /// Количество акций в свободном обращении.
    #[prost(message, optional, tag = "15")]
    pub total_float: ::core::option::Option<Quotation>,
}
/// Облигация.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AssetBond {
    /// Текущий номинал.
    #[prost(message, optional, tag = "1")]
    pub current_nominal: ::core::option::Option<Quotation>,
    /// Наименование заемщика.
    #[prost(string, tag = "2")]
    pub borrow_name: ::prost::alloc::string::String,
    /// Объем эмиссии облигации (стоимость).
    #[prost(message, optional, tag = "3")]
    pub issue_size: ::core::option::Option<Quotation>,
    /// Номинал облигации.
    #[prost(message, optional, tag = "4")]
    pub nominal: ::core::option::Option<Quotation>,
    /// Валюта номинала.
    #[prost(string, tag = "5")]
    pub nominal_currency: ::prost::alloc::string::String,
    /// Форма выпуска облигации.
    #[prost(string, tag = "6")]
    pub issue_kind: ::prost::alloc::string::String,
    /// Форма дохода облигации.
    #[prost(string, tag = "7")]
    pub interest_kind: ::prost::alloc::string::String,
    /// Количество выплат в год.
    #[prost(int32, tag = "8")]
    pub coupon_quantity_per_year: i32,
    /// Признак облигации с индексируемым номиналом.
    #[prost(bool, tag = "9")]
    pub indexed_nominal_flag: bool,
    /// Признак субординированной облигации.
    #[prost(bool, tag = "10")]
    pub subordinated_flag: bool,
    /// Признак обеспеченной облигации.
    #[prost(bool, tag = "11")]
    pub collateral_flag: bool,
    /// Признак показывает, что купоны облигации не облагаются налогом (для mass market).
    #[prost(bool, tag = "12")]
    pub tax_free_flag: bool,
    /// Признак облигации с амортизацией долга.
    #[prost(bool, tag = "13")]
    pub amortization_flag: bool,
    /// Признак облигации с плавающим купоном.
    #[prost(bool, tag = "14")]
    pub floating_coupon_flag: bool,
    /// Признак бессрочной облигации.
    #[prost(bool, tag = "15")]
    pub perpetual_flag: bool,
    /// Дата погашения облигации.
    #[prost(message, optional, tag = "16")]
    pub maturity_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Описание и условия получения дополнительного дохода.
    #[prost(string, tag = "17")]
    pub return_condition: ::prost::alloc::string::String,
    /// Дата выпуска облигации.
    #[prost(message, optional, tag = "18")]
    pub state_reg_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Дата размещения облигации.
    #[prost(message, optional, tag = "19")]
    pub placement_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Цена размещения облигации.
    #[prost(message, optional, tag = "20")]
    pub placement_price: ::core::option::Option<Quotation>,
    /// Объявленное количество шт.
    #[prost(message, optional, tag = "21")]
    pub issue_size_plan: ::core::option::Option<Quotation>,
}
/// Структурная нота.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AssetStructuredProduct {
    /// Наименование заемщика.
    #[prost(string, tag = "1")]
    pub borrow_name: ::prost::alloc::string::String,
    /// Номинал.
    #[prost(message, optional, tag = "2")]
    pub nominal: ::core::option::Option<Quotation>,
    /// Валюта номинала.
    #[prost(string, tag = "3")]
    pub nominal_currency: ::prost::alloc::string::String,
    /// Тип структурной ноты.
    #[prost(enumeration = "StructuredProductType", tag = "4")]
    pub r#type: i32,
    /// Стратегия портфеля.
    #[prost(string, tag = "5")]
    pub logic_portfolio: ::prost::alloc::string::String,
    /// Тип базового актива.
    #[prost(enumeration = "AssetType", tag = "6")]
    pub asset_type: i32,
    /// Вид базового актива в зависимости от типа базового актива.
    #[prost(string, tag = "7")]
    pub basic_asset: ::prost::alloc::string::String,
    /// Барьер сохранности (в процентах).
    #[prost(message, optional, tag = "8")]
    pub safety_barrier: ::core::option::Option<Quotation>,
    /// Дата погашения.
    #[prost(message, optional, tag = "9")]
    pub maturity_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Объявленное количество шт.
    #[prost(message, optional, tag = "10")]
    pub issue_size_plan: ::core::option::Option<Quotation>,
    /// Объем размещения.
    #[prost(message, optional, tag = "11")]
    pub issue_size: ::core::option::Option<Quotation>,
    /// Дата размещения ноты.
    #[prost(message, optional, tag = "12")]
    pub placement_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Форма выпуска.
    #[prost(string, tag = "13")]
    pub issue_kind: ::prost::alloc::string::String,
}
/// Фонд.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AssetEtf {
    /// Суммарные расходы фонда (в %).
    #[prost(message, optional, tag = "1")]
    pub total_expense: ::core::option::Option<Quotation>,
    /// Барьерная ставка доходности после которой фонд имеет право на perfomance fee (в процентах).
    #[prost(message, optional, tag = "2")]
    pub hurdle_rate: ::core::option::Option<Quotation>,
    /// Комиссия за успешные результаты фонда (в процентах).
    #[prost(message, optional, tag = "3")]
    pub performance_fee: ::core::option::Option<Quotation>,
    /// Фиксированная комиссия за управление (в процентах).
    #[prost(message, optional, tag = "4")]
    pub fixed_commission: ::core::option::Option<Quotation>,
    /// Тип распределения доходов от выплат по бумагам.
    #[prost(string, tag = "5")]
    pub payment_type: ::prost::alloc::string::String,
    /// Признак необходимости выхода фонда в плюс для получения комиссии.
    #[prost(bool, tag = "6")]
    pub watermark_flag: bool,
    /// Премия (надбавка к цене) при покупке доли в фонде (в процентах).
    #[prost(message, optional, tag = "7")]
    pub buy_premium: ::core::option::Option<Quotation>,
    /// Ставка дисконта (вычет из цены) при продаже доли в фонде (в процентах).
    #[prost(message, optional, tag = "8")]
    pub sell_discount: ::core::option::Option<Quotation>,
    /// Признак ребалансируемости портфеля фонда.
    #[prost(bool, tag = "9")]
    pub rebalancing_flag: bool,
    /// Периодичность ребалансировки.
    #[prost(string, tag = "10")]
    pub rebalancing_freq: ::prost::alloc::string::String,
    /// Тип управления.
    #[prost(string, tag = "11")]
    pub management_type: ::prost::alloc::string::String,
    /// Индекс, который реплицирует (старается копировать) фонд.
    #[prost(string, tag = "12")]
    pub primary_index: ::prost::alloc::string::String,
    /// База ETF.
    #[prost(string, tag = "13")]
    pub focus_type: ::prost::alloc::string::String,
    /// Признак использования заемных активов (плечо).
    #[prost(bool, tag = "14")]
    pub leveraged_flag: bool,
    /// Количество акций в обращении.
    #[prost(message, optional, tag = "15")]
    pub num_share: ::core::option::Option<Quotation>,
    /// Признак обязательства по отчетности перед регулятором.
    #[prost(bool, tag = "16")]
    pub ucits_flag: bool,
    /// Дата выпуска.
    #[prost(message, optional, tag = "17")]
    pub released_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Описание фонда.
    #[prost(string, tag = "18")]
    pub description: ::prost::alloc::string::String,
    /// Описание индекса, за которым следует фонд.
    #[prost(string, tag = "19")]
    pub primary_index_description: ::prost::alloc::string::String,
    /// Основные компании, в которые вкладывается фонд.
    #[prost(string, tag = "20")]
    pub primary_index_company: ::prost::alloc::string::String,
    /// Срок восстановления индекса (после просадки).
    #[prost(message, optional, tag = "21")]
    pub index_recovery_period: ::core::option::Option<Quotation>,
    /// IVAV-код.
    #[prost(string, tag = "22")]
    pub inav_code: ::prost::alloc::string::String,
    /// Признак наличия дивидендной доходности.
    #[prost(bool, tag = "23")]
    pub div_yield_flag: bool,
    /// Комиссия на покрытие расходов фонда (в процентах).
    #[prost(message, optional, tag = "24")]
    pub expense_commission: ::core::option::Option<Quotation>,
    /// Ошибка следования за индексом (в процентах).
    #[prost(message, optional, tag = "25")]
    pub primary_index_tracking_error: ::core::option::Option<Quotation>,
    /// Плановая ребалансировка портфеля.
    #[prost(string, tag = "26")]
    pub rebalancing_plan: ::prost::alloc::string::String,
    /// Ставки налогообложения дивидендов и купонов.
    #[prost(string, tag = "27")]
    pub tax_rate: ::prost::alloc::string::String,
    /// Даты ребалансировок.
    #[prost(message, repeated, tag = "28")]
    pub rebalancing_dates: ::prost::alloc::vec::Vec<::prost_types::Timestamp>,
    /// Форма выпуска.
    #[prost(string, tag = "29")]
    pub issue_kind: ::prost::alloc::string::String,
    /// Номинал.
    #[prost(message, optional, tag = "30")]
    pub nominal: ::core::option::Option<Quotation>,
    /// Валюта номинала.
    #[prost(string, tag = "31")]
    pub nominal_currency: ::prost::alloc::string::String,
}
/// Клиринговый сертификат участия.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AssetClearingCertificate {
    /// Номинал.
    #[prost(message, optional, tag = "1")]
    pub nominal: ::core::option::Option<Quotation>,
    /// Валюта номинала.
    #[prost(string, tag = "2")]
    pub nominal_currency: ::prost::alloc::string::String,
}
/// Бренд.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Brand {
    /// uid идентификатор бренда.
    #[prost(string, tag = "1")]
    pub uid: ::prost::alloc::string::String,
    /// Наименование бренда.
    #[prost(string, tag = "2")]
    pub name: ::prost::alloc::string::String,
    /// Описание.
    #[prost(string, tag = "3")]
    pub description: ::prost::alloc::string::String,
    /// Информация о бренде.
    #[prost(string, tag = "4")]
    pub info: ::prost::alloc::string::String,
    /// Компания.
    #[prost(string, tag = "5")]
    pub company: ::prost::alloc::string::String,
    /// Сектор.
    #[prost(string, tag = "6")]
    pub sector: ::prost::alloc::string::String,
    /// Код страны риска.
    #[prost(string, tag = "7")]
    pub country_of_risk: ::prost::alloc::string::String,
    /// Наименование страны риска.
    #[prost(string, tag = "8")]
    pub country_of_risk_name: ::prost::alloc::string::String,
}
/// Идентификаторы инструмента.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AssetInstrument {
    /// uid идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub uid: ::prost::alloc::string::String,
    /// figi идентификатор инструмента.
    #[prost(string, tag = "2")]
    pub figi: ::prost::alloc::string::String,
    /// Тип инструмента.
    #[prost(string, tag = "3")]
    pub instrument_type: ::prost::alloc::string::String,
    /// Тикер инструмента.
    #[prost(string, tag = "4")]
    pub ticker: ::prost::alloc::string::String,
    /// Класс-код (секция торгов).
    #[prost(string, tag = "5")]
    pub class_code: ::prost::alloc::string::String,
    /// Массив связанных инструментов.
    #[prost(message, repeated, tag = "6")]
    pub links: ::prost::alloc::vec::Vec<InstrumentLink>,
}
/// Связь с другим инструментом.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InstrumentLink {
    /// Тип связи.
    #[prost(string, tag = "1")]
    pub r#type: ::prost::alloc::string::String,
    /// uid идентификатор связанного инструмента.
    #[prost(string, tag = "2")]
    pub instrument_uid: ::prost::alloc::string::String,
}
/// Запрос списка избранных инструментов, входные параметры не требуются.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetFavoritesRequest {}
/// В ответ передаётся список избранных инструментов в качестве массива.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetFavoritesResponse {
    /// Массив инструментов
    #[prost(message, repeated, tag = "1")]
    pub favorite_instruments: ::prost::alloc::vec::Vec<FavoriteInstrument>,
}
/// Массив избранных инструментов.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FavoriteInstrument {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Тикер инструмента.
    #[prost(string, tag = "2")]
    pub ticker: ::prost::alloc::string::String,
    /// Класс-код инструмента.
    #[prost(string, tag = "3")]
    pub class_code: ::prost::alloc::string::String,
    /// Isin-идентификатор инструмента.
    #[prost(string, tag = "4")]
    pub isin: ::prost::alloc::string::String,
    /// Тип инструмента.
    #[prost(string, tag = "11")]
    pub instrument_type: ::prost::alloc::string::String,
    /// Признак внебиржевой ценной бумаги.
    #[prost(bool, tag = "16")]
    pub otc_flag: bool,
    /// Параметр указывает на возможность торговать инструментом через API.
    #[prost(bool, tag = "17")]
    pub api_trade_available_flag: bool,
}
/// Запрос редактирования списка избранных инструментов.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EditFavoritesRequest {
    /// Массив инструментов.
    #[prost(message, repeated, tag = "1")]
    pub instruments: ::prost::alloc::vec::Vec<EditFavoritesRequestInstrument>,
    /// Тип действия со списком.
    #[prost(enumeration = "EditFavoritesActionType", tag = "6")]
    pub action_type: i32,
}
/// Массив инструментов для редактирования списка избранных инструментов.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EditFavoritesRequestInstrument {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
}
/// Результат редактирования списка избранных инструментов.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct EditFavoritesResponse {
    /// Массив инструментов
    #[prost(message, repeated, tag = "1")]
    pub favorite_instruments: ::prost::alloc::vec::Vec<FavoriteInstrument>,
}
/// Запрос справочника стран.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetCountriesRequest {}
/// Справочник стран.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetCountriesResponse {
    /// Массив стран.
    #[prost(message, repeated, tag = "1")]
    pub countries: ::prost::alloc::vec::Vec<CountryResponse>,
}
/// Данные о стране.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CountryResponse {
    /// Двухбуквенный код страны.
    #[prost(string, tag = "1")]
    pub alfa_two: ::prost::alloc::string::String,
    /// Трёхбуквенный код страны.
    #[prost(string, tag = "2")]
    pub alfa_three: ::prost::alloc::string::String,
    /// Наименование страны.
    #[prost(string, tag = "3")]
    pub name: ::prost::alloc::string::String,
    /// Краткое наименование страны.
    #[prost(string, tag = "4")]
    pub name_brief: ::prost::alloc::string::String,
}
/// Запрос на поиск инструментов.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FindInstrumentRequest {
    /// Строка поиска.
    #[prost(string, tag = "1")]
    pub query: ::prost::alloc::string::String,
}
/// Результат поиска инструментов.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FindInstrumentResponse {
    /// Массив инструментов, удовлетворяющих условиям поиска.
    #[prost(message, repeated, tag = "1")]
    pub instruments: ::prost::alloc::vec::Vec<InstrumentShort>,
}
/// Краткая информация об инструменте.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InstrumentShort {
    /// Isin инструмента.
    #[prost(string, tag = "1")]
    pub isin: ::prost::alloc::string::String,
    /// Figi инструмента.
    #[prost(string, tag = "2")]
    pub figi: ::prost::alloc::string::String,
    /// Ticker инструмента.
    #[prost(string, tag = "3")]
    pub ticker: ::prost::alloc::string::String,
    /// ClassCode инструмента.
    #[prost(string, tag = "4")]
    pub class_code: ::prost::alloc::string::String,
    /// Тип инструмента.
    #[prost(string, tag = "5")]
    pub instrument_type: ::prost::alloc::string::String,
    /// Название инструмента.
    #[prost(string, tag = "6")]
    pub name: ::prost::alloc::string::String,
    /// Уникальный идентификатор инструмента.
    #[prost(string, tag = "7")]
    pub uid: ::prost::alloc::string::String,
    /// Уникальный идентификатор позиции инструмента.
    #[prost(string, tag = "8")]
    pub position_uid: ::prost::alloc::string::String,
    /// Параметр указывает на возможность торговать инструментом через API.
    #[prost(bool, tag = "11")]
    pub api_trade_available_flag: bool,
    /// Признак доступности для ИИС.
    #[prost(bool, tag = "12")]
    pub for_iis_flag: bool,
    /// Дата первой минутной свечи.
    #[prost(message, optional, tag = "26")]
    pub first_1min_candle_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Дата первой дневной свечи.
    #[prost(message, optional, tag = "27")]
    pub first_1day_candle_date: ::core::option::Option<::prost_types::Timestamp>,
    /// Флаг отображающий доступность торговли инструментом только для квалифицированных инвесторов.
    #[prost(bool, tag = "28")]
    pub for_qual_investor_flag: bool,
    /// Флаг отображающий доступность торговли инструментом по выходным
    #[prost(bool, tag = "29")]
    pub weekend_flag: bool,
    /// Флаг заблокированного ТКС
    #[prost(bool, tag = "30")]
    pub blocked_tca_flag: bool,
}
/// Запрос списка брендов.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetBrandsRequest {}
/// Запрос бренда.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetBrandRequest {
    /// Uid-идентификатор бренда.
    #[prost(string, tag = "1")]
    pub id: ::prost::alloc::string::String,
}
/// Список брендов.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetBrandsResponse {
    /// Массив брендов.
    #[prost(message, repeated, tag = "1")]
    pub brands: ::prost::alloc::vec::Vec<Brand>,
}
/// Тип купонов.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum CouponType {
    /// Неопределенное значение
    Unspecified = 0,
    /// Постоянный
    Constant = 1,
    /// Плавающий
    Floating = 2,
    /// Дисконт
    Discount = 3,
    /// Ипотечный
    Mortgage = 4,
    /// Фиксированный
    Fix = 5,
    /// Переменный
    Variable = 6,
    /// Прочее
    Other = 7,
}
impl CouponType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            CouponType::Unspecified => "COUPON_TYPE_UNSPECIFIED",
            CouponType::Constant => "COUPON_TYPE_CONSTANT",
            CouponType::Floating => "COUPON_TYPE_FLOATING",
            CouponType::Discount => "COUPON_TYPE_DISCOUNT",
            CouponType::Mortgage => "COUPON_TYPE_MORTGAGE",
            CouponType::Fix => "COUPON_TYPE_FIX",
            CouponType::Variable => "COUPON_TYPE_VARIABLE",
            CouponType::Other => "COUPON_TYPE_OTHER",
        }
    }
}
/// Тип опциона по направлению сделки.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum OptionDirection {
    /// Тип не определен.
    Unspecified = 0,
    /// Опцион на продажу.
    Put = 1,
    /// Опцион на покупку.
    Call = 2,
}
impl OptionDirection {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            OptionDirection::Unspecified => "OPTION_DIRECTION_UNSPECIFIED",
            OptionDirection::Put => "OPTION_DIRECTION_PUT",
            OptionDirection::Call => "OPTION_DIRECTION_CALL",
        }
    }
}
/// Тип расчетов по опциону.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum OptionPaymentType {
    /// Тип не определен.
    Unspecified = 0,
    /// Опционы с использованием премии в расчетах.
    Premium = 1,
    /// Маржируемые опционы.
    Marginal = 2,
}
impl OptionPaymentType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            OptionPaymentType::Unspecified => "OPTION_PAYMENT_TYPE_UNSPECIFIED",
            OptionPaymentType::Premium => "OPTION_PAYMENT_TYPE_PREMIUM",
            OptionPaymentType::Marginal => "OPTION_PAYMENT_TYPE_MARGINAL",
        }
    }
}
/// Тип опциона по стилю.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum OptionStyle {
    /// Тип не определен.
    Unspecified = 0,
    /// Американский опцион.
    American = 1,
    /// Европейский опцион.
    European = 2,
}
impl OptionStyle {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            OptionStyle::Unspecified => "OPTION_STYLE_UNSPECIFIED",
            OptionStyle::American => "OPTION_STYLE_AMERICAN",
            OptionStyle::European => "OPTION_STYLE_EUROPEAN",
        }
    }
}
/// Тип опциона по способу исполнения.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum OptionSettlementType {
    /// Тип не определен.
    OptionExecutionTypeUnspecified = 0,
    /// Поставочный тип опциона.
    OptionExecutionTypePhysicalDelivery = 1,
    /// Расчетный тип опциона.
    OptionExecutionTypeCashSettlement = 2,
}
impl OptionSettlementType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            OptionSettlementType::OptionExecutionTypeUnspecified => {
                "OPTION_EXECUTION_TYPE_UNSPECIFIED"
            }
            OptionSettlementType::OptionExecutionTypePhysicalDelivery => {
                "OPTION_EXECUTION_TYPE_PHYSICAL_DELIVERY"
            }
            OptionSettlementType::OptionExecutionTypeCashSettlement => {
                "OPTION_EXECUTION_TYPE_CASH_SETTLEMENT"
            }
        }
    }
}
/// Тип идентификатора инструмента. Подробнее об идентификации инструментов: [Идентификация инструментов](<https://tinkoff.github.io/investAPI/faq_identification/>)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum InstrumentIdType {
    /// Значение не определено.
    InstrumentIdUnspecified = 0,
    /// Figi.
    Figi = 1,
    /// Ticker.
    Ticker = 2,
    /// Уникальный идентификатор.
    Uid = 3,
    /// Идентификатор позиции.
    PositionUid = 4,
}
impl InstrumentIdType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            InstrumentIdType::InstrumentIdUnspecified => "INSTRUMENT_ID_UNSPECIFIED",
            InstrumentIdType::Figi => "INSTRUMENT_ID_TYPE_FIGI",
            InstrumentIdType::Ticker => "INSTRUMENT_ID_TYPE_TICKER",
            InstrumentIdType::Uid => "INSTRUMENT_ID_TYPE_UID",
            InstrumentIdType::PositionUid => "INSTRUMENT_ID_TYPE_POSITION_UID",
        }
    }
}
/// Статус запрашиваемых инструментов.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum InstrumentStatus {
    /// Значение не определено.
    Unspecified = 0,
    /// Базовый список инструментов (по умолчанию). Инструменты доступные для торговли через TINKOFF INVEST API.
    Base = 1,
    /// Список всех инструментов.
    All = 2,
}
impl InstrumentStatus {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            InstrumentStatus::Unspecified => "INSTRUMENT_STATUS_UNSPECIFIED",
            InstrumentStatus::Base => "INSTRUMENT_STATUS_BASE",
            InstrumentStatus::All => "INSTRUMENT_STATUS_ALL",
        }
    }
}
/// Тип акций.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ShareType {
    /// Значение не определено.
    Unspecified = 0,
    /// Обыкновенная
    Common = 1,
    /// Привилегированная
    Preferred = 2,
    /// Американские депозитарные расписки
    Adr = 3,
    /// Глобальные депозитарные расписки
    Gdr = 4,
    /// Товарищество с ограниченной ответственностью
    Mlp = 5,
    /// Акции из реестра Нью-Йорка
    NyRegShrs = 6,
    /// Закрытый инвестиционный фонд
    ClosedEndFund = 7,
    /// Траст недвижимости
    Reit = 8,
}
impl ShareType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            ShareType::Unspecified => "SHARE_TYPE_UNSPECIFIED",
            ShareType::Common => "SHARE_TYPE_COMMON",
            ShareType::Preferred => "SHARE_TYPE_PREFERRED",
            ShareType::Adr => "SHARE_TYPE_ADR",
            ShareType::Gdr => "SHARE_TYPE_GDR",
            ShareType::Mlp => "SHARE_TYPE_MLP",
            ShareType::NyRegShrs => "SHARE_TYPE_NY_REG_SHRS",
            ShareType::ClosedEndFund => "SHARE_TYPE_CLOSED_END_FUND",
            ShareType::Reit => "SHARE_TYPE_REIT",
        }
    }
}
/// Тип актива.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum AssetType {
    /// Тип не определён.
    Unspecified = 0,
    /// Валюта.
    Currency = 1,
    /// Товар.
    Commodity = 2,
    /// Индекс.
    Index = 3,
    /// Ценная бумага.
    Security = 4,
}
impl AssetType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            AssetType::Unspecified => "ASSET_TYPE_UNSPECIFIED",
            AssetType::Currency => "ASSET_TYPE_CURRENCY",
            AssetType::Commodity => "ASSET_TYPE_COMMODITY",
            AssetType::Index => "ASSET_TYPE_INDEX",
            AssetType::Security => "ASSET_TYPE_SECURITY",
        }
    }
}
/// Тип структурной ноты.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum StructuredProductType {
    /// Тип не определён.
    SpTypeUnspecified = 0,
    /// Поставочный.
    SpTypeDeliverable = 1,
    /// Беспоставочный.
    SpTypeNonDeliverable = 2,
}
impl StructuredProductType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            StructuredProductType::SpTypeUnspecified => "SP_TYPE_UNSPECIFIED",
            StructuredProductType::SpTypeDeliverable => "SP_TYPE_DELIVERABLE",
            StructuredProductType::SpTypeNonDeliverable => "SP_TYPE_NON_DELIVERABLE",
        }
    }
}
/// Тип действия со списком избранных инструментов.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum EditFavoritesActionType {
    /// Тип не определён.
    Unspecified = 0,
    /// Добавить в список.
    Add = 1,
    /// Удалить из списка.
    Del = 2,
}
impl EditFavoritesActionType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            EditFavoritesActionType::Unspecified => {
                "EDIT_FAVORITES_ACTION_TYPE_UNSPECIFIED"
            }
            EditFavoritesActionType::Add => "EDIT_FAVORITES_ACTION_TYPE_ADD",
            EditFavoritesActionType::Del => "EDIT_FAVORITES_ACTION_TYPE_DEL",
        }
    }
}
/// Реальная площадка исполнения расчётов.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum RealExchange {
    /// Тип не определён.
    Unspecified = 0,
    /// Московская биржа.
    Moex = 1,
    /// Санкт-Петербургская биржа.
    Rts = 2,
    /// Внебиржевой инструмент.
    Otc = 3,
}
impl RealExchange {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            RealExchange::Unspecified => "REAL_EXCHANGE_UNSPECIFIED",
            RealExchange::Moex => "REAL_EXCHANGE_MOEX",
            RealExchange::Rts => "REAL_EXCHANGE_RTS",
            RealExchange::Otc => "REAL_EXCHANGE_OTC",
        }
    }
}
/// Generated client implementations.
pub mod instruments_service_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct InstrumentsServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl InstrumentsServiceClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> InstrumentsServiceClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> InstrumentsServiceClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + Send + Sync,
        {
            InstrumentsServiceClient::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        /// Метод получения расписания торгов торговых площадок.
        pub async fn trading_schedules(
            &mut self,
            request: impl tonic::IntoRequest<super::TradingSchedulesRequest>,
        ) -> Result<tonic::Response<super::TradingSchedulesResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/TradingSchedules",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод получения облигации по её идентификатору.
        pub async fn bond_by(
            &mut self,
            request: impl tonic::IntoRequest<super::InstrumentRequest>,
        ) -> Result<tonic::Response<super::BondResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/BondBy",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод получения списка облигаций.
        pub async fn bonds(
            &mut self,
            request: impl tonic::IntoRequest<super::InstrumentsRequest>,
        ) -> Result<tonic::Response<super::BondsResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/Bonds",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод получения графика выплат купонов по облигации.
        pub async fn get_bond_coupons(
            &mut self,
            request: impl tonic::IntoRequest<super::GetBondCouponsRequest>,
        ) -> Result<tonic::Response<super::GetBondCouponsResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/GetBondCoupons",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод получения валюты по её идентификатору.
        pub async fn currency_by(
            &mut self,
            request: impl tonic::IntoRequest<super::InstrumentRequest>,
        ) -> Result<tonic::Response<super::CurrencyResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/CurrencyBy",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод получения списка валют.
        pub async fn currencies(
            &mut self,
            request: impl tonic::IntoRequest<super::InstrumentsRequest>,
        ) -> Result<tonic::Response<super::CurrenciesResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/Currencies",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод получения инвестиционного фонда по его идентификатору.
        pub async fn etf_by(
            &mut self,
            request: impl tonic::IntoRequest<super::InstrumentRequest>,
        ) -> Result<tonic::Response<super::EtfResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/EtfBy",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод получения списка инвестиционных фондов.
        pub async fn etfs(
            &mut self,
            request: impl tonic::IntoRequest<super::InstrumentsRequest>,
        ) -> Result<tonic::Response<super::EtfsResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/Etfs",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод получения фьючерса по его идентификатору.
        pub async fn future_by(
            &mut self,
            request: impl tonic::IntoRequest<super::InstrumentRequest>,
        ) -> Result<tonic::Response<super::FutureResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/FutureBy",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод получения списка фьючерсов.
        pub async fn futures(
            &mut self,
            request: impl tonic::IntoRequest<super::InstrumentsRequest>,
        ) -> Result<tonic::Response<super::FuturesResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/Futures",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод получения опциона по его идентификатору.
        pub async fn option_by(
            &mut self,
            request: impl tonic::IntoRequest<super::InstrumentRequest>,
        ) -> Result<tonic::Response<super::OptionResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/OptionBy",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод получения списка опционов.
        pub async fn options(
            &mut self,
            request: impl tonic::IntoRequest<super::InstrumentsRequest>,
        ) -> Result<tonic::Response<super::OptionsResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/Options",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод получения акции по её идентификатору.
        pub async fn share_by(
            &mut self,
            request: impl tonic::IntoRequest<super::InstrumentRequest>,
        ) -> Result<tonic::Response<super::ShareResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/ShareBy",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод получения списка акций.
        pub async fn shares(
            &mut self,
            request: impl tonic::IntoRequest<super::InstrumentsRequest>,
        ) -> Result<tonic::Response<super::SharesResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/Shares",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод получения накопленного купонного дохода по облигации.
        pub async fn get_accrued_interests(
            &mut self,
            request: impl tonic::IntoRequest<super::GetAccruedInterestsRequest>,
        ) -> Result<tonic::Response<super::GetAccruedInterestsResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/GetAccruedInterests",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод получения размера гарантийного обеспечения по фьючерсам.
        pub async fn get_futures_margin(
            &mut self,
            request: impl tonic::IntoRequest<super::GetFuturesMarginRequest>,
        ) -> Result<tonic::Response<super::GetFuturesMarginResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/GetFuturesMargin",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод получения основной информации об инструменте.
        pub async fn get_instrument_by(
            &mut self,
            request: impl tonic::IntoRequest<super::InstrumentRequest>,
        ) -> Result<tonic::Response<super::InstrumentResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/GetInstrumentBy",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод для получения событий выплаты дивидендов по инструменту.
        pub async fn get_dividends(
            &mut self,
            request: impl tonic::IntoRequest<super::GetDividendsRequest>,
        ) -> Result<tonic::Response<super::GetDividendsResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/GetDividends",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод получения актива по его идентификатору.
        pub async fn get_asset_by(
            &mut self,
            request: impl tonic::IntoRequest<super::AssetRequest>,
        ) -> Result<tonic::Response<super::AssetResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/GetAssetBy",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод получения списка активов.
        pub async fn get_assets(
            &mut self,
            request: impl tonic::IntoRequest<super::AssetsRequest>,
        ) -> Result<tonic::Response<super::AssetsResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/GetAssets",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод получения списка избранных инструментов.
        pub async fn get_favorites(
            &mut self,
            request: impl tonic::IntoRequest<super::GetFavoritesRequest>,
        ) -> Result<tonic::Response<super::GetFavoritesResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/GetFavorites",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод редактирования списка избранных инструментов.
        pub async fn edit_favorites(
            &mut self,
            request: impl tonic::IntoRequest<super::EditFavoritesRequest>,
        ) -> Result<tonic::Response<super::EditFavoritesResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/EditFavorites",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод получения списка стран.
        pub async fn get_countries(
            &mut self,
            request: impl tonic::IntoRequest<super::GetCountriesRequest>,
        ) -> Result<tonic::Response<super::GetCountriesResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/GetCountries",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод поиска инструмента.
        pub async fn find_instrument(
            &mut self,
            request: impl tonic::IntoRequest<super::FindInstrumentRequest>,
        ) -> Result<tonic::Response<super::FindInstrumentResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/FindInstrument",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод получения списка брендов.
        pub async fn get_brands(
            &mut self,
            request: impl tonic::IntoRequest<super::GetBrandsRequest>,
        ) -> Result<tonic::Response<super::GetBrandsResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/GetBrands",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод получения бренда по его идентификатору.
        pub async fn get_brand_by(
            &mut self,
            request: impl tonic::IntoRequest<super::GetBrandRequest>,
        ) -> Result<tonic::Response<super::Brand>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.InstrumentsService/GetBrandBy",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
    }
}
/// Запрос подписки или отписки на определённые биржевые данные.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MarketDataRequest {
    #[prost(oneof = "market_data_request::Payload", tags = "1, 2, 3, 4, 5, 6")]
    pub payload: ::core::option::Option<market_data_request::Payload>,
}
/// Nested message and enum types in `MarketDataRequest`.
pub mod market_data_request {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Payload {
        /// Запрос подписки на свечи.
        #[prost(message, tag = "1")]
        SubscribeCandlesRequest(super::SubscribeCandlesRequest),
        /// Запрос подписки на стаканы.
        #[prost(message, tag = "2")]
        SubscribeOrderBookRequest(super::SubscribeOrderBookRequest),
        /// Запрос подписки на ленту обезличенных сделок.
        #[prost(message, tag = "3")]
        SubscribeTradesRequest(super::SubscribeTradesRequest),
        /// Запрос подписки на торговые статусы инструментов.
        #[prost(message, tag = "4")]
        SubscribeInfoRequest(super::SubscribeInfoRequest),
        /// Запрос подписки на цены последних сделок.
        #[prost(message, tag = "5")]
        SubscribeLastPriceRequest(super::SubscribeLastPriceRequest),
        /// Запрос своих подписок.
        #[prost(message, tag = "6")]
        GetMySubscriptions(super::GetMySubscriptions),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MarketDataServerSideStreamRequest {
    /// Запрос подписки на свечи.
    #[prost(message, optional, tag = "1")]
    pub subscribe_candles_request: ::core::option::Option<SubscribeCandlesRequest>,
    /// Запрос подписки на стаканы.
    #[prost(message, optional, tag = "2")]
    pub subscribe_order_book_request: ::core::option::Option<SubscribeOrderBookRequest>,
    /// Запрос подписки на ленту обезличенных сделок.
    #[prost(message, optional, tag = "3")]
    pub subscribe_trades_request: ::core::option::Option<SubscribeTradesRequest>,
    /// Запрос подписки на торговые статусы инструментов.
    #[prost(message, optional, tag = "4")]
    pub subscribe_info_request: ::core::option::Option<SubscribeInfoRequest>,
    /// Запрос подписки на цены последних сделок.
    #[prost(message, optional, tag = "5")]
    pub subscribe_last_price_request: ::core::option::Option<SubscribeLastPriceRequest>,
}
/// Пакет биржевой информации по подписке.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MarketDataResponse {
    #[prost(
        oneof = "market_data_response::Payload",
        tags = "1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11"
    )]
    pub payload: ::core::option::Option<market_data_response::Payload>,
}
/// Nested message and enum types in `MarketDataResponse`.
pub mod market_data_response {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Payload {
        /// Результат подписки на свечи.
        #[prost(message, tag = "1")]
        SubscribeCandlesResponse(super::SubscribeCandlesResponse),
        /// Результат подписки на стаканы.
        #[prost(message, tag = "2")]
        SubscribeOrderBookResponse(super::SubscribeOrderBookResponse),
        /// Результат подписки на поток обезличенных сделок.
        #[prost(message, tag = "3")]
        SubscribeTradesResponse(super::SubscribeTradesResponse),
        /// Результат подписки на торговые статусы инструментов.
        #[prost(message, tag = "4")]
        SubscribeInfoResponse(super::SubscribeInfoResponse),
        /// Свеча.
        #[prost(message, tag = "5")]
        Candle(super::Candle),
        /// Сделки.
        #[prost(message, tag = "6")]
        Trade(super::Trade),
        /// Стакан.
        #[prost(message, tag = "7")]
        Orderbook(super::OrderBook),
        /// Торговый статус.
        #[prost(message, tag = "8")]
        TradingStatus(super::TradingStatus),
        /// Проверка активности стрима.
        #[prost(message, tag = "9")]
        Ping(super::Ping),
        /// Результат подписки на цены последние сделок по инструментам.
        #[prost(message, tag = "10")]
        SubscribeLastPriceResponse(super::SubscribeLastPriceResponse),
        /// Цена последней сделки.
        #[prost(message, tag = "11")]
        LastPrice(super::LastPrice),
    }
}
/// subscribeCandles | Изменения статуса подписки на свечи.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubscribeCandlesRequest {
    /// Изменение статуса подписки.
    #[prost(enumeration = "SubscriptionAction", tag = "1")]
    pub subscription_action: i32,
    /// Массив инструментов для подписки на свечи.
    #[prost(message, repeated, tag = "2")]
    pub instruments: ::prost::alloc::vec::Vec<CandleInstrument>,
    /// Флаг ожидания закрытия временного интервала для отправки свечи, применяется только для минутных свечей.
    #[prost(bool, tag = "3")]
    pub waiting_close: bool,
}
/// Запрос изменения статус подписки на свечи.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CandleInstrument {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Интервал свечей.
    #[prost(enumeration = "SubscriptionInterval", tag = "2")]
    pub interval: i32,
    /// Идентификатор инструмента, принимает значение figi или instrument_uid
    #[prost(string, tag = "3")]
    pub instrument_id: ::prost::alloc::string::String,
}
/// Результат изменения статус подписки на свечи.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubscribeCandlesResponse {
    /// Уникальный идентификатор запроса, подробнее: \[tracking_id\](<https://tinkoff.github.io/investAPI/grpc#tracking-id>).
    #[prost(string, tag = "1")]
    pub tracking_id: ::prost::alloc::string::String,
    /// Массив статусов подписки на свечи.
    #[prost(message, repeated, tag = "2")]
    pub candles_subscriptions: ::prost::alloc::vec::Vec<CandleSubscription>,
}
/// Статус подписки на свечи.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CandleSubscription {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Интервал свечей.
    #[prost(enumeration = "SubscriptionInterval", tag = "2")]
    pub interval: i32,
    /// Статус подписки.
    #[prost(enumeration = "SubscriptionStatus", tag = "3")]
    pub subscription_status: i32,
    /// Uid инструмента
    #[prost(string, tag = "4")]
    pub instrument_uid: ::prost::alloc::string::String,
}
/// Запрос на изменение статуса подписки на стаканы.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubscribeOrderBookRequest {
    /// Изменение статуса подписки.
    #[prost(enumeration = "SubscriptionAction", tag = "1")]
    pub subscription_action: i32,
    /// Массив инструментов для подписки на стаканы.
    #[prost(message, repeated, tag = "2")]
    pub instruments: ::prost::alloc::vec::Vec<OrderBookInstrument>,
}
/// Запрос подписки на стаканы.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OrderBookInstrument {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Глубина стакана.
    #[prost(int32, tag = "2")]
    pub depth: i32,
    /// Идентификатор инструмента, принимает значение figi или instrument_uid
    #[prost(string, tag = "3")]
    pub instrument_id: ::prost::alloc::string::String,
}
/// Результат изменения статуса подписки на стаканы.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubscribeOrderBookResponse {
    /// Уникальный идентификатор запроса, подробнее: \[tracking_id\](<https://tinkoff.github.io/investAPI/grpc#tracking-id>).
    #[prost(string, tag = "1")]
    pub tracking_id: ::prost::alloc::string::String,
    /// Массив статусов подписки на стаканы.
    #[prost(message, repeated, tag = "2")]
    pub order_book_subscriptions: ::prost::alloc::vec::Vec<OrderBookSubscription>,
}
/// Статус подписки.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OrderBookSubscription {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Глубина стакана.
    #[prost(int32, tag = "2")]
    pub depth: i32,
    /// Статус подписки.
    #[prost(enumeration = "SubscriptionStatus", tag = "3")]
    pub subscription_status: i32,
    /// Uid инструмента
    #[prost(string, tag = "4")]
    pub instrument_uid: ::prost::alloc::string::String,
}
/// Изменение статуса подписки на поток обезличенных сделок.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubscribeTradesRequest {
    /// Изменение статуса подписки.
    #[prost(enumeration = "SubscriptionAction", tag = "1")]
    pub subscription_action: i32,
    /// Массив инструментов для подписки на поток обезличенных сделок.
    #[prost(message, repeated, tag = "2")]
    pub instruments: ::prost::alloc::vec::Vec<TradeInstrument>,
}
/// Запрос подписки на поток обезличенных сделок.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TradeInstrument {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Идентификатор инструмента, принимает значение figi или instrument_uid
    #[prost(string, tag = "2")]
    pub instrument_id: ::prost::alloc::string::String,
}
/// Результат изменения статуса подписки на поток обезличенных сделок.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubscribeTradesResponse {
    /// Уникальный идентификатор запроса, подробнее: \[tracking_id\](<https://tinkoff.github.io/investAPI/grpc#tracking-id>).
    #[prost(string, tag = "1")]
    pub tracking_id: ::prost::alloc::string::String,
    /// Массив статусов подписки на поток сделок.
    #[prost(message, repeated, tag = "2")]
    pub trade_subscriptions: ::prost::alloc::vec::Vec<TradeSubscription>,
}
/// Статус подписки.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TradeSubscription {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Статус подписки.
    #[prost(enumeration = "SubscriptionStatus", tag = "2")]
    pub subscription_status: i32,
    /// Uid инструмента
    #[prost(string, tag = "3")]
    pub instrument_uid: ::prost::alloc::string::String,
}
/// Изменение статуса подписки на торговый статус инструмента.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubscribeInfoRequest {
    /// Изменение статуса подписки.
    #[prost(enumeration = "SubscriptionAction", tag = "1")]
    pub subscription_action: i32,
    /// Массив инструментов для подписки на торговый статус.
    #[prost(message, repeated, tag = "2")]
    pub instruments: ::prost::alloc::vec::Vec<InfoInstrument>,
}
/// Запрос подписки на торговый статус.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InfoInstrument {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Идентификатор инструмента, принимает значение figi или instrument_uid
    #[prost(string, tag = "2")]
    pub instrument_id: ::prost::alloc::string::String,
}
/// Результат изменения статуса подписки на торговый статус.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubscribeInfoResponse {
    /// Уникальный идентификатор запроса, подробнее: \[tracking_id\](<https://tinkoff.github.io/investAPI/grpc#tracking-id>).
    #[prost(string, tag = "1")]
    pub tracking_id: ::prost::alloc::string::String,
    /// Массив статусов подписки на торговый статус.
    #[prost(message, repeated, tag = "2")]
    pub info_subscriptions: ::prost::alloc::vec::Vec<InfoSubscription>,
}
/// Статус подписки.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InfoSubscription {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Статус подписки.
    #[prost(enumeration = "SubscriptionStatus", tag = "2")]
    pub subscription_status: i32,
    /// Uid инструмента
    #[prost(string, tag = "3")]
    pub instrument_uid: ::prost::alloc::string::String,
}
/// Изменение статуса подписки на цену последней сделки по инструменту.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubscribeLastPriceRequest {
    /// Изменение статуса подписки.
    #[prost(enumeration = "SubscriptionAction", tag = "1")]
    pub subscription_action: i32,
    /// Массив инструментов для подписки на цену последней сделки.
    #[prost(message, repeated, tag = "2")]
    pub instruments: ::prost::alloc::vec::Vec<LastPriceInstrument>,
}
/// Запрос подписки на последнюю цену.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LastPriceInstrument {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Идентификатор инструмента, принимает значение figi или instrument_uid
    #[prost(string, tag = "2")]
    pub instrument_id: ::prost::alloc::string::String,
}
/// Результат изменения статуса подписки на цену последней сделки.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubscribeLastPriceResponse {
    /// Уникальный идентификатор запроса, подробнее: \[tracking_id\](<https://tinkoff.github.io/investAPI/grpc#tracking-id>).
    #[prost(string, tag = "1")]
    pub tracking_id: ::prost::alloc::string::String,
    /// Массив статусов подписки на цену последней сделки.
    #[prost(message, repeated, tag = "2")]
    pub last_price_subscriptions: ::prost::alloc::vec::Vec<LastPriceSubscription>,
}
/// Статус подписки на цену последней сделки.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LastPriceSubscription {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Статус подписки.
    #[prost(enumeration = "SubscriptionStatus", tag = "2")]
    pub subscription_status: i32,
    /// Uid инструмента
    #[prost(string, tag = "3")]
    pub instrument_uid: ::prost::alloc::string::String,
}
/// Пакет свечей в рамках стрима.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Candle {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Интервал свечи.
    #[prost(enumeration = "SubscriptionInterval", tag = "2")]
    pub interval: i32,
    /// Цена открытия за 1 инструмент. Для получения стоимости лота требуется умножить на лотность инструмента. Для перевод цен в валюту рекомендуем использовать [информацию со страницы](<https://tinkoff.github.io/investAPI/faq_marketdata/>)
    #[prost(message, optional, tag = "3")]
    pub open: ::core::option::Option<Quotation>,
    /// Максимальная цена за 1 инструмент. Для получения стоимости лота требуется умножить на лотность инструмента. Для перевод цен в валюту рекомендуем использовать [информацию со страницы](<https://tinkoff.github.io/investAPI/faq_marketdata/>)
    #[prost(message, optional, tag = "4")]
    pub high: ::core::option::Option<Quotation>,
    /// Минимальная цена за 1 инструмент. Для получения стоимости лота требуется умножить на лотность инструмента. Для перевод цен в валюту рекомендуем использовать [информацию со страницы](<https://tinkoff.github.io/investAPI/faq_marketdata/>)
    #[prost(message, optional, tag = "5")]
    pub low: ::core::option::Option<Quotation>,
    /// Цена закрытия за 1 инструмент. Для получения стоимости лота требуется умножить на лотность инструмента. Для перевод цен в валюту рекомендуем использовать [информацию со страницы](<https://tinkoff.github.io/investAPI/faq_marketdata/>)
    #[prost(message, optional, tag = "6")]
    pub close: ::core::option::Option<Quotation>,
    /// Объём сделок в лотах.
    #[prost(int64, tag = "7")]
    pub volume: i64,
    /// Время начала интервала свечи в часовом поясе UTC.
    #[prost(message, optional, tag = "8")]
    pub time: ::core::option::Option<::prost_types::Timestamp>,
    /// Время последней сделки, вошедшей в свечу в часовом поясе UTC.
    #[prost(message, optional, tag = "9")]
    pub last_trade_ts: ::core::option::Option<::prost_types::Timestamp>,
    /// Uid инструмента
    #[prost(string, tag = "10")]
    pub instrument_uid: ::prost::alloc::string::String,
}
/// Пакет стаканов в рамках стрима.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OrderBook {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Глубина стакана.
    #[prost(int32, tag = "2")]
    pub depth: i32,
    /// Флаг консистентности стакана. **false** значит не все заявки попали в стакан по причинам сетевых задержек или нарушения порядка доставки.
    #[prost(bool, tag = "3")]
    pub is_consistent: bool,
    /// Массив предложений.
    #[prost(message, repeated, tag = "4")]
    pub bids: ::prost::alloc::vec::Vec<Order>,
    /// Массив спроса.
    #[prost(message, repeated, tag = "5")]
    pub asks: ::prost::alloc::vec::Vec<Order>,
    /// Время формирования стакана в часовом поясе UTC по времени биржи.
    #[prost(message, optional, tag = "6")]
    pub time: ::core::option::Option<::prost_types::Timestamp>,
    /// Верхний лимит цены за 1 инструмент. Для получения стоимости лота требуется умножить на лотность инструмента. Для перевод цен в валюту рекомендуем использовать [информацию со страницы](<https://tinkoff.github.io/investAPI/faq_marketdata/>)
    #[prost(message, optional, tag = "7")]
    pub limit_up: ::core::option::Option<Quotation>,
    /// Нижний лимит цены за 1 инструмент. Для получения стоимости лота требуется умножить на лотность инструмента. Для перевод цен в валюту рекомендуем использовать [информацию со страницы](<https://tinkoff.github.io/investAPI/faq_marketdata/>)
    #[prost(message, optional, tag = "8")]
    pub limit_down: ::core::option::Option<Quotation>,
    /// Uid инструмента
    #[prost(string, tag = "9")]
    pub instrument_uid: ::prost::alloc::string::String,
}
/// Массив предложений/спроса.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Order {
    /// Цена за 1 инструмент. Для получения стоимости лота требуется умножить на лотность инструмента. Для перевод цен в валюту рекомендуем использовать [информацию со страницы](<https://tinkoff.github.io/investAPI/faq_marketdata/>)
    #[prost(message, optional, tag = "1")]
    pub price: ::core::option::Option<Quotation>,
    /// Количество в лотах.
    #[prost(int64, tag = "2")]
    pub quantity: i64,
}
/// Информация о сделке.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Trade {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Направление сделки.
    #[prost(enumeration = "TradeDirection", tag = "2")]
    pub direction: i32,
    /// Цена за 1 инструмент. Для получения стоимости лота требуется умножить на лотность инструмента. Для перевод цен в валюту рекомендуем использовать [информацию со страницы](<https://tinkoff.github.io/investAPI/faq_marketdata/>)
    #[prost(message, optional, tag = "3")]
    pub price: ::core::option::Option<Quotation>,
    /// Количество лотов.
    #[prost(int64, tag = "4")]
    pub quantity: i64,
    /// Время сделки в часовом поясе UTC по времени биржи.
    #[prost(message, optional, tag = "5")]
    pub time: ::core::option::Option<::prost_types::Timestamp>,
    /// Uid инструмента
    #[prost(string, tag = "6")]
    pub instrument_uid: ::prost::alloc::string::String,
}
/// Пакет изменения торгового статуса.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TradingStatus {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Статус торговли инструментом.
    #[prost(enumeration = "SecurityTradingStatus", tag = "2")]
    pub trading_status: i32,
    /// Время изменения торгового статуса в часовом поясе UTC.
    #[prost(message, optional, tag = "3")]
    pub time: ::core::option::Option<::prost_types::Timestamp>,
    /// Признак доступности выставления лимитной заявки по инструменту.
    #[prost(bool, tag = "4")]
    pub limit_order_available_flag: bool,
    /// Признак доступности выставления рыночной заявки по инструменту.
    #[prost(bool, tag = "5")]
    pub market_order_available_flag: bool,
    /// Uid инструмента
    #[prost(string, tag = "6")]
    pub instrument_uid: ::prost::alloc::string::String,
}
/// Запрос исторических свечей.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetCandlesRequest {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Начало запрашиваемого периода в часовом поясе UTC.
    #[prost(message, optional, tag = "2")]
    pub from: ::core::option::Option<::prost_types::Timestamp>,
    /// Окончание запрашиваемого периода в часовом поясе UTC.
    #[prost(message, optional, tag = "3")]
    pub to: ::core::option::Option<::prost_types::Timestamp>,
    /// Интервал запрошенных свечей.
    #[prost(enumeration = "CandleInterval", tag = "4")]
    pub interval: i32,
    /// Идентификатор инструмента, принимает значение figi или instrument_uid
    #[prost(string, tag = "5")]
    pub instrument_id: ::prost::alloc::string::String,
}
/// Список свечей.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetCandlesResponse {
    /// Массив свечей.
    #[prost(message, repeated, tag = "1")]
    pub candles: ::prost::alloc::vec::Vec<HistoricCandle>,
}
/// Информация о свече.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct HistoricCandle {
    /// Цена открытия за 1 инструмент. Для получения стоимости лота требуется умножить на лотность инструмента. Для перевод цен в валюту рекомендуем использовать [информацию со страницы](<https://tinkoff.github.io/investAPI/faq_marketdata/>)
    #[prost(message, optional, tag = "1")]
    pub open: ::core::option::Option<Quotation>,
    /// Максимальная цена за 1 инструмент. Для получения стоимости лота требуется умножить на лотность инструмента. Для перевод цен в валюту рекомендуем использовать [информацию со страницы](<https://tinkoff.github.io/investAPI/faq_marketdata/>)
    #[prost(message, optional, tag = "2")]
    pub high: ::core::option::Option<Quotation>,
    /// Минимальная цена за 1 инструмент. Для получения стоимости лота требуется умножить на лотность инструмента. Для перевод цен в валюту рекомендуем использовать [информацию со страницы](<https://tinkoff.github.io/investAPI/faq_marketdata/>)
    #[prost(message, optional, tag = "3")]
    pub low: ::core::option::Option<Quotation>,
    /// Цена закрытия за 1 инструмент. Для получения стоимости лота требуется умножить на лотность инструмента. Для перевод цен в валюту рекомендуем использовать [информацию со страницы](<https://tinkoff.github.io/investAPI/faq_marketdata/>)
    #[prost(message, optional, tag = "4")]
    pub close: ::core::option::Option<Quotation>,
    /// Объём торгов в лотах.
    #[prost(int64, tag = "5")]
    pub volume: i64,
    /// Время свечи в часовом поясе UTC.
    #[prost(message, optional, tag = "6")]
    pub time: ::core::option::Option<::prost_types::Timestamp>,
    /// Признак завершённости свечи. **false** значит, свеча за текущие интервал ещё сформирована не полностью.
    #[prost(bool, tag = "7")]
    pub is_complete: bool,
}
/// Запрос получения цен последних сделок.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetLastPricesRequest {
    /// Массив figi-идентификаторов инструментов.
    #[prost(string, repeated, tag = "1")]
    pub figi: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    /// Массив идентификаторов инструмента, принимает значения figi или instrument_uid
    #[prost(string, repeated, tag = "2")]
    pub instrument_id: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
/// Список цен последних сделок.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetLastPricesResponse {
    /// Массив цен последних сделок.
    #[prost(message, repeated, tag = "1")]
    pub last_prices: ::prost::alloc::vec::Vec<LastPrice>,
}
/// Информация о цене последней сделки.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct LastPrice {
    /// Figi инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Цена последней сделки за 1 инструмент. Для получения стоимости лота требуется умножить на лотность инструмента. Для перевод цен в валюту рекомендуем использовать [информацию со страницы](<https://tinkoff.github.io/investAPI/faq_marketdata/>)
    #[prost(message, optional, tag = "2")]
    pub price: ::core::option::Option<Quotation>,
    /// Время получения последней цены в часовом поясе UTC по времени биржи.
    #[prost(message, optional, tag = "3")]
    pub time: ::core::option::Option<::prost_types::Timestamp>,
    /// Uid инструмента
    #[prost(string, tag = "11")]
    pub instrument_uid: ::prost::alloc::string::String,
}
/// Запрос стакана.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetOrderBookRequest {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Глубина стакана.
    #[prost(int32, tag = "2")]
    pub depth: i32,
    /// Идентификатор инструмента, принимает значение figi или instrument_uid
    #[prost(string, tag = "3")]
    pub instrument_id: ::prost::alloc::string::String,
}
/// Информация о стакане.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetOrderBookResponse {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Глубина стакана.
    #[prost(int32, tag = "2")]
    pub depth: i32,
    /// Множество пар значений на покупку.
    #[prost(message, repeated, tag = "3")]
    pub bids: ::prost::alloc::vec::Vec<Order>,
    /// Множество пар значений на продажу.
    #[prost(message, repeated, tag = "4")]
    pub asks: ::prost::alloc::vec::Vec<Order>,
    /// Цена последней сделки за 1 инструмент. Для получения стоимости лота требуется умножить на лотность инструмента. Для перевод цен в валюту рекомендуем использовать [информацию со страницы](<https://tinkoff.github.io/investAPI/faq_marketdata/>)
    #[prost(message, optional, tag = "5")]
    pub last_price: ::core::option::Option<Quotation>,
    /// Цена закрытия за 1 инструмент. Для получения стоимости лота требуется умножить на лотность инструмента. Для перевод цен в валюту рекомендуем использовать [информацию со страницы](<https://tinkoff.github.io/investAPI/faq_marketdata/>)
    #[prost(message, optional, tag = "6")]
    pub close_price: ::core::option::Option<Quotation>,
    /// Верхний лимит цены за 1 инструмент. Для получения стоимости лота требуется умножить на лотность инструмента. Для перевод цен в валюту рекомендуем использовать [информацию со страницы](<https://tinkoff.github.io/investAPI/faq_marketdata/>)
    #[prost(message, optional, tag = "7")]
    pub limit_up: ::core::option::Option<Quotation>,
    /// Нижний лимит цены за 1 инструмент. Для получения стоимости лота требуется умножить на лотность инструмента. Для перевод цен в валюту рекомендуем использовать [информацию со страницы](<https://tinkoff.github.io/investAPI/faq_marketdata/>)
    #[prost(message, optional, tag = "8")]
    pub limit_down: ::core::option::Option<Quotation>,
    /// Время получения цены последней сделки.
    #[prost(message, optional, tag = "21")]
    pub last_price_ts: ::core::option::Option<::prost_types::Timestamp>,
    /// Время получения цены закрытия.
    #[prost(message, optional, tag = "22")]
    pub close_price_ts: ::core::option::Option<::prost_types::Timestamp>,
    /// Время формирования стакана на бирже.
    #[prost(message, optional, tag = "23")]
    pub orderbook_ts: ::core::option::Option<::prost_types::Timestamp>,
    /// Uid инструмента
    #[prost(string, tag = "9")]
    pub instrument_uid: ::prost::alloc::string::String,
}
/// Запрос получения торгового статуса.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetTradingStatusRequest {
    /// Идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Идентификатор инструмента, принимает значение figi или instrument_uid
    #[prost(string, tag = "2")]
    pub instrument_id: ::prost::alloc::string::String,
}
/// Информация о торговом статусе.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetTradingStatusResponse {
    /// Figi-идентификатор инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Статус торговли инструментом.
    #[prost(enumeration = "SecurityTradingStatus", tag = "2")]
    pub trading_status: i32,
    /// Признак доступности выставления лимитной заявки по инструменту.
    #[prost(bool, tag = "3")]
    pub limit_order_available_flag: bool,
    /// Признак доступности выставления рыночной заявки по инструменту.
    #[prost(bool, tag = "4")]
    pub market_order_available_flag: bool,
    /// Признак доступности торгов через API.
    #[prost(bool, tag = "5")]
    pub api_trade_available_flag: bool,
    /// Uid инструмента
    #[prost(string, tag = "6")]
    pub instrument_uid: ::prost::alloc::string::String,
}
/// Запрос обезличенных сделок за последний час.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetLastTradesRequest {
    /// Figi-идентификатор инструмента
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Начало запрашиваемого периода в часовом поясе UTC.
    #[prost(message, optional, tag = "2")]
    pub from: ::core::option::Option<::prost_types::Timestamp>,
    /// Окончание запрашиваемого периода в часовом поясе UTC.
    #[prost(message, optional, tag = "3")]
    pub to: ::core::option::Option<::prost_types::Timestamp>,
    /// Идентификатор инструмента, принимает значение figi или instrument_uid
    #[prost(string, tag = "4")]
    pub instrument_id: ::prost::alloc::string::String,
}
/// Обезличенных сделок за последний час.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetLastTradesResponse {
    /// Массив сделок
    #[prost(message, repeated, tag = "1")]
    pub trades: ::prost::alloc::vec::Vec<Trade>,
}
/// Запрос активных подписок.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetMySubscriptions {}
/// Запрос цен закрытия торговой сессии по инструментам.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetClosePricesRequest {
    /// Массив по инструментам.
    #[prost(message, repeated, tag = "1")]
    pub instruments: ::prost::alloc::vec::Vec<InstrumentClosePriceRequest>,
}
/// Запрос цен закрытия торговой сессии по инструменту.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InstrumentClosePriceRequest {
    /// Идентификатор инструмента, принимает значение figi или instrument_uid
    #[prost(string, tag = "1")]
    pub instrument_id: ::prost::alloc::string::String,
}
/// Цены закрытия торговой сессии по инструментам.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetClosePricesResponse {
    /// Массив по инструментам.
    #[prost(message, repeated, tag = "1")]
    pub close_prices: ::prost::alloc::vec::Vec<InstrumentClosePriceResponse>,
}
/// Цена закрытия торговой сессии по инструменту.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InstrumentClosePriceResponse {
    /// Figi инструмента.
    #[prost(string, tag = "1")]
    pub figi: ::prost::alloc::string::String,
    /// Uid инструмента.
    #[prost(string, tag = "2")]
    pub instrument_uid: ::prost::alloc::string::String,
    /// Цена закрытия торговой сессии.
    #[prost(message, optional, tag = "11")]
    pub price: ::core::option::Option<Quotation>,
    /// Дата совершения торгов.
    #[prost(message, optional, tag = "21")]
    pub time: ::core::option::Option<::prost_types::Timestamp>,
}
/// Тип операции со списком подписок.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum SubscriptionAction {
    /// Статус подписки не определён.
    Unspecified = 0,
    /// Подписаться.
    Subscribe = 1,
    /// Отписаться.
    Unsubscribe = 2,
}
impl SubscriptionAction {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            SubscriptionAction::Unspecified => "SUBSCRIPTION_ACTION_UNSPECIFIED",
            SubscriptionAction::Subscribe => "SUBSCRIPTION_ACTION_SUBSCRIBE",
            SubscriptionAction::Unsubscribe => "SUBSCRIPTION_ACTION_UNSUBSCRIBE",
        }
    }
}
/// Интервал свечи.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum SubscriptionInterval {
    /// Интервал свечи не определён.
    Unspecified = 0,
    /// Минутные свечи.
    OneMinute = 1,
    /// Пятиминутные свечи.
    FiveMinutes = 2,
}
impl SubscriptionInterval {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            SubscriptionInterval::Unspecified => "SUBSCRIPTION_INTERVAL_UNSPECIFIED",
            SubscriptionInterval::OneMinute => "SUBSCRIPTION_INTERVAL_ONE_MINUTE",
            SubscriptionInterval::FiveMinutes => "SUBSCRIPTION_INTERVAL_FIVE_MINUTES",
        }
    }
}
/// Результат подписки.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum SubscriptionStatus {
    /// Статус подписки не определён.
    Unspecified = 0,
    /// Успешно.
    Success = 1,
    /// Инструмент не найден.
    InstrumentNotFound = 2,
    /// Некорректный статус подписки, список возможных значений: \[SubscriptionAction\](<https://tinkoff.github.io/investAPI/marketdata#subscriptionaction>).
    SubscriptionActionIsInvalid = 3,
    /// Некорректная глубина стакана, доступные значения: 1, 10, 20, 30, 40, 50.
    DepthIsInvalid = 4,
    /// Некорректный интервал свечей, список возможных значений: \[SubscriptionInterval\](<https://tinkoff.github.io/investAPI/marketdata#subscriptioninterval>).
    IntervalIsInvalid = 5,
    /// Превышен лимит на общее количество подписок в рамках стрима, подробнее: [Лимитная политика](<https://tinkoff.github.io/investAPI/limits/>).
    LimitIsExceeded = 6,
    /// Внутренняя ошибка сервиса.
    InternalError = 7,
    /// Превышен лимит на количество запросов на подписки в течение установленного отрезка времени
    TooManyRequests = 8,
}
impl SubscriptionStatus {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            SubscriptionStatus::Unspecified => "SUBSCRIPTION_STATUS_UNSPECIFIED",
            SubscriptionStatus::Success => "SUBSCRIPTION_STATUS_SUCCESS",
            SubscriptionStatus::InstrumentNotFound => {
                "SUBSCRIPTION_STATUS_INSTRUMENT_NOT_FOUND"
            }
            SubscriptionStatus::SubscriptionActionIsInvalid => {
                "SUBSCRIPTION_STATUS_SUBSCRIPTION_ACTION_IS_INVALID"
            }
            SubscriptionStatus::DepthIsInvalid => "SUBSCRIPTION_STATUS_DEPTH_IS_INVALID",
            SubscriptionStatus::IntervalIsInvalid => {
                "SUBSCRIPTION_STATUS_INTERVAL_IS_INVALID"
            }
            SubscriptionStatus::LimitIsExceeded => {
                "SUBSCRIPTION_STATUS_LIMIT_IS_EXCEEDED"
            }
            SubscriptionStatus::InternalError => "SUBSCRIPTION_STATUS_INTERNAL_ERROR",
            SubscriptionStatus::TooManyRequests => {
                "SUBSCRIPTION_STATUS_TOO_MANY_REQUESTS"
            }
        }
    }
}
/// Направление сделки.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum TradeDirection {
    /// Направление сделки не определено.
    Unspecified = 0,
    /// Покупка.
    Buy = 1,
    /// Продажа.
    Sell = 2,
}
impl TradeDirection {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            TradeDirection::Unspecified => "TRADE_DIRECTION_UNSPECIFIED",
            TradeDirection::Buy => "TRADE_DIRECTION_BUY",
            TradeDirection::Sell => "TRADE_DIRECTION_SELL",
        }
    }
}
/// Интервал свечей.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum CandleInterval {
    /// Интервал не определён.
    Unspecified = 0,
    /// 1 минута.
    CandleInterval1Min = 1,
    /// 5 минут.
    CandleInterval5Min = 2,
    /// 15 минут.
    CandleInterval15Min = 3,
    /// 1 час.
    Hour = 4,
    /// 1 день.
    Day = 5,
}
impl CandleInterval {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            CandleInterval::Unspecified => "CANDLE_INTERVAL_UNSPECIFIED",
            CandleInterval::CandleInterval1Min => "CANDLE_INTERVAL_1_MIN",
            CandleInterval::CandleInterval5Min => "CANDLE_INTERVAL_5_MIN",
            CandleInterval::CandleInterval15Min => "CANDLE_INTERVAL_15_MIN",
            CandleInterval::Hour => "CANDLE_INTERVAL_HOUR",
            CandleInterval::Day => "CANDLE_INTERVAL_DAY",
        }
    }
}
/// Generated client implementations.
pub mod market_data_service_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct MarketDataServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl MarketDataServiceClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> MarketDataServiceClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> MarketDataServiceClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + Send + Sync,
        {
            MarketDataServiceClient::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        /// Метод запроса исторических свечей по инструменту.
        pub async fn get_candles(
            &mut self,
            request: impl tonic::IntoRequest<super::GetCandlesRequest>,
        ) -> Result<tonic::Response<super::GetCandlesResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.MarketDataService/GetCandles",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод запроса цен последних сделок по инструментам.
        pub async fn get_last_prices(
            &mut self,
            request: impl tonic::IntoRequest<super::GetLastPricesRequest>,
        ) -> Result<tonic::Response<super::GetLastPricesResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.MarketDataService/GetLastPrices",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод получения стакана по инструменту.
        pub async fn get_order_book(
            &mut self,
            request: impl tonic::IntoRequest<super::GetOrderBookRequest>,
        ) -> Result<tonic::Response<super::GetOrderBookResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.MarketDataService/GetOrderBook",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод запроса статуса торгов по инструментам.
        pub async fn get_trading_status(
            &mut self,
            request: impl tonic::IntoRequest<super::GetTradingStatusRequest>,
        ) -> Result<tonic::Response<super::GetTradingStatusResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.MarketDataService/GetTradingStatus",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод запроса обезличенных сделок за последний час.
        pub async fn get_last_trades(
            &mut self,
            request: impl tonic::IntoRequest<super::GetLastTradesRequest>,
        ) -> Result<tonic::Response<super::GetLastTradesResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.MarketDataService/GetLastTrades",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Метод запроса цен закрытия торговой сессии по инструментам.
        pub async fn get_close_prices(
            &mut self,
            request: impl tonic::IntoRequest<super::GetClosePricesRequest>,
        ) -> Result<tonic::Response<super::GetClosePricesResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.MarketDataService/GetClosePrices",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
    }
}
/// Generated client implementations.
pub mod market_data_stream_service_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct MarketDataStreamServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl MarketDataStreamServiceClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> MarketDataStreamServiceClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> MarketDataStreamServiceClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + Send + Sync,
        {
            MarketDataStreamServiceClient::new(
                InterceptedService::new(inner, interceptor),
            )
        }
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        /// Bi-directional стрим предоставления биржевой информации.
        pub async fn market_data_stream(
            &mut self,
            request: impl tonic::IntoStreamingRequest<Message = super::MarketDataRequest>,
        ) -> Result<
            tonic::Response<tonic::codec::Streaming<super::MarketDataResponse>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.MarketDataStreamService/MarketDataStream",
            );
            self.inner.streaming(request.into_streaming_request(), path, codec).await
        }
        /// Server-side стрим предоставления биржевой информации.
        pub async fn market_data_server_side_stream(
            &mut self,
            request: impl tonic::IntoRequest<super::MarketDataServerSideStreamRequest>,
        ) -> Result<
            tonic::Response<tonic::codec::Streaming<super::MarketDataResponse>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/tinkoff.public.invest.api.contract.v1.MarketDataStreamService/MarketDataServerSideStream",
            );
            self.inner.server_streaming(request.into_request(), path, codec).await
        }
    }
}
