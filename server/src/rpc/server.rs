use std::net::SocketAddr;

use tonic::transport::Server;
use tower::steer::Steer;
use tower::ServiceBuilder;
use futures::StreamExt;

use crate::cluster_monitor;

use super::task_service::TaskService;
use super::proto::task_server::TaskServer;

pub fn start(listen_addr: SocketAddr, cluster_monitor: &cluster_monitor::ClusterMonitor) {

    let task_service = TaskService::default();

    let wrapped = ServiceBuilder::new()
        
        .service(task_service);

    Server::builder()
        .add_service(TaskServer::new(wrapped))
        .serve(listen_addr);

    // let steer = Steer::new();
}

