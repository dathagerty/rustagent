use reqwest::StatusCode;
use std::{borrow::Cow, fmt, time::Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    RateLimited,
    Transient,
    Auth,
    BadRequest,
    Unknown,
}

#[derive(Debug)]
pub struct LlmError {
    pub provider: &'static str,
    pub kind: ErrorKind,
    pub status: Option<StatusCode>,
    pub retry_after: Option<Duration>,
    pub user_message: Cow<'static, str>,
    pub raw: Option<String>,
}

impl fmt::Display for LlmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.user_message)
    }
}

impl std::error::Error for LlmError {}

impl LlmError {
    pub fn rate_limited(
        provider: &'static str,
        retry_after: Option<Duration>,
        raw: Option<String>,
    ) -> Self {
        Self {
            provider,
            kind: ErrorKind::RateLimited,
            status: Some(StatusCode::TOO_MANY_REQUESTS),
            retry_after,
            user_message: "Rate limited by provider; retrying shortly.".into(),
            raw,
        }
    }

    pub fn transient(
        provider: &'static str,
        status: Option<StatusCode>,
        raw: Option<String>,
    ) -> Self {
        Self {
            provider,
            kind: ErrorKind::Transient,
            status,
            retry_after: None,
            user_message: "Temporary network/provider error.".into(),
            raw,
        }
    }

    pub fn auth(provider: &'static str, raw: Option<String>) -> Self {
        Self {
            provider,
            kind: ErrorKind::Auth,
            status: Some(StatusCode::UNAUTHORIZED),
            retry_after: None,
            user_message: "Authentication failed. Check API key.".into(),
            raw,
        }
    }

    pub fn bad_request(provider: &'static str, raw: Option<String>) -> Self {
        Self {
            provider,
            kind: ErrorKind::BadRequest,
            status: Some(StatusCode::BAD_REQUEST),
            retry_after: None,
            user_message: "Request rejected by provider.".into(),
            raw,
        }
    }

    pub fn network(provider: &'static str, raw: Option<String>) -> Self {
        Self {
            provider,
            kind: ErrorKind::Transient,
            status: None,
            retry_after: None,
            user_message: "Network error contacting provider.".into(),
            raw,
        }
    }

    pub fn from_status(
        provider: &'static str,
        status: StatusCode,
        body: String,
        retry_after: Option<Duration>,
    ) -> Self {
        let kind = classify_status(status);
        let user_message: Cow<'static, str> = match kind {
            ErrorKind::RateLimited => "Rate limited by provider; retrying shortly.".into(),
            ErrorKind::Transient => "Temporary provider error.".into(),
            ErrorKind::Auth => "Authentication failed. Check API key.".into(),
            ErrorKind::BadRequest => "Request rejected by provider.".into(),
            ErrorKind::Unknown => "Unexpected provider error.".into(),
        };

        Self {
            provider,
            kind,
            status: Some(status),
            retry_after,
            user_message,
            raw: Some(body),
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(self.kind, ErrorKind::RateLimited | ErrorKind::Transient)
    }
}

pub fn classify_status(status: StatusCode) -> ErrorKind {
    match status.as_u16() {
        429 => ErrorKind::RateLimited,
        401 | 403 => ErrorKind::Auth,
        408 | 425 => ErrorKind::Transient,
        400..=499 => ErrorKind::BadRequest,
        500..=599 => ErrorKind::Transient,
        _ => ErrorKind::Unknown,
    }
}
