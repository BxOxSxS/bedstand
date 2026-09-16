use crate::backend::config::global_config;
use crate::backend::data::global_data;
use crate::backend::handlers::*;
use crate::error::*;
use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};
use std::net::SocketAddr;
use tracing::{debug, info};

pub async fn run() -> Result<()> {
    let app = Router::new()
        .route("/update", post(update_handler))
        .route("/settings", get(settings))
        .route("/settings/restart", post(restart))
        .route("/settings/poweroff", post(poweroff))
        .route("/settings/reboot", post(reboot_handler))
        .route("/settings/brightness", get(get_brightness))
        .route("/settings/brightness", post(set_brightness))
        .route("/settings/runtime_config", get(get_runtime_config))
        .route("/settings/runtime_config", post(set_runtime_config))
        .route("/settings/config", get(get_config))
        .route("/settings/config", post(set_config))
        .route("/settings/data", get(get_data))
        .route("/settings/logs", get(logs))
        .layer(DefaultBodyLimit::max(256 * 1024)); //256KiB

    let (fullchain_path, privkey_path, addr) = {
        let config = global_config().read().await;
        (
            config.pem_fullchain_path.clone(),
            config.pem_privkey_path.clone(),
            config.http_server.clone(),
        )
    };

    match (fullchain_path.is_empty(), privkey_path.is_empty()) {
        (true, true) => {
            //tls disabled, plain HTTP
            let listener = tokio::net::TcpListener::bind(addr.clone()).await?;

            info!("HTTP server listening on http://{addr}");
            axum::serve(listener, app).await?;
        }

        (false, false) => {
            //tls enabled, HTTPS
            let tls_config = axum_server::tls_rustls::RustlsConfig::from_pem_file(
                &fullchain_path,
                &privkey_path,
            )
            .await?;
            let addr: SocketAddr = addr.parse()?;

            info!("HTTPS server listening on https://{addr}");
            axum_server::tls_rustls::bind_rustls(addr, tls_config)
                .serve(app.into_make_service())
                .await?;
        }
        (true, false) | (false, true) => {
            return Err(Error::new(
                "TLS configuration is invalid: pem_fullchain_path and pem_privkey_path must either both be set or both be empty",
            ));
        }
    }

    Ok(())
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
