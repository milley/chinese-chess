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
use crate::middleware::rate_limit::RateLimitState;
use crate::websocket::manager::RoomManager;

/// 应用共享状态
#[derive(Clone)]
pub struct AppState {
    pub user_repo: UserRepository,
    pub game_repo: GameRepository,
    pub room_manager: RoomManager,
    pub jwt_secret: String,
    /// Per-user rate limiter for WS game-action messages (30 msgs/10s)
    pub ws_rate_limit: RateLimitState,
    /// Database connection pool for health checks and direct queries
    pub pool: sqlx::PgPool,
}

impl AppState {
    /// Persist a game-ending result to the database with Elo rating updates.
    ///
    /// This is the single entry point for all game-ending paths (checkmate, resign,
    /// draw, disconnect, timeout). It fetches the game from DB, then calls
    /// `finish_game_with_elo` with the in-room FEN and move history.
    ///
    /// Errors are logged but not propagated — the game result is broadcast to
    /// players regardless of DB persistence success.
    pub async fn persist_game_end(
        &self,
        game_id: uuid::Uuid,
        result_str: &str,
        reason_str: &str,
        fen: &str,
    ) {
        let game = match self.game_repo.find_by_id(game_id).await {
            Ok(Some(g)) => g,
            Ok(None) => {
                tracing::error!("persist_game_end: game {} not found in DB", game_id);
                return;
            }
            Err(e) => {
                tracing::error!("persist_game_end: failed to fetch game {}: {}", game_id, e);
                return;
            }
        };

        if let Err(e) = crate::services::elo_service::finish_game_with_elo(
            &self.game_repo,
            &self.user_repo,
            game_id,
            &game,
            result_str,
            reason_str,
            fen,
        ).await {
            tracing::error!("persist_game_end: failed to finish game {} with Elo: {}", game_id, e);
        }
    }
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
        .max_connections(config.database_pool_size)
        .connect(&config.database_url)
        .await?;

    tracing::info!("Connected to database");

    // 5. 运行迁移
    sqlx::migrate!("./migrations").run(&pool).await?;
    tracing::info!("Database migrations applied");

    // 6. 创建应用状态
    let game_repo = GameRepository::new(pool.clone());
    let user_repo = UserRepository::new(pool.clone());
    let room_manager = RoomManager::with_repos(game_repo.clone(), user_repo.clone());
    room_manager.start_timeout_checker();

    // WS rate limiter: 30 game-action messages per 10 seconds per user
    let ws_rate_limit = RateLimitState::new(30, 10);
    spawn_rate_limit_cleanup(ws_rate_limit.clone(), 60);

    let state = AppState {
        user_repo,
        game_repo,
        room_manager,
        jwt_secret: config.jwt_secret.clone(),
        ws_rate_limit,
        pool: pool.clone(),
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

    // 8. Rate limit states
    // Strict: 5 req/min per IP (login, register — prevent brute force)
    // Moderate: 30 req/min per IP (AI, move, valid_moves, list_users — prevent CPU abuse)
    // Generous: 60 req/min per IP (general API endpoints)
    let make_rate_limit = |max: u64, window: u64| -> RateLimitState {
        match &config.trusted_proxy_header {
            Some(header) => RateLimitState::with_trusted_header(max, window, header.clone()),
            None => RateLimitState::new(max, window),
        }
    };
    let strict_limit = make_rate_limit(5, 60);
    let moderate_limit = make_rate_limit(30, 60);
    let generous_limit = make_rate_limit(60, 60);

    // Spawn cleanup tasks for rate limit buckets (every 60 seconds)
    spawn_rate_limit_cleanup(strict_limit.clone(), 60);
    spawn_rate_limit_cleanup(moderate_limit.clone(), 60);
    spawn_rate_limit_cleanup(generous_limit.clone(), 60);

    // 9. 构建路由 — using nested routers for different rate limit tiers
    let auth_routes = Router::new()
        .route("/api/users", post(handlers::user_handler::register))
        .route("/api/users/login", post(handlers::user_handler::login))
        .layer(axum::middleware::from_fn_with_state(
            strict_limit,
            middleware::rate_limit::rate_limit_middleware,
        ));

    let action_routes = Router::new()
        .route("/api/ai/move", post(handlers::ai_handler::get_ai_move))
        .route("/api/moves/valid", post(handlers::move_handler::get_valid_moves))
        .route("/api/games/{id}/move", post(handlers::game_move_handler::make_move))
        .route("/api/users", get(handlers::user_handler::list_users))
        .layer(axum::middleware::from_fn_with_state(
            moderate_limit,
            middleware::rate_limit::rate_limit_middleware,
        ));

    let general_routes = Router::new()
        .route("/api/users/me", get(handlers::user_handler::get_current_user))
        .route("/api/users/me", put(handlers::user_handler::update_user))
        .route("/api/users/me", delete(handlers::user_handler::delete_user))
        .route("/api/users/{id}", get(handlers::user_handler::get_user))
        .route("/api/games", post(handlers::game_handler::create_game))
        .route("/api/games/{id}", get(handlers::game_handler::get_game))
        .route("/api/games/{id}", delete(handlers::game_handler::delete_game))
        .route("/api/games/{id}/moves", get(handlers::game_handler::get_game_moves))
        .route("/api/games/{id}/events", get(handlers::game_handler::get_game_events))
        .route("/api/games", get(handlers::game_handler::list_games))
        .route("/api/games/{id}/join", post(handlers::game_handler::join_game))
        .route("/api/games/{id}/rematch", post(handlers::game_handler::rematch))
        .layer(axum::middleware::from_fn_with_state(
            generous_limit,
            middleware::rate_limit::rate_limit_middleware,
        ));

    let app = Router::new()
        .route("/health", get(handlers::health_handler::health_check))
        .merge(auth_routes)
        .merge(action_routes)
        .merge(general_routes)
        // WebSocket (no rate limit)
        .route("/ws", get(handlers::ws_handler::ws_handler))
        // Global middleware
        .layer(axum::middleware::from_fn_with_state(
            state.jwt_secret.clone(),
            middleware::add_jwt_secret_to_extensions,
        ))
        // Content-Security-Policy header to mitigate XSS attacks
        .layer(tower_http::set_header::SetResponseHeaderLayer::if_not_present(
            axum::http::header::CONTENT_SECURITY_POLICY,
            axum::http::HeaderValue::from_static("default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self' ws: wss:; img-src 'self' data:; font-src 'self';"),
        ))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // 10. 启动服务
    use std::net::SocketAddr;
    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    tracing::info!("Server starting on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;

    Ok(())
}

/// Spawn a background task to periodically clean up expired rate limit entries.
fn spawn_rate_limit_cleanup(state: RateLimitState, interval_secs: u64) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
        loop {
            interval.tick().await;
            state.cleanup().await;
        }
    });
}
