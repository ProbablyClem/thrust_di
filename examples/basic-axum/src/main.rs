mod controllers;
mod services;

use services::*;
use std::sync::Arc;

thrust_macros::init!();

#[tokio::main]
async fn main() {
    let container = Arc::new(Container::build());
    let app = build_router(container);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("listening on http://0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    fn app() -> axum::Router {
        build_router(Arc::new(Container::build()))
    }

    #[tokio::test]
    async fn get_todos_exists() {
        let res = app()
            .oneshot(Request::builder().uri("/todos").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn post_todos_exists() {
        let res = app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/todos")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn unknown_route_returns_404() {
        let res = app()
            .oneshot(Request::builder().uri("/nope").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }
}
