use std::{convert::Infallible, net::SocketAddr};

// use tonic::transport::{Server};
use futures::StreamExt;
use tower::ServiceBuilder;

use crate::cluster_monitor;

use super::partition_router::PartitionRoutingLayer;
use super::proto::task_server::TaskServer;
use super::task_service::TaskService;
use hyper::{service::make_service_fn, Server};
use tower::Service;

pub fn start(listen_addr: SocketAddr, cluster_monitor: &cluster_monitor::ClusterMonitor) {
    let task_service = TaskService::default();
    let tonic = TaskServer::new(task_service);

    Server::bind(&listen_addr).serve(make_service_fn(move |_| {
        let mut core = ServiceBuilder::new()
        .layer(PartitionRoutingLayer::default())
        .service(tonic.clone());

        std::future::ready(Ok::<_, Infallible>(tower::service_fn(move |req: hyper::Request<hyper::Body>| {
            core.call(req)
        })))
    }));


}
