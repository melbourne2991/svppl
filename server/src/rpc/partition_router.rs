use std::{
    collections::HashMap,
    convert::Infallible,
    error::Error,
    io::Stderr,
    net::SocketAddr,
    task::{Context, Poll},
};

use crate::{
    cluster_monitor::{self, ClusterMonitor, ClusterNodeIdEq, ClusterState},
    partition_resolver::PartitionResolver,
};
use futures::{future::BoxFuture, StreamExt};
use tonic::body::BoxBody;
use tonic::transport::channel::{Channel, ResponseFuture};
use tonic::transport::Body;
use tower::Service;

use hyper::body::HttpBody;
// #[derive(Clone)]
pub struct PartitionRouter<S> {
    cluster_monitor: ClusterMonitor,
    partition_resolver: PartitionResolver,
    inner_service: S,
    channel_map: HashMap<ClusterNodeIdEq, (SocketAddr, Channel)>,
    cluster_state: ClusterState,
}

impl<S> PartitionRouter<S>
where
    S: Service<hyper::Request<Body>>,
{
    pub fn new(cluster_monitor: &ClusterMonitor, inner_service: S) -> Self {
        Self {
            cluster_monitor: cluster_monitor.clone(),
            inner_service,
            partition_resolver: PartitionResolver::new(50),
            channel_map: HashMap::new(),
            cluster_state: ClusterState::default(),
        }
    }

    pub async fn start(&mut self) {
        let mut changes = self.cluster_monitor.watch().await;

        while let Some(cs) = changes.next().await {
            self.cluster_state = cs;
            self.partition_resolver.sync(&self.cluster_state).await;

            if self.sync_channels().await.is_err() {
                // TODO: log error
            }
        }
    }

    async fn sync_channels(&mut self) -> anyhow::Result<()> {
        for key in self.cluster_state.keys() {
            let node = self
                .cluster_state
                .get(&key)
                .ok_or_else(|| anyhow::anyhow!("node does not exist for key"))?;
            let latest_addr = node.grpc_addr()?;
            let node_eq_key = ClusterNodeIdEq(key);

            if let Some((addr, _)) = self.channel_map.get(&node_eq_key) {
                if latest_addr == *addr {
                    continue;
                }
            }

            let channel = self.init_channel(latest_addr);
            self.channel_map.insert(node_eq_key, (latest_addr, channel));
        }

        Ok(())
    }

    fn init_channel(&self, addr: SocketAddr) -> Channel {
        let channel = Channel::from_shared(addr.to_string())
            .unwrap()
            .connect_lazy();

        channel
    }

    pub fn get_channel(&self, key: &[u8]) -> Option<&Channel> {
        let node = self.partition_resolver.resolve(key)?;
        let node_eq_key = ClusterNodeIdEq(node.clone());
        self.channel_map
            .get(&node_eq_key)
            .map(|(_, channel)| channel)
    }
}

impl<S> Service<hyper::Request<Body>> for PartitionRouter<S>
where
    S: Service<hyper::Request<Body>, Response = hyper::Response<BoxBody>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    S::Error: Into<std::convert::Infallible> + Send + Sync + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner_service.poll_ready(cx)
    }

    fn call(&mut self, req: hyper::Request<Body>) -> Self::Future {
        let meta = tonic::metadata::MetadataMap::from_headers(req.headers().clone());
        let maybe_channel = if let Some(key) = meta.get("partition_key") {
            self.get_channel(key.as_bytes())
                .map(|channel| channel.clone())
        } else {
            None
        };

        let clone = self.inner_service.clone();
        let mut inner = std::mem::replace(&mut self.inner_service, clone);

        Box::pin(async move {
            let response = if let Some(mut channel) = maybe_channel {
                let (parts, body) = req.into_parts();

                let boxed = body
                    .map_err(|err| {
                        let err: Box<dyn Error + Send + Sync> = err.into();
                        tonic::Status::from_error(err)
                    })
                    .boxed_unsync();

                let req_boxed = hyper::Request::from_parts(parts, boxed);

                let res = channel.call(req_boxed).await;

                match res {
                    Ok(res) => {
                        let (parts, body) = res.into_parts();

                        let body = body
                            .map_err(|err| {
                                let err: Box<dyn Error + Send + Sync> = err.into();
                                tonic::Status::from_error(err)
                            })
                            .boxed_unsync();

                        let res_boxed = hyper::Response::from_parts(parts, body);

                        res_boxed
                    }
                    Err(err) => {
                        let err: Box<dyn Error + Send + Sync> = err.into();
                        let tonic_status = tonic::Status::from_error(err);

                        let body = hyper::Body::empty()
                            .map_err(move |_| tonic_status.clone())
                            .boxed_unsync();

                        hyper::Response::new(body)
                    }
                }
            } else {
                inner.call(req).await?
            };

            Ok(response)
        })
    }
}

#[derive(Clone)]
pub struct PartitionRoutingLayer {
    cluster_monitor: ClusterMonitor,
}

impl PartitionRoutingLayer {
    pub fn new(cluster_monitor: ClusterMonitor) -> Self {
        Self { cluster_monitor }
    }
}

impl<T> tower::Layer<T> for PartitionRoutingLayer
where
    T: Service<hyper::Request<Body>> + Clone + Send + 'static,
    T::Future: Send + 'static,
{
    type Service = PartitionRouter<T>;

    fn layer(&self, inner: T) -> Self::Service {
        PartitionRouter::new(&self.cluster_monitor, inner)
    }
}
