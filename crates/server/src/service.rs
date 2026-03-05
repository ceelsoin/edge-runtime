use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;

use bytes::Bytes;
use http::{Request, Response};
use http_body_util::Full;

use crate::router::Router;

type BoxBody = Full<Bytes>;

/// Tower Service wrapper around our Router.
///
/// This implements `hyper::service::Service` for use with `hyper_util`'s connection builder.
#[derive(Clone)]
pub struct EdgeService {
    router: Router,
}

impl EdgeService {
    pub fn new(router: Router) -> Self {
        Self { router }
    }
}

impl hyper::service::Service<Request<hyper::body::Incoming>> for EdgeService {
    type Response = Response<BoxBody>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<hyper::body::Incoming>) -> Self::Future {
        let router = self.router.clone();
        Box::pin(async move { router.handle(req).await })
    }
}
