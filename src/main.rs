use anyhow::Context;
use axum::{extract::State, http::{header, StatusCode}, response::{IntoResponse, Response}, routing::get, Json, Router};
use dac::DacStream;
use log::{info, LevelFilter};
use serde_json::json;
use settings::Settings;
use simplelog::{ColorChoice, CombinedLogger, TermLogger, TerminalMode, WriteLogger};
use std::{fs::File, sync::Arc};
use tokio::sync::RwLock;

mod dac;
mod scheduler;
mod settings;

const SETTINGS_FILE: &str = "settings.json";
const LOG_FILE: &str = "tensleep.log";

struct AppState {
    dac: Arc<DacStream>,
    settings: Arc<RwLock<Settings>>,
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

    info!("Tensleep started. Spawn DAC thread...");
    let dac = DacStream::spawn().await.unwrap();

    info!("Reading settings file: {SETTINGS_FILE}");
    let init_settings = Settings::from_file(SETTINGS_FILE).unwrap();
    let settings = Arc::new(RwLock::new(init_settings));

    info!("Spawning scheduler thread...");
    scheduler::spawn(dac.clone(), settings.clone());

    let state = Arc::new(AppState { dac, settings });

    info!("Creating Axum router");
    let app = Router::new()
        .route("/health", get(get_health))
        .route("/state", get(get_state))
        .route("/settings", get(get_settings).post(post_settings))
        .route("/prime", get(prime).post(prime))
        .with_state(state);

    info!("Spawning Axum");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .context("Serving Axum")
        .unwrap();
}

async fn get_state(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.dac.get_variables().await {
        Ok(r) => {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/plain")
                .body(r.into())
                .unwrap_or_else(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Failed to create response: {}", e),
                    )
                        .into_response()
                })
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "Failed to get state/variables",
                "details": e.to_string()
            })),
        )
            .into_response(),
    }
}

async fn get_health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    if state.dac.ping().await {
        (
            StatusCode::OK,
            Json(json!({
              "status": "OK"
            })),
        )
            .into_response()
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
              "status": "UNAVAILABLE"
            })),
        )
            .into_response()
    }
}

async fn prime(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.dac.prime().await {
        Ok(r) => (
            StatusCode::OK,
            Json(json!({
              "response": r
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
              "error": e.to_string()
            })),
        )
            .into_response()
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
        )
            .into_response(),
    }
}

async fn post_settings(
    State(state): State<Arc<AppState>>,
    Json(new_settings): Json<Settings>,
) -> impl IntoResponse {
    info!("Axum: set settings to {new_settings:#?}");
    let mut settings = state.settings.write().await;
    *settings = new_settings.clone();

    match new_settings.save(SETTINGS_FILE) {
        Ok(_) => Json(json!({
            "message": "Settings updated successfully",
            "settings": new_settings
        }))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "Failed to save settings",
                "details": e.to_string()
            })),
        )
            .into_response(),
    }
}
