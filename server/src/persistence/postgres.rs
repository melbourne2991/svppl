use anyhow::Result;
use futures::Future;
use sqlx::Row;
use sqlx::{postgres::PgPoolOptions, Acquire, Pool, Postgres};

pub struct PostgresPersistence {
    pool: Pool<Postgres>,
}

impl PostgresPersistence {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    pub async fn initialize_tables(&self) -> Result<()> {
        let mut tx = self.pool.begin().await?; // Start a transaction
    
        // Create the table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS svppl_task (
                id BIGSERIAL NOT NULL,
                queue_id BYTEA NOT NULL,
                partition_id SMALLINT NOT NULL,
                payload BYTEA NOT NULL,
                status SMALLINT NOT NULL,
                scheduled_at BIGINT NOT NULL DEFAULT 0,
                deadline_at BIGINT NOT NULL DEFAULT 0,
                PRIMARY KEY (queue_id, partition_id, id)
            );
            "#
        )
        .execute(&mut *tx)
        .await?;
    
        // Create the first index
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS svppl_idx_task_status ON svppl_task(status);
            "#
        )
        .execute(&mut *tx)
        .await?;
    
        // Create the second index
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS svppl_idx_task_scheduled_at ON svppl_task(scheduled_at);
            "#
        )
        .execute(&mut *tx)
        .await?;
    
        tx.commit().await?; // Commit the transaction
    
        Ok(())
    }
    
    pub async fn handle_next_tasks<F, Fut>(
        &self,
        queue_id: &str,
        partition_id: i16,
        count: i64,
        callback: F,
    ) -> Result<()>
    where
        F: Fn(Vec<u8>) -> Fut,
        Fut: Future<Output = Result<()>>,
    {
        let mut tx = self.pool.begin().await?;

        let rows = sqlx::query(
            r#"
            SELECT payload
            FROM svppl_task
            WHERE queue_id = $1
            AND partition_id = $2
            AND status = 0
            ORDER BY id ASC, scheduled_at ASC
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
            let payload: Vec<u8> = row.try_get(0)?;
            let future = callback(payload);
            futures.push(future);
        }

        let results = futures::future::join_all(futures).await;

        for result in results {
            result?; // Handle each result or error
        }

        tx.commit().await?;

        Ok(())
    }

    pub async fn batch_insert_task(
        &self,
        queue_id: &[u8],
        partition_id: i16,
        payloads: Vec<&[u8]>,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        for payload in payloads {
            PostgresPersistence::tx_insert_task(&mut tx, queue_id, partition_id, payload).await?;
        }

        tx.commit().await?;

        Ok(())
    }

    pub async fn insert_task(
        &self,
        queue_id: &[u8],
        partition_id: i16,
        payload: &[u8],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        PostgresPersistence::tx_insert_task(&mut tx, queue_id, partition_id, payload).await?;

        tx.commit().await?;

        Ok(())
    }

    async fn tx_insert_task(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        queue_id: &[u8],
        partition_id: i16,
        payload: &[u8],
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO svppl_task (queue_id, partition_id, payload, status)
            VALUES ($1, $2, $3, 0)
            "#,
        )
        .bind(queue_id)
        .bind(partition_id)
        .bind(payload)
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    pub async fn query_tasks(
        &self,
        queue_id: &[u8],
        partition_id: i16,
        status: i16,
        count: i64,
    ) -> Result<Vec<Vec<u8>>> {
        let rows = sqlx::query(
            r#"
            SELECT payload
            FROM svppl_task
            WHERE queue_id = $1
            AND partition_id = $2
            AND status = $3
            ORDER BY id ASC, scheduled_at ASC
            LIMIT $4
            "#,
        )
        .bind(queue_id)
        .bind(partition_id)
        .bind(status)
        .bind(count)
        .fetch_all(&self.pool)
        .await?;

        let mut payloads = Vec::new();

        for row in rows {
            let payload: Vec<u8> = row.try_get(0)?;
            payloads.push(payload);
        }

        Ok(payloads)
    }
    
}

pub async fn create_connection_pool(url: &str) -> Result<sqlx::PgPool> {
    let pool = PgPoolOptions::new().max_connections(5).connect(url).await?;

    Ok(pool)
}
