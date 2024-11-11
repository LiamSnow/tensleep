extern crate rustc_serialize;

mod frank;
mod scheduler;
mod settings;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use frank::{
    manager::{self, FrankStream},
    types::*,
};
use log::{info, LevelFilter};
use serde_json::json;
use settings::Settings;
use simplelog::{ColorChoice, CombinedLogger, TermLogger, TerminalMode, WriteLogger};
use std::{fs::File, sync::{Arc, Mutex, OnceLock}};
use tokio::task::{self, JoinHandle};

struct AppState {
    settings: Mutex<Settings>,
    scheduler_task: Mutex<JoinHandle<()>>,
}

static STREAM: OnceLock<FrankStream> = OnceLock::new();

const SETTINGS_PATH: &str = "settings.json";

#[tokio::main]
async fn main() {
    CombinedLogger::init(
        vec![
            TermLogger::new(LevelFilter::Debug, simplelog::Config::default(), TerminalMode::Mixed, ColorChoice::Auto),
            WriteLogger::new(LevelFilter::Debug, simplelog::Config::default(), File::create("tensleep.log").unwrap()),
        ]
    ).unwrap();

    info!("Tensleep started. Connecting to stream...");

    STREAM.get_or_init(|| manager::init());

    if STREAM.get().unwrap().read().unwrap().is_none() {
      info!("Failed to connect to stream!");
      panic!();
    }

    info!("Connected to stream!");

    info!("Reading settings file: {SETTINGS_PATH}");
    let settings = Settings::from_file(SETTINGS_PATH).unwrap();

    info!("Spawning scheduler task...");
    let state = Arc::new(AppState {
        settings: Mutex::new(settings.clone()),
        scheduler_task: Mutex::new(task::spawn(scheduler::run(
            settings,
            &STREAM.get().unwrap(),
        ))),
    });

    info!("Creating axum router");
    let app = Router::new()
        .route("/health", get(get_health))
        .route("/settings", get(get_settings).post(post_settings))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}

async fn get_health() -> impl IntoResponse {
    let res = manager::hello(STREAM.get().unwrap());
    info!("Axum: health check got {res}");
    if res == "ok" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}

async fn get_settings(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    info!("Axum: get settings"); //FIXME
    let settings = state.settings.lock().unwrap();
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
    info!("Axum: set settings to {new_settings:#?}"); //FIXME
    let mut settings = state.settings.lock().unwrap();
    *settings = new_settings.clone();

    let mut task = state.scheduler_task.lock().unwrap();
    task.abort();
    *task = task::spawn(scheduler::run(new_settings.clone(), &STREAM.get().unwrap()));

    match new_settings.save(SETTINGS_PATH) {
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
