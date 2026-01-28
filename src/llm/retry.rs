use crate::llm::error::LlmError;
use reqwest::Response as HttpResponse;
use reqwest::header::RETRY_AFTER;
use std::time::{Duration, SystemTime};
use tokio::time::sleep;
use tracing::{debug, warn};

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub jitter_ratio: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 4,
            base_delay: Duration::from_millis(800),
            max_delay: Duration::from_secs(60),
            jitter_ratio: 0.2,
        }
    }
}

pub async fn with_retry<T, Fut>(
    provider: &'static str,
    cfg: &RetryConfig,
    mut op: impl FnMut() -> Fut,
) -> Result<T, LlmError>
where
    Fut: std::future::Future<Output = Result<T, LlmError>>,
{
    let mut attempt: u32 = 1;

    loop {
        match op().await {
            Ok(v) => return Ok(v),
            Err(err) => {
                let is_retryable = err.is_retryable();
                let can_retry = attempt < cfg.max_attempts && is_retryable;

                if !can_retry {
                    if !is_retryable {
                        debug!(
                            provider = provider,
                            error_kind = ?err.kind,
                            "Non-retryable error"
                        );
                    }
                    return Err(err);
                }

                let delay = compute_delay(attempt, cfg, err.retry_after);

                warn!(
                    provider = provider,
                    attempt = attempt,
                    max_attempts = cfg.max_attempts,
                    delay_ms = delay.as_millis() as u64,
                    error_kind = ?err.kind,
                    "Retrying after error"
                );

                if let Some(raw) = &err.raw {
                    debug!(provider = provider, raw = %raw, "Raw error details");
                }

                sleep(delay).await;
                attempt += 1;
            }
        }
    }
}

fn compute_delay(attempt: u32, cfg: &RetryConfig, server_hint: Option<Duration>) -> Duration {
    let base_delay = if let Some(hint) = server_hint {
        hint.min(cfg.max_delay)
    } else {
        let exp = 2u32.saturating_pow(attempt.saturating_sub(1));
        cfg.base_delay.saturating_mul(exp).min(cfg.max_delay)
    };

    add_jitter(base_delay, cfg.jitter_ratio)
}

fn add_jitter(delay: Duration, jitter_ratio: f64) -> Duration {
    if jitter_ratio <= 0.0 {
        return delay;
    }

    let max_extra = (delay.as_millis() as f64 * jitter_ratio) as u64;
    if max_extra == 0 {
        return delay;
    }

    let extra_ms = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64
        % (max_extra + 1);

    delay + Duration::from_millis(extra_ms)
}

pub fn parse_retry_after_header(resp: &HttpResponse) -> Option<Duration> {
    let value = resp.headers().get(RETRY_AFTER)?.to_str().ok()?.trim();

    if let Ok(secs) = value.parse::<u64>() {
        return Some(Duration::from_secs(secs));
    }

    if let Ok(secs) = value.parse::<f64>()
        && secs.is_finite()
        && secs > 0.0
    {
        return Some(Duration::from_secs_f64(secs));
    }

    parse_http_date(value)
}

fn parse_http_date(value: &str) -> Option<Duration> {
    use chrono::{DateTime, NaiveDateTime, Utc};

    if let Ok(parsed) = DateTime::parse_from_rfc2822(value) {
        let now = Utc::now();
        let diff = parsed.signed_duration_since(now);
        if diff.num_seconds() > 0 {
            return Some(Duration::from_secs(diff.num_seconds() as u64));
        }
        return None;
    }

    let formats = ["%a, %d %b %Y %H:%M:%S GMT", "%A, %d-%b-%y %H:%M:%S GMT"];

    for fmt in &formats {
        if let Ok(naive) = NaiveDateTime::parse_from_str(value, fmt) {
            let parsed = naive.and_utc();
            let now = Utc::now();
            let diff = parsed.signed_duration_since(now);
            if diff.num_seconds() > 0 {
                return Some(Duration::from_secs(diff.num_seconds() as u64));
            }
        }
    }

    None
}

pub fn parse_retry_from_message(message: &str) -> Option<Duration> {
    let lower = message.to_lowercase();

    if let Some(idx) = lower.find("try again in ") {
        let tail = &lower[idx + "try again in ".len()..];

        let num_end = tail
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(tail.len());
        let num_str = tail[..num_end].trim();

        if let Ok(num) = num_str.parse::<f64>()
            && num.is_finite()
            && num > 0.0
        {
            let unit_start = num_end;
            let unit_tail = tail[unit_start..].trim_start();

            let multiplier = if unit_tail.starts_with("ms") {
                1.0
            } else if unit_tail.starts_with('s') || unit_tail.starts_with("second") {
                1000.0
            } else if unit_tail.starts_with('m') && !unit_tail.starts_with("ms") {
                60_000.0
            } else {
                1000.0
            };

            return Some(Duration::from_millis((num * multiplier) as u64));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_retry_from_message_seconds() {
        let msg = "Rate limit reached. Please try again in 45.622s.";
        let dur = parse_retry_from_message(msg).unwrap();
        assert!(dur.as_millis() >= 45000 && dur.as_millis() <= 46000);
    }

    #[test]
    fn test_parse_retry_from_message_ms() {
        let msg = "Try again in 500ms";
        let dur = parse_retry_from_message(msg).unwrap();
        assert_eq!(dur.as_millis(), 500);
    }

    #[test]
    fn test_parse_retry_from_message_none() {
        let msg = "Something went wrong";
        assert!(parse_retry_from_message(msg).is_none());
    }
}
