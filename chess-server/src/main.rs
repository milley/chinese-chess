mod config;
mod db;
mod error;
mod handlers;
mod middleware;
mod services;
mod utils;
mod websocket;

use axum::Router;
use axum::routing::{delete, get, post, put};
use sqlx::postgres::PgPoolOptions;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;
use axum::http::HeaderValue;

use crate::config::AppConfig;
use crate::db::repositories::game_repo::GameRepository;
use crate::db::repositories::user_repo::UserRepository;
use crate::websocket::manager::RoomManager;

/// 应用共享状态
#[derive(Clone)]
pub struct AppState {
    pub user_repo: UserRepository,
    pub game_repo: GameRepository,
    pub room_manager: RoomManager,
    pub jwt_secret: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 加载 .env
    dotenvy::dotenv().ok();

    // 2. 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or("chess_server=debug,tower_http=debug".into()))
        .init();

    // 3. 加载配置
    let config = AppConfig::from_env()?;

    // 4. 连接数据库
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;

    tracing::info!("Connected to database");

    // 5. 运行迁移
    sqlx::migrate!("./migrations").run(&pool).await?;
    tracing::info!("Database migrations applied");

    // 6. 创建应用状态
    let game_repo = GameRepository::new(pool.clone());
    let room_manager = RoomManager::with_game_repo(game_repo.clone());
    room_manager.start_timeout_checker();

    let state = AppState {
        user_repo: UserRepository::new(pool.clone()),
        game_repo,
        room_manager,
        jwt_secret: config.jwt_secret.clone(),
    };

    // 7. 构建 CORS 层
    let cors = if config.cors_origins.contains(&"*".to_string()) {
        CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any)
    } else if config.cors_origins.is_empty() {
        tracing::warn!("No CORS_ORIGINS configured; allowing localhost:5173 (Vite dev) as default");
        CorsLayer::new()
            .allow_origin("http://localhost:5173".parse::<HeaderValue>().unwrap())
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        let origins: Vec<HeaderValue> = config.cors_origins.iter()
            .filter_map(|o| o.parse::<HeaderValue>().ok())
            .collect();
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods(Any)
            .allow_headers(Any)
    };

    // 8. 构建路由
    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        // 用户路由
        .route("/api/users", post(handlers::user_handler::register))
        .route("/api/users/login", post(handlers::user_handler::login))
        .route("/api/users/me", get(handlers::user_handler::get_current_user))
        .route("/api/users/me", put(handlers::user_handler::update_user))
        .route("/api/users/me", delete(handlers::user_handler::delete_user))
        .route("/api/users/{id}", get(handlers::user_handler::get_user))
        .route("/api/users", get(handlers::user_handler::list_users))
        // 对局路由
        .route("/api/games", post(handlers::game_handler::create_game))
        .route("/api/games/{id}", get(handlers::game_handler::get_game))
        .route("/api/games/{id}", delete(handlers::game_handler::delete_game))
        .route("/api/games", get(handlers::game_handler::list_games))
        .route("/api/games/{id}/join", post(handlers::game_handler::join_game))
        // AI 和走法路由
        .route("/api/ai/move", post(handlers::ai_handler::get_ai_move))
        .route("/api/moves/valid", post(handlers::move_handler::get_valid_moves))
        .route("/api/games/{id}/move", post(handlers::game_move_handler::make_move))
        // WebSocket
        .route("/ws", get(handlers::ws_handler::ws_handler))
        // 中间件
        .layer(axum::middleware::from_fn_with_state(
            state.jwt_secret.clone(),
            middleware::add_jwt_secret_to_extensions,
        ))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // 9. 启动服务
    use std::net::SocketAddr;
    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    tracing::info!("Server starting on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
