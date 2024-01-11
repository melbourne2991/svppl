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

#[derive(Debug, Clone)]
pub struct TaskData {
    pub task_id: TaskId,
    pub payload: TaskPayload,
    pub scheduled_at: i64,
    pub deadline_at: Option<i64>,
}


#[async_trait]
pub trait TaskQueue {
    async fn enqueue_tasks(
        &self,
        queue_id: &str,
        partition_id: i16,
        payloads: Vec<&[u8]>,
    ) -> Result<Vec<TaskId>>;

    async fn process_tasks<T: TaskProcessor + Sync>(
        &self,
        queue_id: &str,
        partition_id: i16,
        count: i64,
        task_processor: &T,
    ) -> Result<()>;

    async fn query_tasks(
        &self,
        queue_id: &str,
        partition_id: i16,
        status: i16,
        count: i64,
    ) -> Result<Vec<TaskData>>;
}


#[async_trait]
pub trait TaskProcessor: Sync {
    async fn process_task(&self, task: TaskData) -> Result<()>;
}