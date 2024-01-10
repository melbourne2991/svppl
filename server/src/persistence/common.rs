use anyhow::Result;
use async_trait::async_trait;
use futures::Future;


#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskId {
    seq_id: i64,
    partition_id: i16,
    queue_id: String
}

impl TaskId {
    pub fn from_parts(queue_id: &str,  partition_id: i16, seq_id: i64) -> Self {
        Self {
            seq_id,
            partition_id,
            queue_id: queue_id.to_string()
        }
    }
}

pub type TaskPayload = Vec<u8>;

pub struct TaskData {
    task_id: TaskId,
    payload: TaskPayload,
    scheduled_at: i64,
    deadline_at: Option<i64>,
}

#[async_trait]
pub trait TaskQueue {
    async fn enqueue_tasks(
        &self,
        queue_id: &str,
        partition_id: i16,
        payloads: Vec<&[u8]>,
    ) -> Result<Vec<TaskId>>;

    async fn process_tasks<F, Fut>(
        &self,
        queue_id: &str,
        partition_id: i16,
        count: i64,
        callback: F,
    ) -> Result<()>
    where
        F: Fn(TaskData) -> Fut,
        Fut: Future<Output = Result<()>>;

    async fn query_tasks(
        &self,
        queue_id: &str,
        partition_id: i16,
        status: i16,
        count: i64,
    ) -> Result<Vec<TaskData>>;
}
