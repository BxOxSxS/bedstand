use crate::backend::config::global_config;
use crate::backend::data::{Data, global_data};
use crate::error::*;
use axum::{Router, body::Bytes, extract::DefaultBodyLimit, http::StatusCode, routing::post};
use tracing::{debug, info};

pub async fn run() -> Result<()> {
    let app = Router::new()
        .route("/update", post(update_handler))
        .layer(DefaultBodyLimit::max(256 * 1024)); //256KiB

    let addr = {
        let config = global_config().read().await;
        config.http_server.clone()
    };

    let listener = tokio::net::TcpListener::bind(addr.clone()).await?;
    info!("HTTP server listening on http://{addr}");

    match axum::serve(listener, app).await {
        Ok(_) => Ok(()),
        Err(err) => Err(Error::new(format!("HTTP server error: {err}"))),
    }
}

async fn update_handler(body: Bytes) -> StatusCode {
    let json = match std::str::from_utf8(&body) {
        Ok(json) => json,
        Err(_) => {
            let _ = Error::new("Invalid UTF-8 in request body");
            return StatusCode::BAD_REQUEST;
        }
    };

    match Data::update(json) {
        Ok(()) => StatusCode::OK,
        Err(err) => {
            let _ = Error::new(format!("Data update failed: {err}"));
            StatusCode::BAD_REQUEST
        }
    }
}

pub async fn ask_update() -> Result<()> {
    debug!("Triggering webhook update");

    let (retry_cooldown, webhook_url) = {
        let config = global_config().read().await;
        (config.retry_cooldown, config.webhook_url.clone())
    };

    let last_try_time_field = &global_data().last_try_time;
    let now = chrono::Local::now();

    if last_try_time_field.get().is_some_and(|last_try_time| {
        (now - last_try_time).num_seconds() < retry_cooldown.as_secs() as i64
    }) {
        return Err(Error::new("Webhook retry cooldown not yet passed"));
    }
    last_try_time_field.update(Some(now));

    let result = reqwest::Client::new().post(webhook_url).send().await;

    match result {
        Ok(_) => Ok(()),
        Err(err) => Err(Error::new(format!("Webhook send error: {err:?}"))),
    }
}

pub fn trigger_ask_update() {
    tokio::spawn(ask_update());
}
