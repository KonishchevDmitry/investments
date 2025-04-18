use std::time::Duration;

use tonic::{Request, Status};
use tonic::service::Interceptor;

pub struct ClientInterceptor {
    token: String,
    request_timeout: Duration,
}

impl ClientInterceptor {
    pub fn new(token: &str, request_timeout: Duration) -> ClientInterceptor {
        ClientInterceptor {
            token: token.to_owned(),
            request_timeout,
        }
    }
}

impl Interceptor for ClientInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        let metadata = request.metadata_mut();

        metadata.insert("x-app-name", "KonishchevDmitry.investments".parse().map_err(|_|
            Status::invalid_argument("Invalid application name"))?);

        metadata.insert("authorization", format!("Bearer {}", self.token).parse().map_err(|_|
            Status::invalid_argument("Invalid token value"))?);

        request.set_timeout(self.request_timeout);
        Ok(request)
    }
}