use std::time::Duration;

use tokio::runtime::Runtime;
use tonic::{Request, Status};
use tonic::service::Interceptor;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::{Channel, Endpoint};

use super::TinkoffApiConfig;

mod api {
    include!("tinkoff.public.invest.api.contract.v1.rs");
}

use api::{
    instruments_service_client::InstrumentsServiceClient, InstrumentsRequest, SharesResponse,
    market_data_service_client::MarketDataServiceClient, GetLastPricesRequest,
};
use crate::core::{EmptyResult, GenericResult};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

pub struct Client {
    token: String,
    channel: Channel,
    runtime: Runtime,
}

impl Client {
    pub fn new(config: &TinkoffApiConfig) -> GenericResult<Client> {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;

        let channel = runtime.block_on(async {
            Channel::from_static("https://sandbox-invest-public-api.tinkoff.ru")
                .connect_timeout(CONNECT_TIMEOUT)
                .timeout(REQUEST_TIMEOUT)
                .connect_lazy()
        });

        Ok(Client {
            token: config.token.clone(),
            channel: channel,
            runtime: runtime,
        })
    }

    fn instruments(&self) -> InstrumentsServiceClient<InterceptedService<Channel, ClientInterceptor>> {
        InstrumentsServiceClient::with_interceptor(self.channel.clone(), ClientInterceptor::new(&self.token))
    }

    pub fn test(&self) -> EmptyResult {
        let shares = self.runtime.block_on(async {
            self.instruments().shares(InstrumentsRequest {
                ..Default::default()
            }).await
        })?;

        for instrument in &shares.get_ref().instruments {
            println!(">>> {}:{}:{}", instrument.ticker, instrument.class_code, instrument.exchange);
        }

        // let mut market_data = MarketDataServiceClient::with_interceptor(channel, interceptor);
        //
        // let request = GetLastPricesRequest {
        //     ..Default::default()
        // };
        //
        // let response = market_data.get_last_prices(request).await?;
        // Ok(())
        // let response = client.get_last_prices(request).await?;
        // println!("RESPONSE={:?}", response.get_ref().last_prices.len());
        // for price in &response.get_ref().last_prices {
        //     println!("{:?}", price.figi);
        // }

        Ok(())
    }
}

struct ClientInterceptor {
    token: String,
}

impl ClientInterceptor {
    fn new(token: &str) -> ClientInterceptor {
        ClientInterceptor {
            token: token.to_owned(),
        }
    }
}

impl Interceptor for ClientInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        let metadata = request.metadata_mut();
        metadata.insert("x-app-name", "KonishchevDmitry.investments".parse()?);
        metadata.insert("authorization", format!("Bearer {}", self.token).parse().unwrap());
        request.set_timeout(REQUEST_TIMEOUT);
        Ok(request)
    }
}