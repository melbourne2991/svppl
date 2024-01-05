use tonic::transport::Server;

use super::task_service::TaskService;
use super::proto::task_server::TaskServer;

pub fn run_server() {
    Server::builder()
        .add_service(TaskServer::new(TaskService::default()));
        

}

