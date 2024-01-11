use crate::persistence::postgres_image::PostgresImage;
use anyhow::Result;
use nanoid::nanoid;
use server_lib::persistence::{common::TaskQueue,postgres::{self, PersistencePostgres}};
use testcontainers::{clients};


#[tokio::test]
async fn insert_and_query_tasks() -> Result<()> {
    let docker = clients::Cli::default();
    let postgres_image = PostgresImage::default();

    let node = docker.run(postgres_image);

    let connection_string = &format!(
        "postgres://postgres:postgres@127.0.0.1:{}/postgres",
        node.get_host_port_ipv4(5432)
    );

    let queue_id = nanoid!();

    let pool = postgres::create_connection_pool(connection_string).await?;
    let store = PersistencePostgres::new(pool);
    let payloads: Vec<String> = (0..10).map(|i| format!("payload {}", i)).collect();

    store.initialize_tables().await?;

    store
        .enqueue_tasks(
            queue_id.as_str(),
            0,
            payloads.iter().map(|s| s.as_bytes()).collect(),
        )
        .await?;


    let tasks = store.query_tasks(queue_id.as_str(), 0, 0, 10).await?;
    let first = tasks.first().unwrap();
    
    assert_eq!(tasks.len(), 10);
    assert_eq!(String::from_utf8(first.payload.clone())?, "payload 0");
    
    Ok(())
}
