

struct QueueItem {
    id: String,
    data: String,
}

trait QueueStore {
    type Item;
    type Error;

    fn get_batch(&self, count: u64) -> Result<Option<String>, Error>;

    fn complete(&self, key: &str) -> Result<(), Error>;
}