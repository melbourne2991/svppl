use super::common::{TaskData, TaskId, TaskProcessor, TaskQueue};
use anyhow::Result;
use async_trait::async_trait;

use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use sqlx::{Executor, QueryBuilder, Row};
pub struct PersistencePostgres {
    pool: Pool<Postgres>,
}

impl PersistencePostgres {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    pub async fn initialize_tables(&self) -> Result<()> {
        let mut tx = self.pool.begin().await?; // Start a transaction

        // Create the table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS svppl_task (
                seq_id BIGSERIAL NOT NULL,
                queue_id TEXT NOT NULL,
                partition_id SMALLINT NOT NULL,
                payload BYTEA NOT NULL,
                status SMALLINT NOT NULL,
                scheduled_at BIGINT NOT NULL DEFAULT 0,
                deadline_at BIGINT,
                PRIMARY KEY (queue_id, partition_id, seq_id)
            );
            "#,
        )
        .execute(&mut *tx)
        .await?;

        // Create the first index
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS svppl_idx_task_status ON svppl_task(status);
            "#,
        )
        .execute(&mut *tx)
        .await?;

        // Create the second index
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS svppl_idx_task_scheduled_at ON svppl_task(scheduled_at);
            "#,
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?; // Commit the transaction

        Ok(())
    }
}

#[async_trait]
impl TaskQueue for PersistencePostgres {
    async fn enqueue_tasks(
        &self,
        queue_id: &str,
        partition_id: i16,
        payloads: Vec<&[u8]>,
    ) -> Result<Vec<TaskId>> {
        let mut conn = self.pool.acquire().await?;

        let mut query_builder =
            QueryBuilder::new("INSERT INTO svppl_task (queue_id, partition_id, payload, status) ");

        query_builder.push_values(payloads, |mut b, payload| {
            b.push_bind(queue_id)
                .push_bind(partition_id)
                .push_bind(payload)
                .push_bind(0);
        });

        query_builder.push("RETURNING seq_id");

        let query = query_builder.build();
        let fetched = conn.fetch_all(query).await;

        match fetched {
            Ok(rows) => {
                let mapped_vec: anyhow::Result<Vec<TaskId>> = rows
                    .into_iter()
                    .map(|row| {
                        let maybe_seq_id: sqlx::Result<i64, sqlx::Error> = row.try_get(0);

                        maybe_seq_id
                            .map_err(|err| anyhow::Error::new(err))
                            .map(|seq_id| TaskId::from_parts(queue_id, partition_id, seq_id))
                    })
                    .collect();

                mapped_vec
            }
            Err(err) => Err(anyhow::Error::new(err)),
        }
    }

    async fn process_tasks<T: TaskProcessor>(
        &self,
        queue_id: &str,
        partition_id: i16,
        count: i64,
        task_processor: &T,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        let rows = sqlx::query(
            r#"
            SELECT seq_id, payload, scheduled_at, deadline_at
            FROM svppl_task
            WHERE queue_id = $1
            AND partition_id = $2
            AND status = 0
            ORDER BY seq_id ASC, scheduled_at ASC
            LIMIT $3
            SKIP LOCKED
            FOR UPDATE;
            "#,
        )
        .bind(queue_id)
        .bind(partition_id)
        .bind(count)
        .fetch_all(&mut *tx)
        .await?;

        let mut futures = Vec::new();

        for row in rows {
            let seq_id: i64 = row.try_get(0)?;
            let payload: Vec<u8> = row.try_get(1)?;
            let scheduled_at: i64 = row.try_get(2)?;
            let deadline_at: Option<i64> = row.try_get(3)?;

            let future = task_processor.process_task(TaskData {
                task_id: TaskId::from_parts(queue_id, partition_id, seq_id),
                payload: payload,
                scheduled_at,
                deadline_at,
            });

            futures.push(future);
        }

        let results = futures::future::join_all(futures).await;

        for result in results {
            result?; // Handle each result or error
        }

        tx.commit().await?;

        Ok(())
    }

    async fn query_tasks(
        &self,
        queue_id: &str,
        partition_id: i16,
        status: i16,
        count: i64,
    ) -> Result<Vec<TaskData>> {
        let rows = sqlx::query(
            r#"
            SELECT seq_id, payload, scheduled_at, deadline_at
            FROM svppl_task
            WHERE queue_id = $1
            AND partition_id = $2
            AND status = $3
            ORDER BY seq_id ASC, scheduled_at ASC
            LIMIT $4
            "#,
        )
        .bind(queue_id)
        .bind(partition_id)
        .bind(status)
        .bind(count)
        .fetch_all(&self.pool)
        .await?;

        let mut tasks = Vec::new();

        for row in rows {
            let seq_id: i64 = row.try_get(0)?;
            let payload: Vec<u8> = row.try_get(1)?;
            let scheduled_at: i64 = row.try_get(2)?;
            let deadline_at: Option<i64> = row.try_get(3)?;

            tasks.push(TaskData {
                task_id: TaskId::from_parts(queue_id, partition_id, seq_id),
                payload: payload,
                scheduled_at,
                deadline_at,
            });
        }

        Ok(tasks)
    }
}

pub async fn create_connection_pool(url: &str) -> Result<sqlx::PgPool> {
    let pool = PgPoolOptions::new().max_connections(5).connect(url).await?;

    Ok(pool)
}
