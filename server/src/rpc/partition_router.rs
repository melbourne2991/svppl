use std::{
    collections::HashMap,
    convert::Infallible,
    error::Error,
    net::SocketAddr,
    sync::Arc,
    task::{Context, Poll},
};

use crate::{
    cluster_monitor::{ClusterNodeIdEq, ClusterNodeKey, ClusterStateChange},
    partition_resolver::PartitionResolver,
};

use futures::future::BoxFuture;
use hyper::body::HttpBody;
use tokio::sync::RwLock;
use tonic::body::BoxBody;
use tonic::transport::channel::Channel;
use tonic::transport::Body;
use tower::Service;
use tracing::{span, Instrument, Level};

pub struct PartitionRouter<S> {
    channel_store: PartitionChannelStore,
    inner_service: S,
}

impl<S> PartitionRouter<S>
where
    S: Service<hyper::Request<Body>>,
{
    pub fn new(channel_store: PartitionChannelStore, inner_service: S) -> Self {
        Self {
            channel_store: channel_store,
            inner_service,
        }
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
        let clone = self.inner_service.clone();
        let mut inner = std::mem::replace(&mut self.inner_service, clone);
        let channel_store = self.channel_store.clone();

        Box::pin(async move {
            let meta = tonic::metadata::MetadataMap::from_headers(req.headers().clone());
            let partition_key = meta.get("partition_key");

            let maybe_channel = if let Some(key) = partition_key {
                channel_store
                    .get_channel(key.as_bytes())
                    .await
                    .map(|channel| channel.clone())
            } else {
                None
            };

            let response = if let Some(mut channel) = maybe_channel {
                let (parts, body) = req.into_parts();

                let boxed = body
                    .map_err(|err| {
                        let err: Box<dyn Error + Send + Sync> = err.into();
                        tonic::Status::from_error(err)
                    })
                    .boxed_unsync();

                let req_boxed = hyper::Request::from_parts(parts, boxed);
                let res = channel
                    .call(req_boxed)
                    .instrument(span!(Level::INFO, "external_rpc", partition_key =? partition_key))
                    .await;

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
                inner
                    .call(req)
                    .instrument(span!(Level::INFO, "internal_rpc"))
                    .await?
            };

            Ok(response)
        })
    }
}

#[derive(Clone)]
pub struct PartitionRoutingLayer {
    channel_store: PartitionChannelStore,
}

impl PartitionRoutingLayer {
    pub fn new(channel_store: PartitionChannelStore) -> Self {
        Self { channel_store }
    }
}

impl<T> tower::Layer<T> for PartitionRoutingLayer
where
    T: Service<hyper::Request<Body>> + Clone + Send + 'static,
    T::Future: Send + 'static,
{
    type Service = PartitionRouter<T>;

    fn layer(&self, inner: T) -> Self::Service {
        PartitionRouter::new(self.channel_store.clone(), inner)
    }
}

#[derive(Clone)]
pub struct PartitionChannelStore {
    partition_resolver: Arc<RwLock<PartitionResolver>>,
    channel_map: Arc<RwLock<HashMap<ClusterNodeIdEq, (SocketAddr, Channel)>>>,
    self_node_id: ClusterNodeKey,
}

impl PartitionChannelStore {
    pub fn new(self_node_id: ClusterNodeKey) -> Self {
        Self {
            self_node_id,
            channel_map: Arc::new(RwLock::new(HashMap::new())),
            partition_resolver: Arc::new(RwLock::new(PartitionResolver::new(50, (0, 0)))),
        }
    }

    pub async fn sync(&mut self, change: &ClusterStateChange) -> anyhow::Result<()> {
        self.partition_resolver.write().await.sync(&change).await;

        for key in change.state.keys() {
            // skip self
            if key.eq_node_id(&self.self_node_id) {
                continue;
            }

            let node = change
                .state
                .get(&key)
                .ok_or_else(|| anyhow::anyhow!("node does not exist for key"))?;

            let node_eq_key = ClusterNodeIdEq(key.clone());
            let latest_addr = node.grpc_addr()?;

            if let Some((addr, _)) = self.channel_map.read().await.get(&node_eq_key) {
                if latest_addr == *addr {
                    continue;
                }
            }

            match self.init_channel(latest_addr) {
                Ok(channel) => {
                    self.channel_map
                        .write()
                        .await
                        .insert(node_eq_key, (latest_addr, channel));
                }
                Err(_e) => {
                    // TODO: log error
                }
            }
        }

        Ok(())
    }

    fn init_channel(&self, addr: SocketAddr) -> anyhow::Result<Channel> {
        let channel = Channel::from_shared(addr.to_string())
            .map_err(|e| anyhow::anyhow!("failed to create channel: {}", e))?
            .connect_lazy();

        Ok(channel)
    }

    pub async fn get_channel(&self, key: &[u8]) -> Option<Channel> {
        let resolver = self.partition_resolver.read().await;
        let node = resolver.resolve(key)?;
        let node_eq_key = ClusterNodeIdEq(node.clone());

        let cm = self.channel_map.read().await;

        cm.get(&node_eq_key).map(|(_, channel)| channel.clone())
    }
}
