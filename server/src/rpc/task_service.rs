use super::proto::{self, task_server::Task};

#[derive(Default)]
pub struct TaskService {}

#[tonic::async_trait]
impl Task for TaskService {
    async fn schedule_task(
        &self,
        request: tonic::Request<proto::ScheduleTaskRequest>,
    ) -> Result<tonic::Response<proto::ScheduleTaskReply>, tonic::Status> {
        println!("Got a request: {:?}", request);
        let response = proto::ScheduleTaskReply {
            success: true,
            task_id: None,
        };

        Ok(tonic::Response::new(response))
    }
}
