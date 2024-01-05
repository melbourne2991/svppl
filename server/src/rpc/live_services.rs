use std::task::{Context, Poll};

use crate::{cluster_monitor::ClusterMonitor, partition_resolver::PartitionResolver};
use futures::{future::BoxFuture, StreamExt};
use hyper::body::{Body, Buf};
use tonic::body::BoxBody;
use tower::Service;

pub struct LiveServices<'a, S> {
    cluster_monitor: &'a mut ClusterMonitor,
    inner_service: S,
}

impl<'a, S> LiveServices<'a, S>
where
    S: Service<hyper::Request<BoxBody>>,
{
    pub fn new(cluster_monitor: &'a mut ClusterMonitor, inner_service: S) -> Self {
        Self {
            cluster_monitor,
            inner_service,
        }
    }

    pub async fn start(&mut self) {
        let mut changes = self.cluster_monitor.watch().await;

        while let Some(cs) = changes.next().await {}
    }
}

impl<'a, S> Service<hyper::Request<BoxBody>> for LiveServices<'a, S>
where
    S: Service<hyper::Request<BoxBody>, Response = hyper::Response<BoxBody>>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner_service.poll_ready(cx)
    }

    fn call(&mut self, req: hyper::Request<BoxBody>) -> Self::Future {
        let clone = self.inner_service.clone();
        let mut inner = std::mem::replace(&mut self.inner_service, clone);

        Box::pin(async move {
            let response = inner.call(req).await?;
            Ok(response)
        })
    }
}
