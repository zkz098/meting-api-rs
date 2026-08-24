use meting_server::{app_router, middleware::auth_middleware};
use axum::middleware;
use vercel_runtime::{run, Body, Error, Request, Response};

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(handler).await
}

async fn handler(req: Request) -> Result<Response<Body>, Error> {
    let app = app_router().layer(middleware::from_fn(auth_middleware));
    // Axum Router implements tower Service
    use tower::ServiceExt;
    let resp = app.oneshot(req).await.unwrap();
    Ok(resp)
}
