use anyhow::Context;
use axum::{extract::{Path, State}, http::{header, StatusCode}, response::{IntoResponse, Response}, routing::get, Json, Router};
use frank::FrankStream;
use log::{info, LevelFilter};
use serde_json::{json, Value};
use settings::{TenSettings, ByPath};
use simplelog::{ColorChoice, CombinedLogger, TermLogger, TerminalMode, WriteLogger};
use std::{fs::File, sync::Arc};
use tokio::sync::RwLock;

mod frank;
mod scheduler;
mod settings;
mod test;

const SETTINGS_FILE: &str = "settings.json";
const LOG_FILE: &str = "tensleep.log";

struct AppState {
    dac: Arc<FrankStream>,
    settings: Arc<RwLock<TenSettings>>,
}

#[tokio::main]
async fn main() {
    CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Debug,
            simplelog::Config::default(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            LevelFilter::Debug,
            simplelog::Config::default(),
            File::create(LOG_FILE).context("Making log file").unwrap(),
        ),
    ])
    .context("Making combined logger")
    .unwrap();

    info!("Tensleep started. Connecting to frankenfirmware...");
    let dac = FrankStream::spawn().await.unwrap();

    info!("Reading settings file: {SETTINGS_FILE}");
    let init_settings = TenSettings::from_file(SETTINGS_FILE).unwrap();
    let settings = Arc::new(RwLock::new(init_settings));

    info!("Spawning scheduler thread...");
    scheduler::spawn(dac.clone(), settings.clone());

    let state = Arc::new(AppState { dac, settings });

    info!("Creating Axum router");
    let app = Router::new()
        .route("/health", get(get_health))
        .route("/state", get(get_state))
        .route("/settings", get(get_settings).post(post_settings))
        .route("/setting/*path", get(get_setting).post(post_setting))
        .route("/prime", get(prime).post(prime))
        .fallback(get_lost)
        .with_state(state);

    info!("Spawning Axum");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .context("Serving Axum")
        .unwrap();
}

async fn get_lost() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "404 Not Found")
}

async fn get_state(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let variables = state.dac.get_state().await;
    if let Err(e) = variables {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "Failed to get state",
                "details": e.to_string()
            })),
        ).into_response()
    }
    match variables.unwrap().serialize() {
        Ok(serialized) => Json(serialized).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "Failed to serialize state",
                "details": e.to_string()
            })),
        ).into_response(),
    }
}

async fn get_health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    if state.dac.ping().await.is_ok() {
        (
            StatusCode::OK,
            Json(json!({
              "status": "OK"
            })),
        ).into_response()
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
              "status": "UNAVAILABLE"
            })),
        ).into_response()
    }
}

async fn prime(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.dac.prime().await {
        Ok(r) => (
            StatusCode::OK,
            Json(json!({
              "response": r
            })),
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
              "error": e.to_string()
            })),
        ).into_response()
    };
}

async fn get_settings(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let settings = state.settings.read().await;
    match settings.serialize() {
        Ok(serialized) => Json(serialized).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "Failed to serialize settings",
                "details": e.to_string()
            })),
        ).into_response(),
    }
}

async fn get_setting(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
) -> impl IntoResponse {
    let settings = state.settings.read().await;
    info!("API: get setting {}", path);
    match settings.get_at_path(path.split('/').collect()) {
        Ok(Some(value)) => Json(value).into_response(),
        Ok(None) => Json(Value::Null).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": e.to_string()
            }))
        ).into_response(),
    }
}

async fn post_settings(
    State(state): State<Arc<AppState>>,
    Json(new_settings): Json<TenSettings>,
) -> impl IntoResponse {
    info!("API: set settings to {new_settings:#?}");
    let mut settings = state.settings.write().await;
    *settings = new_settings.clone();

    match new_settings.save(SETTINGS_FILE) {
        Ok(_) => Json(json!({
            "message": "Settings updated successfully",
            "settings": new_settings
        })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "Failed to save settings",
                "details": e.to_string()
            })),
        ).into_response(),
    }
}

async fn post_setting(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
    value: String,
) -> impl IntoResponse {
    let mut settings = state.settings.write().await;
    info!("API: setting setting {} to {}", path, value);
    let res = settings.set_at_path(path.split('/').collect(), value.to_string());
    if let Err(e) = res {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": e.to_string(),
            })),
        ).into_response()
    }

    match settings.save(SETTINGS_FILE) {
        Ok(_) => Json(json!({
            "message": "Setting updated successfully",
            "settings": *settings
        })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "Failed to save settings",
                "details": e.to_string()
            })),
        ).into_response(),
    }
}
