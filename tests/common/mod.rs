use axum::Router;
use deadpool_diesel::postgres::{Manager, Pool};
use diesel::prelude::*;

use deesl::auth::AuthConfig;
use deesl::models::{NewFuelStation, NewUser, NewVehicle};
use deesl::schema::{fuel_stations, users, vehicles};

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
    let pool = Pool::builder(manager).build().unwrap();

    // Run migrations (only needed for first test, diesel tracks which ran)
    let conn = pool
        .get()
        .await
        .expect("Failed to get connection from pool");
    let _ = conn
        .interact(|conn| {
            use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
            const MIGRATIONS: EmbeddedMigrations = embed_migrations!();
            // Use ignore to avoid panicking if migrations already ran or table exists
            let _ = conn.run_pending_migrations(MIGRATIONS);
        })
        .await;

    pool
}

/// Creates a test app with the given database pool
pub async fn create_test_app(pool: Pool) -> Router {
    use axum::routing::get;
    use deesl::handlers;
    use deesl::oauth_handlers;
    use tower_http::trace::TraceLayer;

    let app_state = deesl::AppState {
        pool,
        oauth: oauth_handlers::OAuthConfig::test_config(),
        auth: deesl::auth::AuthConfig::new(),
    };

    Router::new()
        .route(
            "/",
            get(|| async { axum::response::Redirect::to("/dashboard") }),
        )
        .merge(handlers::auth::router())
        .merge(handlers::misc::router())
        .merge(handlers::settings::router())
        .nest("/vehicles", handlers::vehicles::router())
        .nest("/fuel-entries", handlers::fuel_entries::router())
        .nest("/stations", handlers::stations::router())
        .nest("/stats", handlers::stats::router())
        .nest("/import", handlers::import::router())
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
                    distance_unit: "km".to_string(),
                    volume_unit: "L".to_string(),
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

/// Creates a vehicle share in the database
pub async fn create_test_vehicle_share_db(
    pool: &Pool,
    vehicle_id: i32,
    shared_with_user_id: i32,
    permission_level: &str,
) {
    let conn = pool.get().await.unwrap();
    let permission_level = permission_level.to_string();

    conn.interact(move |conn| {
        diesel::insert_into(deesl::schema::vehicle_shares::table)
            .values((
                deesl::schema::vehicle_shares::vehicle_id.eq(vehicle_id),
                deesl::schema::vehicle_shares::shared_with_user_id.eq(shared_with_user_id),
                deesl::schema::vehicle_shares::permission_level.eq(permission_level),
            ))
            .execute(conn)
    })
    .await
    .unwrap()
    .unwrap();
}

/// Creates a test fuel station in the database
pub async fn create_test_station_db(pool: &Pool, user_id: i32, name: &str) -> i32 {
    let conn = pool.get().await.unwrap();
    let name = name.to_string();

    let station: deesl::models::FuelStation = conn
        .interact(move |conn| {
            diesel::insert_into(fuel_stations::table)
                .values(NewFuelStation {
                    name,
                    user_id: Some(user_id),
                })
                .get_result(conn)
        })
        .await
        .unwrap()
        .unwrap();

    station.id
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

/// Posts import execute data as form (not multipart, since file is already stored)
pub async fn post_import_execute(
    server: &TestServer,
    token: &str,
    import_id: &str,
    vehicle_id: i32,
    mappings: std::collections::HashMap<String, String>,
) -> TestResponse {
    use std::collections::HashMap;

    let mut form_data: HashMap<String, String> = HashMap::new();
    form_data.insert("import_id".to_string(), import_id.to_string());
    form_data.insert("vehicle_id".to_string(), vehicle_id.to_string());

    for (k, v) in mappings {
        form_data.insert(k, v);
    }

    server
        .post("/import/htmx/execute")
        .add_header("Cookie", format!("auth_token={}", token))
        .form(&form_data)
        .await
}
