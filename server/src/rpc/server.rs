use futures::{Future, FutureExt};
use tracing::info;
use tracing::span;
use tracing::Instrument;
use tracing::Level;

use std::{convert::Infallible, net::SocketAddr};
use tokio::sync::oneshot;
use tower::ServiceBuilder;

use crate::cluster_monitor;

use super::partition_router::PartitionChannelStore;
use super::partition_router::PartitionRoutingLayer;
use super::proto::task_server::TaskServer;
use super::task_service::TaskService;
use hyper::{service::make_service_fn, Server};
use tokio_stream::StreamExt;
use tower::Service;

pub struct RpcServerHandle {
    tx_shutdown: Option<oneshot::Sender<()>>,
    shutdown_complete: Option<Box<dyn Future<Output = ()> + Send + Unpin>>,
}

impl RpcServerHandle {
    fn new(
        tx_shutdown: oneshot::Sender<()>,
        shutdown_complete: impl Future<Output = ()> + Send + Unpin + 'static,
    ) -> Self {
        Self {
            tx_shutdown: Some(tx_shutdown),
            shutdown_complete: Some(Box::new(shutdown_complete)),
        }
    }

    pub async fn shutdown(&mut self) {
        if let Some(tx) = self.tx_shutdown.take() {
            tx.send(())
                .unwrap_or_else(|_| println!("Failed to send shutdown signal"));
        }

        if let Some(f) = self.shutdown_complete.take() {
            f.await;
        }
    }
}

pub async fn start(
    listen_addr: SocketAddr,
    cluster_monitor: &mut cluster_monitor::ClusterMonitor,
) -> RpcServerHandle {
    let task_service = TaskService::default();
    let task_server = TaskServer::new(task_service);

    let own_key = cluster_monitor.self_key().await;

    let mut channel_store = PartitionChannelStore::new(own_key);
    let mut changes = cluster_monitor.watch().await;

    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

    let channel_store_ = channel_store.clone();

    let channel_store_sync_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => {
                    break;
                },

                Some(change) = changes.next() => {
                    let result = channel_store.sync(&change).await;

                    if let Err(err) = result {
                        tracing::error!(err = ?err, "channel_store_sync_error");
                    }
                }
            }
        }
    });

    let on_channel_store_end = channel_store_sync_handle.map(|result| {
        if let Err(err) = result {
            tracing::error!(err = ?err, "on_channel_store_end_error");
        }
    });

    let shutdown_complete = Server::bind(&listen_addr)
        .serve(make_service_fn(move |_| {
            let mut core = ServiceBuilder::new()
                .layer(PartitionRoutingLayer::new(channel_store_.clone()))
                .service(task_server.clone());

            std::future::ready(Ok::<_, Infallible>(tower::service_fn(
                move |req: hyper::Request<hyper::Body>| {
                    let span = span!(
                        Level::INFO,
                        "rpc",
                        method = ?req.method(),
                        uri = ?req.uri(),
                        headers = ?req.headers()
                    );

                    info!("rpc_request_received");

                    core.call(req).instrument(span)
                },
            )))
        }))
        .with_graceful_shutdown(on_channel_store_end);

    let handle = RpcServerHandle::new(
        shutdown_tx,
        shutdown_complete.map(|result| {
            if let Err(err) = result {
                tracing::error!(err = ?err, "shutdown_complete_error");
            }
        }),
    );

    return handle;
}
