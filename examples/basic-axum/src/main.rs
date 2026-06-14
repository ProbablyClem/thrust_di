mod config;
mod controllers;
mod services;

use services::*;

thrust_macros::init!();

#[tokio::main]
async fn main() {
    run().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use std::sync::Arc;
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
