mod api_exercise;
mod api_league;
mod api_pay;
mod api_profile;
mod api_review;
mod api_words;
mod error;
mod solve;
mod user;
mod webhook;

use crate::config::Config;
use crate::settings::Settings;
use crate::telegram::{Bot, Texts};
use axum::Router;
use axum::http::{HeaderValue, header};
use axum::routing::{get, post};
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
    pub settings: Arc<Settings>,
    pub bot: Arc<Bot>,
    pub texts: Arc<Texts>,
}

pub fn router(state: AppState) -> Router {
    let audio = ServeDir::new(state.config.data_dir.join("audio"));
    let web_dir = state.config.web_dir.clone();
    let api = Router::new()
        .route("/me", get(api_profile::me).patch(api_profile::update_me))
        .route("/stats", get(api_profile::stats))
        .route("/stats/pro", get(api_league::pro_stats))
        .route("/league", get(api_league::current))
        .route("/badges", get(api_league::badges))
        .route("/map", get(api_profile::map))
        .route("/lessons/{id}/next", post(api_exercise::next_in_lesson))
        .route("/attempts/{id}/play", post(api_exercise::play))
        .route("/attempts/{id}/check", post(api_exercise::check))
        .route("/attempts/{id}/reveal", post(api_exercise::reveal))
        .route("/review/next", post(api_review::next_review))
        .route("/challenge/today", get(api_review::challenge_today))
        .route("/challenge/next", post(api_review::next_challenge))
        .route("/words", get(api_words::list))
        .route("/words/{word}/save", post(api_words::save))
        .route("/pay/stars/{plan}", post(api_pay::stars_invoice));
    Router::new()
        .route("/health", get(health))
        .route("/tg/{path}", post(webhook::receive))
        .nest("/api", api)
        .nest_service(
            "/audio",
            tower::ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::overriding(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=31536000, immutable"),
                ))
                .service(audio),
        )
        .fallback_service(
            tower::ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::overriding(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("no-cache"),
                ))
                .service(ServeDir::new(&web_dir).append_index_html_on_directories(true)),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

#[cfg(test)]
mod bot_tests;

#[cfg(test)]
mod engagement_tests;

#[cfg(test)]
mod flow_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    pub fn test_config() -> Config {
        Config::from_vars(
            [
                ("BOT_TOKEN", "123456:TEST-TOKEN"),
                ("PUBLIC_URL", "https://example.com"),
                ("BIND_ADDR", "127.0.0.1:0"),
                ("DATABASE_URL", "postgresql://x:x@127.0.0.1:1/x"),
                ("WEBHOOK_PATH", "path"),
                ("WEBHOOK_SECRET", "secret"),
                ("ADSGRAM_REWARD_SECRET", "reward"),
            ]
            .map(|(k, v)| (k.to_owned(), v.to_owned())),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let state = AppState {
            pool: sqlx::postgres::PgPoolOptions::new()
                .connect_lazy("postgresql://x:x@127.0.0.1:1/x")
                .unwrap(),
            config: Arc::new(test_config()),
            settings: Arc::new(Settings::empty()),
            bot: Arc::new(Bot::new("1:x", "http://127.0.0.1:1").unwrap()),
            texts: Arc::new(Texts::load().unwrap()),
        };
        let res = router(state)
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = axum::body::to_bytes(res.into_body(), 64).await.unwrap();
        assert_eq!(&body[..], b"ok");
    }

    #[tokio::test]
    async fn api_requires_authorization() {
        let state = AppState {
            pool: sqlx::postgres::PgPoolOptions::new()
                .connect_lazy("postgresql://x:x@127.0.0.1:1/x")
                .unwrap(),
            config: Arc::new(test_config()),
            settings: Arc::new(Settings::empty()),
            bot: Arc::new(Bot::new("1:x", "http://127.0.0.1:1").unwrap()),
            texts: Arc::new(Texts::load().unwrap()),
        };
        let res = router(state)
            .oneshot(Request::get("/api/me").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }
}
