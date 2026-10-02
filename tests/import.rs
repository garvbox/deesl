use diesel::prelude::*;

mod common;

use common::AuthenticatedRequest;

#[tokio::test]
async fn test_import_page_loads_with_auth() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "import_page").await;

    let response = env.server.get("/import").with_auth(&user.token).await;

    response.assert_status_ok();
    assert!(response.text().contains("Import Fuel Entries"));
}

#[tokio::test]
async fn test_import_preview_accepts_csv() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "import_test").await;
    let vehicle_id =
        common::create_test_vehicle_db(&env.pool, user.id, "Import", "Car", "IMP-1").await;

    let csv_content = b"Date,Litres,Cost,Mileage\n2024-03-01,40.5,60.0,10000";

    let response = common::post_import_csv(
        &env.server,
        "/import/htmx/preview",
        &user.token,
        vehicle_id,
        csv_content,
        None,
    )
    .await;

    response.assert_status_ok();
    assert!(response.text().contains("Map CSV Columns"));
    assert!(response.text().contains("2024-03-01"));
}

#[tokio::test]
async fn test_import_preview_when_vehicle_id_missing_is_bad_request() {
    use axum_test::multipart::{MultipartForm, Part};

    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "preview_no_vehicle").await;

    let form = MultipartForm::new().add_part(
        "file",
        Part::bytes(b"Date,Litres\n2024-03-01,1".to_vec()).file_name("test.csv"),
    );

    let response = env
        .server
        .post("/import/htmx/preview")
        .with_auth(&user.token)
        .multipart(form)
        .await;

    response.assert_status(axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_import_preview_when_file_missing_is_bad_request() {
    use axum_test::multipart::{MultipartForm, Part};

    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "preview_no_file").await;
    let vehicle_id = common::create_unique_vehicle_db(&env.pool, user.id).await;

    let form = MultipartForm::new().add_part("vehicle_id", Part::text(vehicle_id.to_string()));

    let response = env
        .server
        .post("/import/htmx/preview")
        .with_auth(&user.token)
        .multipart(form)
        .await;

    response.assert_status(axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_import_preview_when_vehicle_id_invalid_is_bad_request() {
    use axum_test::multipart::{MultipartForm, Part};

    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "preview_bad_vehicle").await;

    let form = MultipartForm::new()
        .add_part("vehicle_id", Part::text("not-a-number"))
        .add_part(
            "file",
            Part::bytes(b"Date,Litres\n2024-03-01,1".to_vec()).file_name("test.csv"),
        );

    let response = env
        .server
        .post("/import/htmx/preview")
        .with_auth(&user.token)
        .multipart(form)
        .await;

    response.assert_status(axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_import_preview_when_no_write_access_is_forbidden() {
    let env = common::create_test_env().await;
    let owner = common::create_test_user(&env, "preview_owner").await;
    let other = common::create_test_user(&env, "preview_other").await;
    let vehicle_id = common::create_unique_vehicle_db(&env.pool, owner.id).await;

    common::create_test_vehicle_share_db(&env.pool, vehicle_id, other.id, "read").await;

    let response = common::post_import_csv(
        &env.server,
        "/import/htmx/preview",
        &other.token,
        vehicle_id,
        b"Date,Litres,Cost,Mileage\n2024-03-01,40.5,60.0,10000",
        None,
    )
    .await;

    response.assert_status(axum::http::StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_htmx_import_execute() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "import_exec").await;
    let vehicle_id =
        common::create_test_vehicle_db(&env.pool, user.id, "Import", "Car", "IMP-2").await;

    let csv_content = b"Date,Litres,Cost,Mileage\n2024-03-01,40.5,60.0,10000";

    let preview_response = common::post_import_csv(
        &env.server,
        "/import/htmx/preview",
        &user.token,
        vehicle_id,
        csv_content,
        None,
    )
    .await;

    preview_response.assert_status_ok();
    let preview_html = preview_response.text();

    let import_id = preview_html
        .split("name=\"import_id\" value=\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    let mut mappings = std::collections::HashMap::new();
    mappings.insert("col_0".to_string(), "Date".to_string());
    mappings.insert("map_0".to_string(), "filled_at_date".to_string());
    mappings.insert("col_1".to_string(), "Litres".to_string());
    mappings.insert("map_1".to_string(), "litres".to_string());
    mappings.insert("col_2".to_string(), "Cost".to_string());
    mappings.insert("map_2".to_string(), "cost".to_string());
    mappings.insert("col_3".to_string(), "Mileage".to_string());
    mappings.insert("map_3".to_string(), "mileage_km".to_string());

    let response =
        common::post_import_execute(&env.server, &user.token, import_id, vehicle_id, mappings)
            .await;

    response.assert_status_ok();
    assert!(response.text().contains("Import Successful"));
    assert!(response.text().contains("1"));
}

#[tokio::test]
async fn test_import_execute_when_import_id_missing_is_bad_request() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "exec_no_import_id").await;

    let response = env
        .server
        .post("/import/htmx/execute")
        .with_auth(&user.token)
        .form(&[("vehicle_id", "1")])
        .await;

    response.assert_status(axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_import_execute_when_import_id_invalid_is_bad_request() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "exec_bad_import_id").await;

    let response = env
        .server
        .post("/import/htmx/execute")
        .with_auth(&user.token)
        .form(&[("import_id", "not-a-uuid"), ("vehicle_id", "1")])
        .await;

    response.assert_status(axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_import_execute_when_import_not_found_is_not_found() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "exec_missing_import").await;
    let vehicle_id = common::create_unique_vehicle_db(&env.pool, user.id).await;

    let response = env
        .server
        .post("/import/htmx/execute")
        .with_auth(&user.token)
        .form(&[
            ("import_id", uuid::Uuid::new_v4().to_string()),
            ("vehicle_id", vehicle_id.to_string()),
        ])
        .await;

    response.assert_status(axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_import_execute_when_duplicate_mapping_reports_error() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "exec_dup_mapping").await;
    let vehicle_id = common::create_unique_vehicle_db(&env.pool, user.id).await;

    let preview = common::post_import_csv(
        &env.server,
        "/import/htmx/preview",
        &user.token,
        vehicle_id,
        b"Date,Other\n2024-03-01,x",
        None,
    )
    .await;
    preview.assert_status_ok();
    let preview_html = preview.text();
    let import_id = preview_html
        .split("name=\"import_id\" value=\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    let mut mappings = std::collections::HashMap::new();
    mappings.insert("col_0".to_string(), "Date".to_string());
    mappings.insert("map_0".to_string(), "filled_at_date".to_string());
    mappings.insert("col_1".to_string(), "Other".to_string());
    mappings.insert("map_1".to_string(), "filled_at_date".to_string());

    let response =
        common::post_import_execute(&env.server, &user.token, import_id, vehicle_id, mappings)
            .await;

    response.assert_status_ok();
    assert!(response.text().contains("mapped multiple times"));
}

#[tokio::test]
async fn test_import_execute_when_duplicate_entry_is_skipped() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "exec_duplicate").await;
    let vehicle_id = common::create_unique_vehicle_db(&env.pool, user.id).await;

    let filled_at = chrono::NaiveDate::from_ymd_opt(2024, 3, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    common::create_test_entry_db(&env.pool, vehicle_id, None, 10000, 40.5, 60.0, filled_at).await;

    let preview = common::post_import_csv(
        &env.server,
        "/import/htmx/preview",
        &user.token,
        vehicle_id,
        b"Date,Litres,Cost,Mileage\n2024-03-01,40.5,60.0,10000",
        None,
    )
    .await;
    preview.assert_status_ok();
    let preview_html = preview.text();
    let import_id = preview_html
        .split("name=\"import_id\" value=\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    let mut mappings = std::collections::HashMap::new();
    mappings.insert("col_0".to_string(), "Date".to_string());
    mappings.insert("map_0".to_string(), "filled_at_date".to_string());
    mappings.insert("col_1".to_string(), "Litres".to_string());
    mappings.insert("map_1".to_string(), "litres".to_string());
    mappings.insert("col_2".to_string(), "Cost".to_string());
    mappings.insert("map_2".to_string(), "cost".to_string());
    mappings.insert("col_3".to_string(), "Mileage".to_string());
    mappings.insert("map_3".to_string(), "mileage_km".to_string());

    let response =
        common::post_import_execute(&env.server, &user.token, import_id, vehicle_id, mappings)
            .await;

    response.assert_status_ok();
    let body = response.text();
    assert!(body.contains("Import Successful"));
    assert!(body.contains("Skipped"));
}

#[tokio::test]
async fn test_import_execute_creates_station() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "exec_station").await;
    let vehicle_id = common::create_unique_vehicle_db(&env.pool, user.id).await;

    let preview = common::post_import_csv(
        &env.server,
        "/import/htmx/preview",
        &user.token,
        vehicle_id,
        b"Date,Litres,Cost,Mileage,Location\n2024-03-01,40.5,60.0,10000,MyShell",
        None,
    )
    .await;
    preview.assert_status_ok();
    let preview_html = preview.text();
    let import_id = preview_html
        .split("name=\"import_id\" value=\"")
        .nth(1)
        .unwrap()
        .split("\"")
        .next()
        .unwrap();

    let mut mappings = std::collections::HashMap::new();
    mappings.insert("col_0".to_string(), "Date".to_string());
    mappings.insert("map_0".to_string(), "filled_at_date".to_string());
    mappings.insert("col_1".to_string(), "Litres".to_string());
    mappings.insert("map_1".to_string(), "litres".to_string());
    mappings.insert("col_2".to_string(), "Cost".to_string());
    mappings.insert("map_2".to_string(), "cost".to_string());
    mappings.insert("col_3".to_string(), "Mileage".to_string());
    mappings.insert("map_3".to_string(), "mileage_km".to_string());
    mappings.insert("col_4".to_string(), "Location".to_string());
    mappings.insert("map_4".to_string(), "station".to_string());

    let response =
        common::post_import_execute(&env.server, &user.token, import_id, vehicle_id, mappings)
            .await;

    response.assert_status_ok();
    assert!(response.text().contains("Stations Created"));

    let conn = env.pool.get().await.unwrap();
    let user_id = user.id;
    let station_exists: bool = conn
        .interact(move |conn| {
            deesl::schema::fuel_stations::table
                .filter(deesl::schema::fuel_stations::user_id.eq(user_id))
                .filter(deesl::schema::fuel_stations::name.eq("MyShell"))
                .first::<deesl::models::FuelStation>(conn)
                .optional()
                .map(|s| s.is_some())
        })
        .await
        .unwrap()
        .unwrap();
    assert!(station_exists);
}
