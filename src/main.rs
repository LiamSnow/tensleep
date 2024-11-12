#[macro_use]
extern crate rustc_serialize;

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use dac::{
    manager::{self, DacStream},
    types::*,
};
use log::{debug, info, LevelFilter};
use serde_json::json;
use settings::Settings;
use simplelog::{ColorChoice, CombinedLogger, TermLogger, TerminalMode, WriteLogger};
use std::{
    fs::File,
    net::TcpListener,
    os::unix::net::{UnixListener, UnixStream},
    sync::{Arc, Mutex, RwLock},
    thread,
};
use tokio::{task::{self, JoinHandle}, time};

mod dac;
mod scheduler;
mod settings;

struct AppState {
    settings: Mutex<Settings>,
    scheduler_task: Mutex<JoinHandle<()>>,
    dac_stream: DacStream,
}

const SETTINGS_PATH: &str = "settings.json";

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
            File::create("tensleep.log").unwrap(),
        ),
    ])
    .unwrap();

    info!("Tensleep started. Connecting to stream...");

    let dac_stream = Arc::new(RwLock::<Option<UnixStream>>::new(None));
    manager::init(dac_stream.clone());

    while !manager::hello(dac_stream.clone()).contains("ok") {
        time::sleep(time::Duration::from_secs(10)).await;
        debug!("still connecting to stream...");
    }

    info!("Connected to stream!");

    info!("Reading settings file: {SETTINGS_PATH}");
    let settings = Settings::from_file(SETTINGS_PATH).unwrap();

    info!("Spawning scheduler task...");
    let scheduler_task = tokio::spawn(scheduler::run(settings.clone(), dac_stream.clone()));

    let state = Arc::new(AppState {
        settings: Mutex::new(settings),
        scheduler_task: Mutex::new(scheduler_task),
        dac_stream,
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

async fn get_health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let res = manager::hello(state.dac_stream.clone());
    info!("Axum: health check got {res}");
    if res.contains("ok") {
        (
            StatusCode::OK,
            Json(json!({
              "status": "OK"
            }))
        ).into_response()
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
              "status": "UNAVAILABLE"
            }))
        ).into_response()
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
    *task = tokio::spawn(scheduler::run(new_settings.clone(), state.dac_stream.clone()));

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
