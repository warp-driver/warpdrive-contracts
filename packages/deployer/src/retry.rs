//! Generic async retry, mirroring `common.sh`'s `retry` semantics: re-invoke
//! the operation on `Err`, sleeping between attempts, up to `max_retries`
//! total attempts.

use std::future::Future;
use std::time::Duration;

/// Retry configuration. Defaults follow the shell deployer: 3 attempts, 5s
/// between them.
#[derive(Clone, Copy, Debug)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub sleep: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            sleep: Duration::from_secs(5),
        }
    }
}

impl RetryConfig {
    /// Read `MAX_RETRIES` / `RETRY_SLEEP_SECONDS` from the environment, falling
    /// back to the defaults when unset or unparseable.
    pub fn from_env() -> Self {
        let default = Self::default();
        let max_retries = std::env::var("MAX_RETRIES")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|&n| n >= 1)
            .unwrap_or(default.max_retries);
        let sleep = std::env::var("RETRY_SLEEP_SECONDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .map(Duration::from_secs)
            .unwrap_or(default.sleep);
        Self { max_retries, sleep }
    }
}

/// Run `op` until it succeeds or the attempt budget is exhausted. The closure
/// is re-invoked from scratch each attempt, so it should build any per-attempt
/// state (e.g. a fresh client) inside itself.
pub async fn retry<T, E, F, Fut>(cfg: RetryConfig, mut op: F) -> std::result::Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = std::result::Result<T, E>>,
    E: std::fmt::Display,
{
    let mut attempt = 0u32;
    loop {
        attempt += 1;
        match op().await {
            Ok(value) => return Ok(value),
            Err(err) if attempt < cfg.max_retries => {
                eprintln!(
                    "  attempt {attempt}/{} failed: {err}; retrying in {}s...",
                    cfg.max_retries,
                    cfg.sleep.as_secs()
                );
                tokio::time::sleep(cfg.sleep).await;
            }
            Err(err) => return Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[tokio::test]
    async fn retries_then_succeeds() {
        let cfg = RetryConfig {
            max_retries: 5,
            sleep: Duration::from_millis(0),
        };
        let calls = Cell::new(0u32);
        let result: Result<u32, String> = retry(cfg, || {
            calls.set(calls.get() + 1);
            let n = calls.get();
            async move {
                if n < 3 {
                    Err(format!("transient {n}"))
                } else {
                    Ok(n)
                }
            }
        })
        .await;
        assert_eq!(result, Ok(3));
        assert_eq!(calls.get(), 3);
    }

    #[tokio::test]
    async fn gives_up_after_budget() {
        let cfg = RetryConfig {
            max_retries: 2,
            sleep: Duration::from_millis(0),
        };
        let calls = Cell::new(0u32);
        let result: Result<(), String> = retry(cfg, || {
            calls.set(calls.get() + 1);
            async move { Err("always".to_string()) }
        })
        .await;
        assert!(result.is_err());
        assert_eq!(calls.get(), 2);
    }
}
