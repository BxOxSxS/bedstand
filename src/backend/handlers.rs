use crate::backend::app_state::config::{Config, config_path};
use crate::backend::app_state::global_app_state;
use crate::backend::server::ServerState;
use crate::error::*;
use axum::{
    Form,
    body::{Body, Bytes},
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, Response, StatusCode, header},
    response::{Html, IntoResponse, Redirect},
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use linemux::MuxedLines;
use nix::{
    sys::reboot::{RebootMode, reboot},
    unistd::execv,
};
use std::{env::current_exe, ffi::CString, os::unix::ffi::OsStrExt};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{info, warn};

pub async fn update_handler(body: Bytes) -> HttpResult<()> {
    let json = match std::str::from_utf8(&body) {
        Ok(json) => json,
        Err(_) => {
            let e = Error::new("Invalid UTF-8 in request body");
            return Err(e).map_err(|e| e.into_http_error(StatusCode::BAD_REQUEST));
        }
    };

    match global_app_state().data.update(json) {
        Ok(()) => Ok(()),
        Err(err) => {
            let e = Error::new(format!("Data update failed: {err}"));
            Err(e).map_err(|e| e.into_http_error(StatusCode::BAD_REQUEST))
        }
    }
}

pub async fn restart() -> HttpResult<()> {
    let exe = current_exe()?;
    let exe = CString::new(exe.as_os_str().as_bytes())?;
    let args: Vec<CString> = std::env::args_os()
        .map(|x| CString::new(x.as_bytes()).unwrap())
        .collect();

    // Use spawn and wait to avoid restart before response
    tokio::spawn(async move {
        info!("Restarting application...\n");
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        match execv(&exe, &args) {
            Ok(_) => (),
            Err(err) => {
                let _ = Error::new(format!("Failed to restart: {err}"));
            }
        }
    });

    Ok(())
}

pub async fn poweroff() -> HttpResult<()> {
    tokio::spawn(async move {
        info!("Powering off system...\n");
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        match reboot(RebootMode::RB_POWER_OFF) {
            Ok(_) => (),
            Err(err) => {
                let _ = Error::new(format!("Failed to power off: {err}"));
            }
        }
    });
    Ok(())
}

pub async fn reboot_handler() -> HttpResult<()> {
    tokio::spawn(async move {
        info!("Rebooting system...\n");
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        match reboot(RebootMode::RB_AUTOBOOT) {
            Ok(_) => (),
            Err(err) => {
                let _ = Error::new(format!("Failed to reboot: {err}"));
            }
        }
    });
    Ok(())
}

pub async fn get_brightness() -> HttpResult<String> {
    let res = global_app_state()
        .hardware
        .read()
        .await
        .panel
        .refresh()
        .await;
    match res {
        Ok(Some(value)) => Ok(value.to_string()),
        Ok(None) => Err(Error::new("Brightness not available")
            .into_http_error(StatusCode::INTERNAL_SERVER_ERROR)),
        Err(e) => Err(e).map_err(|e| e.into_http_error(StatusCode::INTERNAL_SERVER_ERROR)),
    }
}

pub async fn set_brightness(body: Bytes) -> HttpResult<()> {
    let value: u32 = std::str::from_utf8(&body)
        .map_err(|e| {
            Error::new(format!("Failed to parse brightness: {e}"))
                .into_http_error(StatusCode::BAD_REQUEST)
        })?
        .parse()
        .map_err(|e| {
            Error::new(format!("Failed to parse brightness: {e}"))
                .into_http_error(StatusCode::BAD_REQUEST)
        })?;

    let res = global_app_state()
        .hardware
        .read()
        .await
        .panel
        .set_force(Some(value))
        .await
        .add();
    match res {
        Ok(_) => Ok(()),
        Err(e) => Err(e).map_err(|e| e.into_http_error(StatusCode::BAD_REQUEST)),
    }
}

pub async fn get_runtime_config() -> HttpResult<String> {
    let config = global_app_state().config.read().await;
    let config_json = toml::to_string_pretty(&*config)?;
    Ok(config_json)
}

pub async fn set_runtime_config(body: Bytes) -> HttpResult<()> {
    let new_config: Config = toml::from_str(&String::from_utf8_lossy(&body)).map_err(|e| {
        Error::new(format!("Failed to parse runtime config: {e}"))
            .into_http_error(StatusCode::BAD_REQUEST)
    })?;

    info!("Setting runtime config:\n{new_config:#?}");

    let mut config = global_app_state().config.write().await;
    *config = new_config;

    Ok(())
}

pub async fn get_config() -> HttpResult<String> {
    let config_str = std::fs::read_to_string(config_path()).map_err(|e| {
        Error::new(format!("Failed to read config file: {e}"))
            .into_http_error(StatusCode::INTERNAL_SERVER_ERROR)
    })?;

    Ok(config_str)
}

pub async fn set_config(body: Bytes) -> HttpResult<()> {
    let new_config: Config = toml::from_slice(&body).map_err(|e| {
        Error::new(format!("Failed to parse config: {e}")).into_http_error(StatusCode::BAD_REQUEST)
    })?;

    new_config.save().map_err(|e| {
        Error::new(format!("Failed to save config: {e}"))
            .into_http_error(StatusCode::INTERNAL_SERVER_ERROR)
    })?;

    info!("Setting config:\n{new_config:#?}");

    let mut config = global_app_state().config.write().await;
    *config = new_config;

    Ok(())
}

pub async fn get_data() -> HttpResult<String> {
    let data = &global_app_state().data;
    let data_str = format!("{:#?}", data);
    Ok(data_str)
}

pub async fn logs() -> Result<Response<Body>> {
    let (tx, rx) = tokio::sync::mpsc::channel::<std::result::Result<Bytes, std::io::Error>>(64);

    tokio::spawn(async move {
        let mut lines = MuxedLines::new()?;
        lines.add_file_from_start(crate::LOG_FILE).await?;

        while let Ok(Some(line)) = lines.next_line().await {
            let bytes = Bytes::from(format!("{}\n", line.line()));

            if tx.send(Ok(bytes)).await.is_err() {
                // client disconnected, stop sending logs
                break;
            }
        }
        Ok::<(), Error>(())
    });

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Body::from_stream(ReceiverStream::new(rx)))?)
}

pub async fn settings() -> Html<&'static str> {
    Html::from(include_str!("../../assets/settings.html")) //todo add new stuff
}

pub async fn login(State(state): State<ServerState>, request: Request) -> Response<Body> {
    if crate::backend::server::is_authorized(&request, &state.auth_tokens) {
        return Redirect::temporary("/settings").into_response();
    }

    Html::from(include_str!("../../assets/login.html")).into_response()
}

#[derive(serde::Deserialize)]
pub struct LoginForm {
    token: String,
}

pub async fn login_post(
    State(state): State<ServerState>,
    Form(form): Form<LoginForm>,
) -> Response<Body> {
    let valid = state.auth_tokens.iter().any(|token| token == &form.token);

    if !valid {
        warn!("Invalid login attempt with token: {}", form.token);
        return (StatusCode::UNAUTHORIZED, "Invalid token").into_response();
    }

    let cookie_name = if state.https { "__Host-auth" } else { "auth" };

    let cookie = Cookie::build((cookie_name, form.token))
        .path("/")
        .secure(state.https)
        .http_only(true)
        .same_site(SameSite::Strict)
        .build();

    let mut set_cookie = cookie.to_string();
    set_cookie.push_str("; Max-Age=31536000");

    let mut headers = HeaderMap::new();

    headers.insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&set_cookie).expect("valid Set-Cookie header"),
    );

    (headers, Redirect::to("/settings")).into_response()
}
