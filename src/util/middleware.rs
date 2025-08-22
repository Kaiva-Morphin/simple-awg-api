use axum::{
    body::Body,
    http::Request,
    middleware::Next,
    response::Response,
};
use tracing::{info, Instrument, Span};


pub async fn logging_middleware(req: Request<Body>, next: Next) -> Response {
    let span = Span::current();
    info!("Received request on: {}.", req.uri().to_string());
    let response = next.run(req).instrument(span).await;
    info!("Response status: {:?}", response.status());
    response
}

#[macro_export]
macro_rules! make_unique_span {
    ($name:ident) => {
        let $name = $crate::tracing::info_span!("", %format!("\x1b[90m{}\x1b[0m", uuid::Uuid::new_v4().simple()));
    };

    ($prefix:expr, $name:ident) => {
        let id = format!("\x1b[90m{}\x1b[0m", uuid::Uuid::new_v4().simple());
        let $name = tracing::info_span!($prefix, "id" = %id);
    };
}

#[macro_export]
macro_rules! layer_with_unique_span {
    ($prefix:expr) => {
        async |req: axum::extract::Request<axum::body::Body>, next: axum::middleware::Next| -> axum::response::Response {
            $crate::make_unique_span!($prefix, span);
            let response = tracing::Instrument::instrument(next.run(req), span).await;
            response
        }
    };
    () => {
        async |req: Request<Body>, next: Next| -> Response {
            let id : uuid::Uuid = uuid::Uuid::new_v4();
            $crate::make_unique_span!(span);
            let response = next.run(req).instrument(span).await;
            response
        }
    };
}
