use axum::Router;
use deadpool_diesel::postgres::{Manager, Pool};
use diesel::prelude::*;

use deesl::auth::AuthConfig;
use deesl::models::{NewUser, NewVehicle};
use deesl::schema::{users, vehicles};

/// Test user data for creating test fixtures
#[derive(Clone)]
pub struct TestUser {
    pub id: i32,
    pub token: String,
}

/// Creates a test database pool connected to the test database
pub async fn create_test_pool() -> Pool {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/deesl_test".to_string());
    let manager = Manager::new(database_url, deadpool_diesel::Runtime::Tokio1);
    Pool::builder(manager).build().unwrap()
}

/// Creates a test app with the given database pool
pub async fn create_test_app(pool: Pool) -> Router {
    use axum::routing::{delete, get, post};
    use deesl::handlers;
    use deesl::oauth_handlers;
    use tower_http::trace::TraceLayer;

    let app_state = deesl::AppState {
        pool,
        oauth: oauth_handlers::OAuthConfig::test_config(),
    };

    Router::new()
        .route(
            "/",
            get(|| async { axum::response::Redirect::to("/dashboard") }),
        )
        .route("/login", get(handlers::login))
        .route("/logout", get(oauth_handlers::logout))
        .route("/dashboard", get(handlers::dashboard))
        .route(
            "/settings",
            get(handlers::settings_page).patch(handlers::update_settings),
        )
        .route(
            "/vehicles",
            get(handlers::vehicles_page).post(handlers::create_vehicle),
        )
        .route("/vehicles/new", get(handlers::new_vehicle))
        .route("/fuel-entries/new", get(handlers::new_fuel_entry))
        .route("/fuel-entries", post(handlers::create_fuel_entry))
        .route("/import", get(handlers::import_page))
        .route("/htmx/import/preview", post(handlers::htmx_import_preview))
        .route("/htmx/import/execute", post(handlers::htmx_import_execute))
        .route("/htmx/vehicles", get(handlers::htmx_vehicles))
        .route("/htmx/vehicles/{id}", delete(handlers::htmx_delete_vehicle))
        .route("/htmx/entries/recent", get(handlers::htmx_recent_entries))
        .merge(oauth_handlers::router())
        .layer(TraceLayer::new_for_http())
        .with_state(app_state)
}

/// Creates a JWT token for a test user
pub fn create_test_token(user_id: i32, email: &str) -> String {
    let auth_config = AuthConfig::new();
    auth_config.create_token(user_id, email).unwrap()
}

// ============================================================================
// AXUM-TEST BASED HELPERS
// ============================================================================

use axum_test::TestResponse;
use axum_test::TestServer;

/// Test environment using axum-test's TestServer for cleaner testing
pub struct TestEnv {
    pub server: TestServer,
    pub pool: Pool,
}

/// Creates a test environment with axum-test TestServer
pub async fn create_test_env() -> TestEnv {
    let pool = create_test_pool().await;
    let app = create_test_app(pool.clone()).await;
    let server = TestServer::new(app).unwrap();

    TestEnv { server, pool }
}

/// Creates a test user and returns the user with a configured server
pub async fn create_test_user(env: &TestEnv, prefix: &str) -> TestUser {
    let email = format!("{}_{}@test.com", prefix, uuid::Uuid::new_v4());
    create_test_user_db(&env.pool, &email).await
}

/// Extension trait for TestServer to add authentication
pub trait AuthenticatedRequest {
    fn with_auth(self, token: &str) -> Self;
}

impl AuthenticatedRequest for axum_test::TestRequest {
    fn with_auth(self, token: &str) -> Self {
        self.add_header("Cookie", format!("auth_token={}", token))
    }
}

/// Creates a test user in the database
pub async fn create_test_user_db(pool: &Pool, email: &str) -> TestUser {
    let conn = pool.get().await.unwrap();
    let email = email.to_string();

    let user: deesl::models::User = conn
        .interact(move |conn| {
            diesel::insert_into(users::table)
                .values(NewUser {
                    email: email.clone(),
                    password_hash: None,
                    currency: "EUR".to_string(),
                    google_id: None,
                })
                .get_result(conn)
        })
        .await
        .unwrap()
        .unwrap();

    let token = create_test_token(user.id, &user.email);

    TestUser { id: user.id, token }
}

/// Creates a test vehicle in the database
pub async fn create_test_vehicle_db(
    pool: &Pool,
    owner_id: i32,
    make: &str,
    model: &str,
    registration: &str,
) -> i32 {
    let conn = pool.get().await.unwrap();
    let make = make.to_string();
    let model = model.to_string();
    let registration = registration.to_string();

    let vehicle: deesl::models::Vehicle = conn
        .interact(move |conn| {
            diesel::insert_into(vehicles::table)
                .values(NewVehicle {
                    make,
                    model,
                    registration,
                    owner_id,
                })
                .get_result(conn)
        })
        .await
        .unwrap()
        .unwrap();

    vehicle.id
}

pub async fn post_import_csv(
    server: &TestServer,
    path: &str,
    token: &str,
    vehicle_id: i32,
    csv_content: &[u8],
    mappings: Option<std::collections::HashMap<String, String>>,
) -> TestResponse {
    use axum_test::multipart::{MultipartForm, Part};

    let mut form = MultipartForm::new()
        .add_part("vehicle_id", Part::text(vehicle_id.to_string()))
        .add_part(
            "file",
            Part::bytes(csv_content.to_vec()).file_name("test.csv"),
        );

    if let Some(m) = mappings {
        for (k, v) in m {
            form = form.add_part(k, Part::text(v));
        }
    }

    server
        .post(path)
        .add_header("Cookie", format!("auth_token={}", token))
        .multipart(form)
        .await
}
