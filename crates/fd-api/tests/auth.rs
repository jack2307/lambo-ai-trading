//! The two listeners, driven the way a request actually arrives.
//!
//! Every case here goes through the whole `Router` with `oneshot` rather than
//! calling a handler directly, because the thing being tested is the layer in
//! front of the handlers and a test that calls the handler has stepped over
//! it. The unauthenticated router is checked in the same file and against the
//! same paths, because "8138 still behaves exactly as it did" is half the
//! requirement and the cheapest way to break it is to change a shared router
//! and only test the other one.

use std::path::Path;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use fd_api::auth::{Auth, COOKIE, StoredPassword};
use fd_api::{AppState, authed_router, router};
use fd_core::config::Config;
use fd_core::types::Bar;
use fd_store::write_bars;
use http_body_util::BodyExt;
use tower::ServiceExt;

const PASSWORD: &str = "a long enough desk password";

/// Stands in for the built SPA's `index.html`. Distinctive, so a test can say
/// "the app was served" rather than "something HTML was served".
const INDEX: &str = "<!doctype html><title>Backcom Desk</title><div id=root></div>";

/// Every mutating route on this API, plus the reads a stranger should not have
/// either. Written out rather than derived from the router: the point is to
/// notice when one is added, and a list generated from the router would agree
/// with the router by construction and notice nothing.
const GUARDED: [(&str, &str); 17] = [
    ("POST", "/api/paper/stop"),
    ("POST", "/api/paper/pause"),
    ("POST", "/api/paper/start"),
    ("POST", "/api/paper/open"),
    ("POST", "/api/paper/intent"),
    ("POST", "/api/paper/pending/act"),
    ("POST", "/api/paper/bar"),
    ("POST", "/api/paper/tick"),
    ("POST", "/api/paper/advice"),
    ("POST", "/api/paper/guards"),
    ("POST", "/api/advisor/credentials"),
    ("POST", "/api/advisor/credentials/test"),
    ("POST", "/api/advisor/credentials/clear"),
    ("POST", "/api/chart/backtest"),
    ("POST", "/api/chart/indicators"),
    ("GET", "/api/paper/status"),
    ("GET", "/api/version"),
];

fn config() -> Config {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("config");
    Config::load(dir).expect("the workspace config")
}

/// A state over an empty store, and a `ui/` directory beside it holding the
/// one file a built SPA must have. The UI branch of `router` is the one the
/// desk actually runs and the one with a catch-all fallback, so it is the one
/// worth testing: without a guard, `/anything` answers with the app.
fn fixture() -> (tempfile::TempDir, Arc<AppState>, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("a temp dir");
    write_bars(&dir.path().join("bars").join("BTCUSDT-15m.parquet"), &[Bar {
        time: 1_788_000_000_000,
        open: 1.0,
        high: 2.0,
        low: 0.5,
        close: 1.5,
        volume: Some(1.0),
    }])
    .expect("write bars");
    let ui = dir.path().join("ui");
    std::fs::create_dir_all(&ui).expect("ui dir");
    std::fs::write(ui.join("index.html"), INDEX).expect("index.html");
    let state = Arc::new(AppState::new(config(), dir.path().to_path_buf()));
    (dir, state, ui)
}

fn guard() -> Arc<Auth> {
    Arc::new(Auth::new(StoredPassword::create(PASSWORD).expect("a verifier")))
}

struct Answer {
    status: StatusCode,
    body: String,
    set_cookie: Option<String>,
    content_type: String,
}

async fn send(app: axum::Router, request: Request<Body>) -> Answer {
    let response = app.oneshot(request).await.expect("the router answers");
    let status = response.status();
    let set_cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let bytes = response.into_body().collect().await.expect("a body").to_bytes();
    Answer { status, body: String::from_utf8_lossy(&bytes).to_string(), set_cookie, content_type }
}

fn get(path: &str) -> Request<Body> {
    Request::builder().method("GET").uri(path).body(Body::empty()).expect("a request")
}

fn json(method: &str, path: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("a request")
}

/* ------------------------------------------------- the authenticated listener */

#[tokio::test]
async fn every_route_refuses_without_a_session() {
    let (_dir, state, ui) = fixture();
    let auth = guard();
    for (method, path) in GUARDED {
        let app = authed_router(Arc::clone(&state), Some(ui.clone()), Arc::clone(&auth));
        let request = if method == "GET" { get(path) } else { json(method, path, "{}") };
        let got = send(app, request).await;
        assert_eq!(
            got.status,
            StatusCode::UNAUTHORIZED,
            "{method} {path} answered {} without a session",
            got.status
        );
        // And in particular it did not DO the thing and then complain.
        assert!(got.body.contains("sign in first"), "{method} {path}: {}", got.body);
    }
}

#[tokio::test]
async fn a_path_that_does_not_exist_answers_exactly_like_one_that_does() {
    // The reason this matters: a 302 for a real route and a 404 for a typo is
    // a map of the API, handed to anyone who asks for it.
    let (_dir, state, ui) = fixture();
    let auth = guard();
    let real = send(
        authed_router(Arc::clone(&state), Some(ui.clone()), Arc::clone(&auth)),
        json("POST", "/api/paper/stop", "{}"),
    )
    .await;
    let typo = send(
        authed_router(Arc::clone(&state), Some(ui.clone()), Arc::clone(&auth)),
        json("POST", "/api/paper/stpo", "{}"),
    )
    .await;
    let nonsense = send(
        authed_router(Arc::clone(&state), Some(ui.clone()), Arc::clone(&auth)),
        json("POST", "/wp-admin/setup-config.php", "{}"),
    )
    .await;
    assert_eq!(real.status, StatusCode::UNAUTHORIZED);
    assert_eq!(typo.status, real.status);
    assert_eq!(nonsense.status, real.status);
    assert_eq!(typo.body, real.body);
    assert_eq!(nonsense.body, real.body);

    // A method the route does not take must not answer 405 either, which
    // would say just as much.
    let wrong_method =
        send(authed_router(Arc::clone(&state), Some(ui), Arc::clone(&auth)), get("/api/paper/stop"))
            .await;
    assert_eq!(wrong_method.status, StatusCode::UNAUTHORIZED, "a 405 is a disclosure too");
}

#[tokio::test]
async fn the_spa_itself_needs_a_session() {
    // The fallback serves index.html for every unknown path, which is what
    // makes client-side routing work — and what would serve the whole desk to
    // a stranger if the guard sat behind it instead of in front of it.
    let (_dir, state, ui) = fixture();
    let auth = guard();
    for path in ["/", "/index.html", "/desk", "/settings"] {
        let app = authed_router(Arc::clone(&state), Some(ui.clone()), Arc::clone(&auth));
        let request = Request::builder()
            .method("GET")
            .uri(path)
            .header(header::ACCEPT, "text/html,application/xhtml+xml")
            .body(Body::empty())
            .expect("a request");
        let got = send(app, request).await;
        assert_eq!(got.status, StatusCode::UNAUTHORIZED, "{path}");
        // A browser gets the login page as the body of the 401, not a
        // redirect and not a bare status line.
        assert!(got.content_type.starts_with("text/html"), "{path}: {}", got.content_type);
        assert!(got.body.contains("/api/auth/login"), "{path} did not get the login form");
        assert_ne!(got.body, INDEX, "{path} got the app instead of the login page");
    }
}

#[tokio::test]
async fn a_wrong_password_does_not_authenticate() {
    let (_dir, state, ui) = fixture();
    let auth = guard();
    let got = send(
        authed_router(Arc::clone(&state), Some(ui.clone()), Arc::clone(&auth)),
        json("POST", "/api/auth/login", r#"{"password":"not the password"}"#),
    )
    .await;
    assert_eq!(got.status, StatusCode::UNAUTHORIZED);
    assert!(got.set_cookie.is_none(), "a failed sign-in must not set a cookie");
    assert_eq!(auth.session_count(), 0);

    // An empty one, and a near miss.
    for attempt in [r#"{"password":""}"#, r#"{"password":"a long enough desk passwor"}"#] {
        let got = send(
            authed_router(Arc::clone(&state), Some(ui.clone()), Arc::clone(&auth)),
            json("POST", "/api/auth/login", attempt),
        )
        .await;
        assert_eq!(got.status, StatusCode::UNAUTHORIZED, "{attempt}");
        assert!(got.set_cookie.is_none(), "{attempt}");
    }
    assert_eq!(auth.session_count(), 0, "three failures, no sessions");
}

#[tokio::test]
async fn the_right_password_mints_a_cookie_that_works() {
    let (_dir, state, ui) = fixture();
    let auth = guard();
    let login = send(
        authed_router(Arc::clone(&state), Some(ui.clone()), Arc::clone(&auth)),
        json("POST", "/api/auth/login", &format!(r#"{{"password":"{PASSWORD}"}}"#)),
    )
    .await;
    assert_eq!(login.status, StatusCode::OK, "{}", login.body);
    let cookie = login.set_cookie.expect("a Set-Cookie");

    // Every flag, because each one is doing a job and a missing one is
    // invisible until it matters.
    assert!(cookie.starts_with(&format!("{COOKIE}=")), "{cookie}");
    assert!(cookie.contains("HttpOnly"), "script must not be able to read it: {cookie}");
    assert!(cookie.contains("SameSite=Strict"), "{cookie}");
    assert!(cookie.contains("Secure"), "{cookie}");
    assert!(cookie.contains("Path=/"), "{cookie}");

    // And the cookie the browser would send back gets through.
    let value = cookie.split(';').next().expect("the pair").to_string();
    let request = Request::builder()
        .method("GET")
        .uri("/api/version")
        .header(header::COOKIE, &value)
        .body(Body::empty())
        .expect("a request");
    let got = send(authed_router(Arc::clone(&state), Some(ui.clone()), Arc::clone(&auth)), request)
        .await;
    assert_eq!(got.status, StatusCode::OK, "{}", got.body);

    // A tampered cookie does not.
    let mut tampered = value.clone();
    tampered.pop();
    tampered.push(if value.ends_with('A') { 'B' } else { 'A' });
    let request = Request::builder()
        .method("GET")
        .uri("/api/version")
        .header(header::COOKIE, tampered)
        .body(Body::empty())
        .expect("a request");
    let got = send(authed_router(Arc::clone(&state), Some(ui), Arc::clone(&auth)), request).await;
    assert_eq!(got.status, StatusCode::UNAUTHORIZED, "one byte changed must be no session");
}

#[tokio::test]
async fn logging_out_puts_the_cookie_back() {
    let (_dir, state, ui) = fixture();
    let auth = guard();
    let login = send(
        authed_router(Arc::clone(&state), Some(ui.clone()), Arc::clone(&auth)),
        json("POST", "/api/auth/login", &format!(r#"{{"password":"{PASSWORD}"}}"#)),
    )
    .await;
    let value = login.set_cookie.expect("a cookie").split(';').next().expect("the pair").to_string();

    let out = send(
        authed_router(Arc::clone(&state), Some(ui.clone()), Arc::clone(&auth)),
        Request::builder()
            .method("POST")
            .uri("/api/auth/logout")
            .header(header::COOKIE, &value)
            .body(Body::empty())
            .expect("a request"),
    )
    .await;
    assert_eq!(out.status, StatusCode::OK);
    assert!(out.set_cookie.expect("a cleared cookie").contains("Max-Age=0"));

    let after = send(
        authed_router(Arc::clone(&state), Some(ui), Arc::clone(&auth)),
        Request::builder()
            .method("GET")
            .uri("/api/version")
            .header(header::COOKIE, value)
            .body(Body::empty())
            .expect("a request"),
    )
    .await;
    assert_eq!(after.status, StatusCode::UNAUTHORIZED, "the cookie is spent");
}

#[tokio::test]
async fn the_login_page_is_reachable_without_a_session() {
    // Otherwise there is no way in at all, which is a lockout rather than a
    // security property.
    let (_dir, state, ui) = fixture();
    let got = send(authed_router(state, Some(ui), guard()), get("/login")).await;
    assert_eq!(got.status, StatusCode::OK);
    assert!(got.content_type.starts_with("text/html"));
    assert!(got.body.contains("<form"));
}

/* ----------------------------------------------- the unauthenticated listener */

#[tokio::test]
async fn the_desks_own_port_still_answers_exactly_as_before() {
    // Nine ai_trader.py processes, mt5_executor.py and four mt5_bars.py
    // pollers call this port with no credentials of any kind. If any of these
    // became a 401 the desk would stop trading, and the symptom would be
    // silence rather than an error.
    let (_dir, state, ui) = fixture();
    for (method, path) in GUARDED {
        let app = router(Arc::clone(&state), Some(ui.clone()));
        let request = if method == "GET" { get(path) } else { json(method, path, "{}") };
        let got = send(app, request).await;
        assert_ne!(got.status, StatusCode::UNAUTHORIZED, "{method} {path} must not need a session");
        assert!(
            !got.body.contains("sign in first"),
            "{method} {path} reached the auth layer: {}",
            got.body
        );
    }
    // /api/version is the one with no arguments and no state to be missing,
    // so it is the one that can be asserted positively.
    let got = send(router(Arc::clone(&state), Some(ui.clone())), get("/api/version")).await;
    assert_eq!(got.status, StatusCode::OK, "{}", got.body);

    // And the SPA fallback still serves the app for a client-side route.
    let got = send(router(state, Some(ui)), get("/desk")).await;
    assert_eq!(got.status, StatusCode::OK);
    assert_eq!(got.body, INDEX, "the fallback still serves index.html");
}

#[tokio::test]
async fn the_unauthenticated_port_has_no_login_routes_at_all() {
    // They belong to the other listener. On this one `/login` is just a
    // client-side route the SPA fallback answers, and `/api/auth/login` must
    // not exist — a password endpoint on an unauthenticated port is a way to
    // brute-force the password with no lockout in front of it, since the
    // lockout lives in the layer this port does not have.
    let (_dir, state, ui) = fixture();
    let got = send(
        router(Arc::clone(&state), Some(ui.clone())),
        json("POST", "/api/auth/login", r#"{"password":"anything"}"#),
    )
    .await;
    assert_ne!(got.status, StatusCode::OK, "there must be nothing here to guess against");
    assert!(got.set_cookie.is_none());
}
