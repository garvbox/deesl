use deadpool_diesel::postgres::{Manager, Pool};
use tokio::net::TcpListener;
use tracing::info;

#[cfg(feature = "dev")]
use tower_livereload::LiveReloadLayer;

use deesl::{AppConfig, AppError, AppState, app::build_router, db::run_migrations, oauth_handlers};

#[tokio::main]
async fn main() -> Result<(), AppError> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let config = AppConfig::from_env().expect("Failed to load configuration");

    let manager = Manager::new(&config.database_url, deadpool_diesel::Runtime::Tokio1);
    let pool = Pool::builder(manager)
        .max_size(10)
        .build()
        .expect("Failed to create pool");

    run_migrations(&pool).await?;

    let auth = deesl::auth::AuthConfig::new(&config.jwt_secret, config.jwt_expiration_hours);

    #[cfg(feature = "dev")]
    let auth = {
        let dev_user = setup_dev_auth_user(&pool, config.dev_auth_email).await?;
        let mut auth = auth.clone();
        auth.dev_user = Some(dev_user);
        auth
    };

    let app_state = AppState {
        pool,
        oauth: oauth_handlers::OAuthConfig::new(
            &config.google_client_id,
            &config.google_client_secret,
            &config.base_url,
        ),
        auth,
    };

    let app = build_router(app_state);

    #[cfg(feature = "dev")]
    let app = app.layer(LiveReloadLayer::new());

    let addr = format!("{}:{}", config.host, config.port);
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|err| AppError::Internal(format!("Failed to bind {addr}: {err}")))?;
    info!("listening on {}", addr);
    axum::serve(listener, app)
        .await
        .map_err(|err| AppError::Internal(format!("Server error: {err}")))?;

    Ok(())
}

#[cfg(feature = "dev")]
async fn setup_dev_auth_user(
    pool: &Pool,
    dev_auth_email: Option<String>,
) -> Result<deesl::auth::AuthUser, AppError> {
    let user = deesl::user::create_user_if_not_exists(
        pool,
        deesl::models::NewUser::for_email(dev_auth_email.expect("Missing dev email")),
    )
    .await?;

    tracing::info!("dev auth bypass user ready: {}", user.email);

    Ok(deesl::auth::AuthUser {
        user_id: user.id,
        email: user.email,
    })
}
