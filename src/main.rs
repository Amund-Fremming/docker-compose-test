use std::{env, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path, Request, State},
    http::StatusCode,
    middleware::{Next, from_fn},
    response::{IntoResponse, Response},
    routing::get,
};
use dotenv::dotenv;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres, prelude::FromRow};
use thiserror::Error;
use tracing::{error, info, level_filters::LevelFilter};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() {
    dotenv().ok();

    let connection_string =
        env::var("DATABASE_URL").expect("DATABASE_URL variable is missing in .env file");

    let app_state = AppState::from_connection_string(&connection_string)
        .await
        .unwrap_or_else(|e| panic!("{}", e));

    let app = Router::new()
        .route("/{user_id}", get(get_user))
        .route("/health", get(health))
        .with_state(app_state.clone())
        .layer(from_fn(request_logger_mw));

    let subscriber = FmtSubscriber::builder()
        .with_max_level(LevelFilter::DEBUG)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set global tracing");

    let port = env::var("PORT").expect("PORT variable is missing in .env file");
    let host = env::var("HOST").expect("HOST variable is missing in .env file");
    let addr = format!("{}:{}", host, port);

    info!("Server running on address: {}", &addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app)
        .await
        .expect("Failed to start serving web-server");
}

pub struct AppState {
    pool: Pool<Postgres>,
}

impl AppState {
    async fn from_connection_string(connection_string: &str) -> Result<Arc<Self>, ServerError> {
        let pool = Pool::<Postgres>::connect(&connection_string)
            .await
            .map_err(|_| ServerError::DBInitError)?;

        Ok(Arc::new(Self { pool }))
    }

    pub fn get_pool(&self) -> &Pool<Postgres> {
        &self.pool
    }
}

#[derive(Serialize, Deserialize, Debug, FromRow)]
struct User {
    id: i32,
    email: String,
    name: Option<String>,
}

#[derive(Debug, Error)]
enum ServerError {
    #[error("SQLX failed: {0}")]
    SqlError(#[from] sqlx::Error),

    #[error("Failed to initialize database")]
    DBInitError,
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        match self {
            ServerError::SqlError(e) => {
                error!("SQLX failed: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, String::new())
            }
            ServerError::DBInitError => {
                // This will never be hit
                (StatusCode::INTERNAL_SERVER_ERROR, String::new())
            }
        }
        .into_response()
    }
}

async fn request_logger_mw(req: Request, next: Next) -> Result<Response, ServerError> {
    let method = req.method().clone();
    let uri = req.uri().path().to_string();
    info!("Incomming request: status={} uri={}", method, uri);

    let res = next.run(req).await;

    info!("Outgoing request: method={} uri={}", method, uri);

    Ok(res)
}

async fn health() -> Result<Response, ServerError> {
    Ok((StatusCode::OK, String::from("OK")).into_response())
}

async fn get_user(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<i32>,
) -> Result<Response, ServerError> {
    let user = get_user_from_db(state.get_pool(), user_id).await?;

    match user {
        Some(user) => Ok((StatusCode::OK, Json(user)).into_response()),
        None => Ok((StatusCode::NOT_FOUND).into_response()),
    }
}

async fn get_user_from_db(
    pool: &Pool<Postgres>,
    user_id: i32,
) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as!(
        User,
        r#"
        SELECT id, name, email 
        FROM "user"
        WHERE id = $1
        "#,
        user_id
    )
    .fetch_optional(pool)
    .await
}
