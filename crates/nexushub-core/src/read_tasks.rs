//! Bound native read work independently of Tokio's worker and blocking pools.
use std::sync::{Arc, LazyLock};
use tokio::sync::Semaphore;

static READERS: LazyLock<Arc<Semaphore>> = LazyLock::new(|| {
    Arc::new(Semaphore::new(
        std::thread::available_parallelism()
            .map_or(2, usize::from)
            .clamp(2, 4),
    ))
});

pub async fn spawn_blocking<F, R>(read: F) -> Result<R, tokio::task::JoinError>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let permit = READERS
        .clone()
        .acquire_owned()
        .await
        .expect("read semaphore remains open");
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        read()
    })
    .await
}
