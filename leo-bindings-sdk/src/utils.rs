use crate::error::Result;
use std::future::Future;
use std::time::{Duration, Instant};

pub async fn poll_until<F, Fut, T>(
    mut check_fn: F,
    timeout: Duration,
    initial_delay: Duration,
) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<Option<T>>>,
{
    let start = Instant::now();
    let mut delay = initial_delay;
    let max_delay = Duration::from_secs(5);

    loop {
        if start.elapsed() >= timeout {
            return Err(crate::error::Error::Config(
                "Polling timeout exceeded".to_string(),
            ));
        }

        match check_fn().await? {
            Some(result) => return Ok(result),
            None => {
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(max_delay);
            }
        }
    }
}
