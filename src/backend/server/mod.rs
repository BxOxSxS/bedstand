mod handlers;

use crate::backend::app_state::global_app_state;
use crate::error::*;
use axum::{
    Router,
    extract::DefaultBodyLimit,
    extract::{Request, State},
    http::{Method, StatusCode, header},
    middleware,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use axum_extra::extract::CookieJar;
use axum_server::tls_rustls::RustlsConfig;
use handlers::*;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::net::SocketAddr;
use std::time::Duration;
use tracing::{debug, info, warn};

#[derive(Clone)]
pub struct ServerState {
    pub https: bool,
    pub auth_tokens: Vec<String>,
}

pub async fn run() -> Result<()> {
    let (fullchain_path, privkey_path, addr, pem_notify, auth_tokens) = {
        let config = global_app_state().config.read().await;

        (
            config.http.fullchain_path.clone(),
            config.http.privkey_path.clone(),
            config.http.server.clone(),
            config.http.notify,
            config.http.auth_tokens.clone(),
        )
    };

    let https = !fullchain_path.is_empty() && !privkey_path.is_empty();

    let state = ServerState { https, auth_tokens };

    let public_routes = Router::new()
        .route("/", get(login))
        .route("/", post(login_post));

    let protected_routes = Router::new()
        .route("/update", post(update_handler))
        .route("/settings", get(settings))
        .route("/settings/restart", post(restart))
        .route("/settings/poweroff", post(poweroff))
        .route("/settings/reboot", post(reboot_handler))
        .route("/settings/runtime_config", get(get_runtime_config))
        .route("/settings/runtime_config", post(set_runtime_config))
        .route("/settings/config", get(get_config))
        .route("/settings/config", post(set_config))
        .route("/settings/data", get(get_data))
        .route("/settings/logs", get(logs))
        .route("/settings/ambient", get(ambient))
        .route("/settings/proximity", get(proximity))
        .route("/settings/panel", get(panel))
        .route("/settings/panel", post(set_panel))
        .route("/settings/ui_alpha", get(ui_alpha))
        .route("/settings/battery", get(battery))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let app = public_routes
        .merge(protected_routes)
        .layer(DefaultBodyLimit::max(256 * 1024)) //256 KB
        .with_state(state.clone());

    match (fullchain_path.is_empty(), privkey_path.is_empty()) {
        (true, true) => {
            //tls disabled, plain HTTP
            let listener = tokio::net::TcpListener::bind(addr.clone()).await?;

            info!("HTTP server listening on http://{addr}");
            axum::serve(listener, app).await?;
        }

        (false, false) => {
            //tls enabled, HTTPS
            let tls_config = RustlsConfig::from_pem_file(&fullchain_path, &privkey_path).await?;
            let addr: SocketAddr = addr.parse()?;

            if pem_notify {
                spawn_tls_watcher(
                    tls_config.clone(),
                    fullchain_path.clone(),
                    privkey_path.clone(),
                )?;
            }

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
        let config = global_app_state().config.read().await;
        (config.webhook.retry_cooldown, config.webhook.url.clone())
    };

    let last_try_time_field = &global_app_state().data.last_try_time;
    let now = chrono::Local::now();

    if last_try_time_field.get().is_some_and(|last_try_time| {
        (now - last_try_time).num_seconds() < retry_cooldown.as_secs() as i64
    }) {
        return Err(Error::new("Webhook retry cooldown not yet passed"));
    }
    last_try_time_field.set(Some(now)).await?;

    let result = reqwest::Client::new().post(webhook_url).send().await;

    match result {
        Ok(_) => Ok(()),
        Err(err) => Err(Error::new(format!("Webhook send error: {err:?}"))),
    }
}

pub fn trigger_ask_update() {
    tokio::spawn(ask_update());
}

fn spawn_tls_watcher(
    tls_config: RustlsConfig,
    fullchain_path: String,
    privkey_path: String,
) -> Result<()> {
    let fullchain_path = std::path::absolute(fullchain_path)?;
    let privkey_path = std::path::absolute(privkey_path)?;

    let watch_dir = fullchain_path
        .parent()
        .ok_or_else(|| Error::new("Invalid certificate path"))?
        .to_path_buf();

    let (tx, mut rx) = tokio::sync::mpsc::channel::<notify::Result<Event>>(16);

    let mut watcher = RecommendedWatcher::new(
        move |result| {
            let _ = tx.blocking_send(result);
        },
        Config::default(),
    )?;

    watcher.watch(&watch_dir, RecursiveMode::NonRecursive)?;

    tokio::spawn(async move {
        //keep the watcher alive in this async task
        let _watcher = watcher;

        let debounce_duration = Duration::from_secs(1);
        let mut reload_deadline = None;

        let timer = tokio::time::sleep(Duration::from_secs(1));
        tokio::pin!(timer);

        loop {
            tokio::select! {
                result = rx.recv() => {
                    let Some(result) = result else {
                        break;
                    };

                    let event = match result {
                        Ok(event) => event,
                        Err(err) => {
                            let _ = Error::new(format!("Failed to watch TLS certificate files: {err}"));
                            continue;
                        }
                    };

                    let relevant = event.paths.iter().any(|path| {
                        path == &fullchain_path || path == &privkey_path
                    });

                    if !relevant {
                        continue;
                    }

                    if !matches!(
                        event.kind,
                        EventKind::Create(_)
                            | EventKind::Modify(_)
                            | EventKind::Remove(_)
                    ) {
                        continue;
                    }

                    debug!("TLS certificate files changed: {:?}", event.paths);

                    let deadline = tokio::time::Instant::now() + debounce_duration;

                    reload_deadline = Some(deadline);
                    timer.as_mut().reset(deadline);
                }

                _ = &mut timer, if reload_deadline.is_some() => {
                    reload_deadline = None;

                    match tls_config
                        .reload_from_pem_file(&fullchain_path, &privkey_path)
                        .await
                    {
                        Ok(()) => info!("TLS configuration reloaded"),
                        Err(err) => {
                            let _ = Error::new(format!("Failed to reload TLS configuration: {err}"));
                        }
                    }
                }
            }
        }
    });

    Ok(())
}

async fn auth_middleware(
    State(state): State<ServerState>,
    request: Request,
    next: middleware::Next,
) -> Response {
    if is_authorized(&request, &state.auth_tokens) {
        return next.run(request).await;
    }

    warn!(
        "Unauthorized request: {} {}",
        request.method(),
        request.uri().path()
    );

    if request.method() == Method::GET && request.uri().path() == "/settings" {
        return Redirect::to("/").into_response();
    }

    StatusCode::UNAUTHORIZED.into_response()
}

pub fn is_authorized(request: &Request, auth_tokens: &[String]) -> bool {
    if auth_tokens.is_empty() {
        return true;
    }

    if request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|token| auth_tokens.iter().any(|valid| valid == token))
    {
        return true;
    }

    let cookies = CookieJar::from_headers(request.headers());

    cookies.iter().any(|cookie| {
        matches!(cookie.name(), "auth" | "__Host-auth")
            && auth_tokens.iter().any(|valid| valid == cookie.value())
    })
}
