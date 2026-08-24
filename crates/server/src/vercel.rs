//! Vercel Fluid adapter: `api/meting.rs` entry
//! Build with `--features vercel`
use axum::middleware;
use meting_server::{app_router, middleware::auth_middleware};
use vercel_runtime::{run, Error, Request, Response, Body};

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(handler).await
}

async fn handler(req: Request) -> Result<Response<Body>, Error> {
    let app = app_router().layer(middleware::from_fn(auth_middleware));
    // vercel_runtime expects tower Service; axum Router is one
    let resp = app.oneshot(req).await.unwrap();
    Ok(resp)
}

// for `api/meting.rs` style file-based function, also export service_fn variant
pub async fn vercel_handler(req: Request) -> Result<Response<Body>, Error> {
    handler(req).await
}
