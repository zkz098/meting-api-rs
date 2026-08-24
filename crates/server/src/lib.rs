pub mod handlers;
pub mod middleware;
pub mod provider;

use axum::{
    routing::{get, post},
    Router,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

pub fn app_router() -> Router {
    Router::new()
        // health & docs
        .route("/v1/health", get(handlers::health))
        .route("/v1/openapi.json", get(handlers::openapi))
        .route("/docs", get(handlers::docs))
        // modern /v1
        .route("/v1/search", get(handlers::search))
        .route("/v1/songs/{id}", get(handlers::song))
        .route("/v1/songs/{id}/url", get(handlers::song_url))
        .route("/v1/songs/{id}/lyric", get(handlers::song_lyric))
        .route("/v1/songs/{id}/pic", get(handlers::song_pic))
        .route("/v1/playlists/{id}", get(handlers::playlist))
        .route("/v1/albums/{id}", get(handlers::album))
        .route("/v1/artists/{id}", get(handlers::artist))
        .route("/v1/songs/batch", post(handlers::batch))
        // legacy compat: GET /api?server=&type=&id=
        .route("/api", get(handlers::legacy_api))
        .route("/meting", get(handlers::legacy_api))
        .route("/", get(handlers::legacy_api))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}
