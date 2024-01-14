use futures::{Future, FutureExt};
use tokio::task::JoinHandle;
use tracing::span;
use tracing::Instrument;
use tracing::Level;

use std::{convert::Infallible, net::SocketAddr};
use tokio::sync::oneshot;
use tower::ServiceBuilder;

use crate::partition_resolver::PartitionResolver;

use super::partition_router::PartitionRoutingLayer;
use super::proto::task_server::TaskServer;
use super::task_service::TaskService;
use hyper::{service::make_service_fn, Server};
use tower::Service;

pub struct RpcServerHandle {
    tx_shutdown: oneshot::Sender<()>,
    shutdown_complete: Box<dyn Future<Output = ()> + Send + Unpin>,
}

impl RpcServerHandle {
    fn new(
        tx_shutdown: oneshot::Sender<()>,
        shutdown_complete: impl Future<Output = ()> + Send + Unpin + 'static,
    ) -> Self {
        Self {
            tx_shutdown,
            shutdown_complete: Box::new(shutdown_complete),
        }
    }

    pub async fn shutdown(self) -> anyhow::Result<()> {
        self.tx_shutdown.send(()).ok();
        self.shutdown_complete.await;

        Ok(())
    }
}

pub async fn start(
    listen_addr: SocketAddr,
    partition_resolver: PartitionResolver,
) -> RpcServerHandle {
    let task_service = TaskService::default();
    let task_server = TaskServer::new(task_service);

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    // let pr = partition_resolver.clone();

    let graceful = Server::bind(&listen_addr)
        .serve(make_service_fn(move |_| {
            let mut core = ServiceBuilder::new()
                .layer(PartitionRoutingLayer::new(partition_resolver.clone()))
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

                    tracing::info!("rpc_request_received");

                    core.call(req).instrument(span)
                },
            )))
        }))
        .with_graceful_shutdown(async {
            shutdown_rx.await.ok();
        });

    let shutdown_complete = Box::pin(graceful);

    let handle = RpcServerHandle::new(
        shutdown_tx,
        shutdown_complete.map(|result| {
            if let Err(err) = result {
                tracing::error!(err = ?err, "rpc_shutdown_error");
            }
        }),
    );

    return handle;
}
