use axum::Router;
use axum::body::Body;
use axum::http::{self, Request, Response};
use deadpool_diesel::postgres::{Manager, Pool};
use diesel::prelude::*;
use tower::ServiceExt;

use deesl::auth::AuthConfig;
use deesl::models::{NewFuelEntry, NewFuelStation, NewUser, NewVehicle, NewVehicleShare};
use deesl::schema::{fuel_entries, fuel_stations, users, vehicle_shares, vehicles};

/// Test user data for creating test fixtures
#[derive(Clone)]
pub struct TestUser {
    pub id: i32,
    pub email: String,
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
    use deesl::AppState;
    use deesl::import_handlers;
    use deesl::oauth_handlers;
    use deesl::user_handlers;
    use deesl::vehicle_fuel_handlers;
    use deesl::vehicle_share_handlers;
    use tower_http::trace::TraceLayer;

    let app_state = AppState {
        pool,
        oauth: oauth_handlers::OAuthConfig::test_config(),
    };

    Router::new()
        .merge(oauth_handlers::router())
        .merge(user_handlers::router())
        .merge(vehicle_fuel_handlers::router())
        .merge(vehicle_share_handlers::router())
        .merge(import_handlers::router())
        .layer(TraceLayer::new_for_http())
        .with_state(app_state)
}

/// Creates a JWT token for a test user
pub fn create_test_token(user_id: i32, email: &str) -> String {
    let auth_config = AuthConfig::new();
    auth_config.create_token(user_id, email).unwrap()
}

/// Makes an authenticated GET request
pub async fn get(app: &Router, path: &str, token: &str) -> Response<Body> {
    let request = Request::builder()
        .uri(path)
        .header(http::header::COOKIE, format!("auth_token={}", token))
        .body(Body::empty())
        .unwrap();

    app.clone().oneshot(request).await.unwrap()
}

/// Makes an authenticated POST request with JSON body
pub async fn post<T: serde::Serialize>(
    app: &Router,
    path: &str,
    token: &str,
    body: T,
) -> Response<Body> {
    let body = serde_json::to_string(&body).unwrap();
    let request = Request::builder()
        .method(http::Method::POST)
        .uri(path)
        .header(http::header::COOKIE, format!("auth_token={}", token))
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    app.clone().oneshot(request).await.unwrap()
}

/// Makes an authenticated PUT request with JSON body
#[allow(dead_code)]
pub async fn put<T: serde::Serialize>(
    app: &Router,
    path: &str,
    token: &str,
    body: T,
) -> Response<Body> {
    let body = serde_json::to_string(&body).unwrap();
    let request = Request::builder()
        .method(http::Method::PUT)
        .uri(path)
        .header(http::header::COOKIE, format!("auth_token={}", token))
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    app.clone().oneshot(request).await.unwrap()
}

/// Makes an authenticated DELETE request
pub async fn delete(app: &Router, path: &str, token: &str) -> Response<Body> {
    let request = Request::builder()
        .method(http::Method::DELETE)
        .uri(path)
        .header(http::header::COOKIE, format!("auth_token={}", token))
        .body(Body::empty())
        .unwrap();

    app.clone().oneshot(request).await.unwrap()
}

/// Makes an authenticated multipart POST request for CSV import
pub async fn post_import_csv(
    app: &Router,
    path: &str,
    token: &str,
    vehicle_id: i32,
    csv_content: &[u8],
    mappings: Option<serde_json::Value>,
) -> Response<Body> {
    use std::io::Write;

    let _boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
    let mut body: Vec<u8> = Vec::new();

    // vehicle_id field
    write!(&mut body, "------WebKitFormBoundary7MA4YWxkTrZu0gW\r\n").unwrap();
    write!(
        &mut body,
        "Content-Disposition: form-data; name=\"vehicle_id\"\r\n\r\n"
    )
    .unwrap();
    write!(&mut body, "{}\r\n", vehicle_id).unwrap();

    // file field
    write!(&mut body, "------WebKitFormBoundary7MA4YWxkTrZu0gW\r\n").unwrap();
    write!(
        &mut body,
        "Content-Disposition: form-data; name=\"file\"; filename=\"test.csv\"\r\n"
    )
    .unwrap();
    write!(&mut body, "Content-Type: text/csv\r\n\r\n").unwrap();
    body.extend_from_slice(csv_content);
    write!(&mut body, "\r\n").unwrap();

    // mappings field (if provided)
    if let Some(mappings) = mappings {
        write!(&mut body, "------WebKitFormBoundary7MA4YWxkTrZu0gW\r\n").unwrap();
        write!(
            &mut body,
            "Content-Disposition: form-data; name=\"mappings\"\r\n\r\n"
        )
        .unwrap();
        write!(&mut body, "{}\r\n", mappings).unwrap();
    }

    // End boundary
    write!(&mut body, "------WebKitFormBoundary7MA4YWxkTrZu0gW--\r\n").unwrap();

    let content_type = "multipart/form-data; boundary=----WebKitFormBoundary7MA4YWxkTrZu0gW";

    let request = Request::builder()
        .method(http::Method::POST)
        .uri(path)
        .header(http::header::COOKIE, format!("auth_token={}", token))
        .header(http::header::CONTENT_TYPE, content_type)
        .body(Body::from(body))
        .unwrap();

    app.clone().oneshot(request).await.unwrap()
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
                .returning((
                    users::id,
                    users::email,
                    users::password_hash,
                    users::created_at,
                    users::currency,
                    users::google_id,
                ))
                .get_result(conn)
        })
        .await
        .unwrap()
        .unwrap();

    let token = create_test_token(user.id, &user.email);

    TestUser {
        id: user.id,
        email: user.email,
        token,
    }
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
                .returning((
                    vehicles::id,
                    vehicles::make,
                    vehicles::model,
                    vehicles::registration,
                    vehicles::created,
                    vehicles::owner_id,
                ))
                .get_result(conn)
        })
        .await
        .unwrap()
        .unwrap();

    vehicle.id
}

/// Creates a vehicle share in the database
pub async fn create_test_share_db(
    pool: &Pool,
    vehicle_id: i32,
    shared_with_user_id: i32,
    permission_level: &str,
) {
    let conn = pool.get().await.unwrap();
    let permission_level = permission_level.to_string();

    conn.interact(move |conn| {
        diesel::insert_into(vehicle_shares::table)
            .values(NewVehicleShare {
                vehicle_id,
                shared_with_user_id,
                permission_level: Some(permission_level),
            })
            .execute(conn)
    })
    .await
    .unwrap()
    .unwrap();
}

/// Creates a fuel station in the database
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
                .returning((
                    fuel_stations::id,
                    fuel_stations::name,
                    fuel_stations::created_at,
                    fuel_stations::user_id,
                ))
                .get_result(conn)
        })
        .await
        .unwrap()
        .unwrap();

    station.id
}

/// Creates a fuel entry in the database
pub async fn create_test_fuel_entry_db(
    pool: &Pool,
    vehicle_id: i32,
    station_id: Option<i32>,
    mileage_km: i32,
    litres: f64,
    cost: f64,
) -> i32 {
    let conn = pool.get().await.unwrap();

    let entry: deesl::models::FuelEntry = conn
        .interact(move |conn| {
            diesel::insert_into(fuel_entries::table)
                .values(NewFuelEntry {
                    vehicle_id,
                    station_id,
                    mileage_km,
                    litres,
                    cost,
                    filled_at: Some(chrono::Utc::now().naive_utc()),
                })
                .returning((
                    fuel_entries::id,
                    fuel_entries::vehicle_id,
                    fuel_entries::station_id,
                    fuel_entries::mileage_km,
                    fuel_entries::litres,
                    fuel_entries::cost,
                    fuel_entries::filled_at,
                    fuel_entries::created_at,
                ))
                .get_result(conn)
        })
        .await
        .unwrap()
        .unwrap();

    entry.id
}

/// Cleans up test data (useful for cleanup between tests)
#[allow(dead_code)]
pub async fn cleanup_test_data(pool: &Pool) {
    let conn = pool.get().await.unwrap();

    conn.interact(|conn| {
        diesel::delete(fuel_entries::table).execute(conn)?;
        diesel::delete(vehicle_shares::table).execute(conn)?;
        diesel::delete(fuel_stations::table).execute(conn)?;
        diesel::delete(vehicles::table).execute(conn)?;
        diesel::delete(users::table).execute(conn)?;
        Ok::<_, diesel::result::Error>(())
    })
    .await
    .unwrap()
    .unwrap();
}

/// Parses JSON response body
pub async fn parse_json_response<T: serde::de::DeserializeOwned>(response: Response<Body>) -> T {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}
