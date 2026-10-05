#![allow(dead_code)]

use deadpool_diesel::postgres::{Manager, Pool};
use diesel::prelude::*;

use deesl::auth::AuthConfig;
use deesl::models::{NewFuelStation, NewUser, NewVehicle};
use deesl::schema::{fuel_stations, users, vehicles};

#[derive(Clone)]
pub struct TestUser {
    pub id: i32,
    pub token: String,
}

pub async fn create_test_pool() -> Pool {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/deesl_test".to_string());
    let manager = Manager::new(database_url, deadpool_diesel::Runtime::Tokio1);
    let pool = Pool::builder(manager).build().unwrap();

    // Tests share a database and build their pool concurrently, so a failure here is
    // usually a lost race on `__diesel_schema_migrations` rather than a broken schema.
    if let Err(err) = deesl::db::run_migrations(&pool).await {
        tracing::warn!("migrations reported: {:?}", err);
    }

    pool
}

pub fn create_test_token(user_id: i32, email: &str) -> String {
    let auth_config = AuthConfig::new("test-secret", 168);
    auth_config.create_token(user_id, email).unwrap()
}

use axum_test::TestResponse;
use axum_test::TestServer;

pub struct TestEnv {
    pub server: TestServer,
    pub pool: Pool,
}

pub async fn create_test_env() -> TestEnv {
    build_test_env(deesl::oauth_handlers::OAuthConfig::test_config()).await
}

pub async fn create_test_env_with_oauth(oauth: deesl::oauth_handlers::OAuthConfig) -> TestEnv {
    build_test_env(oauth).await
}

async fn build_test_env(oauth: deesl::oauth_handlers::OAuthConfig) -> TestEnv {
    let pool = create_test_pool().await;
    let app_state = deesl::AppState {
        pool: pool.clone(),
        oauth,
        auth: AuthConfig::new("test-secret", 168),
    };
    let app = deesl::app::build_router(app_state);
    let server = TestServer::new(app).unwrap();

    TestEnv { server, pool }
}

pub async fn create_test_user(env: &TestEnv, prefix: &str) -> TestUser {
    let email = format!("{}_{}@test.com", prefix, uuid::Uuid::new_v4());
    create_test_user_db(&env.pool, &email).await
}

pub trait AuthenticatedRequest {
    fn with_auth(self, token: &str) -> Self;
}

impl AuthenticatedRequest for axum_test::TestRequest {
    fn with_auth(self, token: &str) -> Self {
        self.add_header("Cookie", format!("auth_token={}", token))
    }
}

pub async fn create_test_user_db(pool: &Pool, email: &str) -> TestUser {
    let conn = pool.get().await.unwrap();
    let email = email.to_string();

    let user: deesl::models::User = conn
        .interact(move |conn| {
            diesel::insert_into(users::table)
                .values(NewUser::for_email(&email))
                .get_result(conn)
        })
        .await
        .unwrap()
        .unwrap();

    let token = create_test_token(user.id, &user.email);

    TestUser { id: user.id, token }
}

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

pub async fn create_unique_vehicle_db(pool: &Pool, owner_id: i32) -> i32 {
    let registration = format!("REG-{}", uuid::Uuid::new_v4());
    create_test_vehicle_db(pool, owner_id, "Make", "Model", &registration).await
}

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

pub async fn create_test_global_station_db(pool: &Pool, name: &str) -> i32 {
    let conn = pool.get().await.unwrap();
    let name = name.to_string();

    let station: deesl::models::FuelStation = conn
        .interact(move |conn| {
            diesel::insert_into(fuel_stations::table)
                .values(NewFuelStation {
                    name,
                    user_id: None,
                })
                .get_result(conn)
        })
        .await
        .unwrap()
        .unwrap();

    station.id
}

pub async fn create_test_entry_db(
    pool: &Pool,
    vehicle_id: i32,
    station_id: Option<i32>,
    mileage_km: i32,
    litres: f64,
    cost: f64,
    filled_at: chrono::NaiveDateTime,
) -> i32 {
    let conn = pool.get().await.unwrap();

    let entry: deesl::models::FuelEntry = conn
        .interact(move |conn| {
            diesel::insert_into(deesl::schema::fuel_entries::table)
                .values((
                    deesl::schema::fuel_entries::vehicle_id.eq(vehicle_id),
                    deesl::schema::fuel_entries::station_id.eq(station_id),
                    deesl::schema::fuel_entries::mileage_km.eq(mileage_km),
                    deesl::schema::fuel_entries::litres.eq(litres),
                    deesl::schema::fuel_entries::cost.eq(cost),
                    deesl::schema::fuel_entries::filled_at.eq(filled_at),
                ))
                .get_result(conn)
        })
        .await
        .unwrap()
        .unwrap();

    entry.id
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

pub fn assert_login_redirect(response: &TestResponse) {
    response.assert_status(axum::http::StatusCode::SEE_OTHER);
    assert_eq!(response.header("location"), "/login");
}

pub fn assert_unauthorized(response: &TestResponse) {
    response.assert_status(axum::http::StatusCode::UNAUTHORIZED);
}

pub fn assert_hx_redirect(response: &TestResponse, path: &str) {
    response.assert_status(axum::http::StatusCode::SEE_OTHER);
    assert_eq!(response.header("HX-Redirect"), path);
    assert_eq!(response.header("location"), path);
}
