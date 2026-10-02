use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordVerifier};
use axum::Json;
use axum::extract::Request;
use axum::http::{HeaderMap, Method, StatusCode, Uri, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use uuid::Uuid;

use crate::ErrorBody;

const SESSION_AGE: Duration = Duration::from_secs(60 * 60 * 24);
const ATTEMPT_WINDOW: Duration = Duration::from_secs(60 * 5);
const MAX_FAILURES: usize = 5;

#[derive(Clone)]
pub(crate) struct WebAuth {
    token: String,
    password_hash: Option<String>,
    state: Arc<Mutex<AuthState>>,
    secure_cookie: bool,
    reveal_password: Option<String>,
}

#[derive(Default)]
struct AuthState {
    sessions: HashMap<String, Instant>,
    failures: Vec<Instant>,
}

#[derive(Deserialize)]
pub(crate) struct LoginIn {
    password: String,
}

impl WebAuth {
    pub(crate) fn new(
        token: String,
        password_hash: Option<String>,
        secure_cookie: bool,
        reveal_password: Option<String>,
    ) -> Self {
        Self {
            token,
            password_hash,
            state: Arc::new(Mutex::new(AuthState::default())),
            secure_cookie,
            reveal_password,
        }
    }

    pub(crate) fn reveal(&self) -> Response {
        let Some(password) = &self.reveal_password else {
            return StatusCode::NOT_FOUND.into_response();
        };
        Json(serde_json::json!({ "password": password })).into_response()
    }

    pub(crate) async fn login(&self, headers: HeaderMap, Json(body): Json<LoginIn>) -> Response {
        if headers.get(header::ORIGIN).is_none()
            || !same_origin(&headers)
            || (self.secure_cookie && !https_origin(&headers))
        {
            return StatusCode::FORBIDDEN.into_response();
        }
        let Some(hash) = &self.password_hash else {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        };
        {
            let mut state = self.state.lock().unwrap();
            state
                .failures
                .retain(|time| time.elapsed() < ATTEMPT_WINDOW);
            if state.failures.len() >= MAX_FAILURES {
                return StatusCode::TOO_MANY_REQUESTS.into_response();
            }
        }
        // Argon2 verification is CPU-intensive; never block the async executor.
        let hash = hash.clone();
        let valid = tokio::task::spawn_blocking(move || {
            PasswordHash::new(&hash).is_ok_and(|parsed| {
                Argon2::default()
                    .verify_password(body.password.as_bytes(), &parsed)
                    .is_ok()
            })
        })
        .await
        .unwrap_or(false);
        let mut state = self.state.lock().unwrap();
        state
            .failures
            .retain(|time| time.elapsed() < ATTEMPT_WINDOW);
        if state.failures.len() >= MAX_FAILURES {
            return StatusCode::TOO_MANY_REQUESTS.into_response();
        }
        if !valid {
            state.failures.push(Instant::now());
            return unauthorized();
        }
        state.failures.clear();
        let session = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        state
            .sessions
            .retain(|_, expires| *expires > Instant::now());
        state
            .sessions
            .insert(session.clone(), Instant::now() + SESSION_AGE);
        let secure = if self.secure_cookie { "; Secure" } else { "" };
        (
            [(
                header::SET_COOKIE,
                format!(
                    "ira_session={session}; HttpOnly; SameSite=Strict; Path=/api; Max-Age={}{}",
                    SESSION_AGE.as_secs(),
                    secure
                ),
            )],
            Json(serde_json::json!({ "ok": true })),
        )
            .into_response()
    }

    pub(crate) async fn logout(&self, headers: HeaderMap) -> Response {
        if let Some(id) = session_id(&headers) {
            self.state.lock().unwrap().sessions.remove(id);
        }
        let secure = if self.secure_cookie { "; Secure" } else { "" };
        (
            [(
                header::SET_COOKIE,
                format!(
                    "ira_session=; HttpOnly; SameSite=Strict; Path=/api; Max-Age=0{}",
                    secure
                ),
            )],
            Json(serde_json::json!({ "ok": true })),
        )
            .into_response()
    }

    pub(crate) async fn authorize(&self, req: Request, next: Next) -> Response {
        let path = req.uri().path();
        if !path.starts_with("/api/") || (req.method() == Method::GET && path == "/api/health") {
            return next.run(req).await;
        }
        if path == "/api/auth/login" && req.method() == Method::POST {
            return next.run(req).await;
        }
        if path == "/api/auth/password"
            && req.method() == Method::GET
            && self.reveal_password.is_some()
        {
            return next.run(req).await;
        }
        let bearer = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .is_some_and(|v| token_eq(v.trim(), &self.token));
        let cookie = session_id(req.headers()).is_some_and(|id| {
            self.state
                .lock()
                .unwrap()
                .sessions
                .get(id)
                .is_some_and(|expires| *expires > Instant::now())
        });
        if !bearer && !cookie {
            return unauthorized();
        }
        if !bearer
            && !matches!(*req.method(), Method::GET | Method::HEAD)
            && (req.headers().get(header::ORIGIN).is_none()
                || !same_origin(req.headers())
                || (self.secure_cookie && !https_origin(req.headers())))
        {
            return StatusCode::FORBIDDEN.into_response();
        }
        next.run(req).await
    }
}

fn session_id(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.split(';')
                .find_map(|part| part.trim().strip_prefix("ira_session="))
        })
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(ErrorBody {
            error: "no autorizado".into(),
        }),
    )
        .into_response()
}

// Compare browser Origin with Host; never trust client-controlled X-Forwarded-*.
fn same_origin(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return true;
    };
    let (Some(host), Ok(origin)) = (
        headers.get(header::HOST).and_then(|v| v.to_str().ok()),
        origin.to_str().unwrap_or("").parse::<Uri>(),
    ) else {
        return false;
    };
    matches!(origin.scheme_str(), Some("http" | "https"))
        && origin
            .authority()
            .is_some_and(|authority| authority.as_str().eq_ignore_ascii_case(host))
        && origin.path() == "/"
        && origin.query().is_none()
}

fn https_origin(headers: &HeaderMap) -> bool {
    headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<Uri>().ok())
        .is_some_and(|uri| uri.scheme_str() == Some("https"))
}

fn token_eq(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right) {
        diff |= a ^ b;
    }
    diff == 0
}
