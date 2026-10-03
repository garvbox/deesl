mod common;

use common::AuthenticatedRequest;

#[tokio::test]
async fn test_stats_page_when_no_data_renders_empty() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "stats_empty").await;

    let response = env.server.get("/stats").with_auth(&user.token).await;

    response.assert_status_ok();
    assert!(response.text().contains("No Data Yet"));
}

#[tokio::test]
async fn test_stats_page_when_data_present_renders_totals() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "stats_data").await;
    let vehicle_id =
        common::create_test_vehicle_db(&env.pool, user.id, "Stats", "Car", "STATS-1").await;

    common::create_test_entry_db(
        &env.pool,
        vehicle_id,
        None,
        1000,
        40.0,
        60.0,
        chrono::Utc::now().naive_utc(),
    )
    .await;

    let response = env.server.get("/stats").with_auth(&user.token).await;

    response.assert_status_ok();
    let body = response.text();
    assert!(body.contains("Total Entries"));
    assert!(body.contains("STATS-1"));
    assert!(!body.contains("No Data Yet"));
}

#[tokio::test]
async fn test_stats_page_excludes_other_users_entries() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "stats_owner").await;
    let other = common::create_test_user(&env, "stats_other").await;

    let own_vehicle =
        common::create_test_vehicle_db(&env.pool, user.id, "Mine", "Car", "STATS-MINE").await;
    let other_vehicle =
        common::create_test_vehicle_db(&env.pool, other.id, "Theirs", "Car", "STATS-THEIRS").await;

    common::create_test_entry_db(
        &env.pool,
        own_vehicle,
        None,
        1000,
        40.0,
        60.0,
        chrono::Utc::now().naive_utc(),
    )
    .await;
    common::create_test_entry_db(
        &env.pool,
        other_vehicle,
        None,
        2000,
        40.0,
        60.0,
        chrono::Utc::now().naive_utc(),
    )
    .await;

    let response = env.server.get("/stats").with_auth(&user.token).await;

    response.assert_status_ok();
    let body = response.text();
    assert!(body.contains("STATS-MINE"));
    assert!(!body.contains("STATS-THEIRS"));
}

#[tokio::test]
async fn test_stats_page_period_filter_excludes_old_entries() {
    let env = common::create_test_env().await;
    let user = common::create_test_user(&env, "stats_period").await;

    let old_vehicle =
        common::create_test_vehicle_db(&env.pool, user.id, "Old", "Car", "STATS-OLD").await;
    let new_vehicle =
        common::create_test_vehicle_db(&env.pool, user.id, "New", "Car", "STATS-NEW").await;

    let old_date = (chrono::Utc::now() - chrono::Duration::days(400)).naive_utc();
    common::create_test_entry_db(&env.pool, old_vehicle, None, 1000, 40.0, 60.0, old_date).await;
    common::create_test_entry_db(
        &env.pool,
        new_vehicle,
        None,
        2000,
        40.0,
        60.0,
        chrono::Utc::now().naive_utc(),
    )
    .await;

    let recent = env
        .server
        .get("/stats?period=7d")
        .with_auth(&user.token)
        .await;
    recent.assert_status_ok();
    let recent_body = recent.text();
    assert!(recent_body.contains("STATS-NEW"));
    assert!(!recent_body.contains("STATS-OLD"));

    let all = env
        .server
        .get("/stats?period=all")
        .with_auth(&user.token)
        .await;
    all.assert_status_ok();
    let all_body = all.text();
    assert!(all_body.contains("STATS-NEW"));
    assert!(all_body.contains("STATS-OLD"));
}
