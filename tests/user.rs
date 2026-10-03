use diesel::prelude::*;

mod common;

async fn count_users_with_email(pool: &deadpool_diesel::postgres::Pool, email: &str) -> i64 {
    let conn = pool.get().await.unwrap();
    let email = email.to_string();

    conn.interact(move |conn| {
        deesl::schema::users::table
            .filter(deesl::schema::users::email.eq(&email))
            .count()
            .get_result(conn)
    })
    .await
    .unwrap()
    .unwrap()
}

#[tokio::test]
async fn test_create_user_if_not_exists_inserts_user_when_user_absent() {
    let pool = common::create_test_pool().await;
    let email = format!("absent_{}@test.com", uuid::Uuid::new_v4());

    let user =
        deesl::user::create_user_if_not_exists(&pool, deesl::models::NewUser::for_email(&email))
            .await
            .unwrap();

    assert_eq!(user.email, email);
    assert_eq!(user.currency, "EUR");
    assert_eq!(user.distance_unit, "km");
    assert_eq!(user.volume_unit, "L");
    assert_eq!(count_users_with_email(&pool, &email).await, 1);
}

#[tokio::test]
async fn test_create_user_if_not_exists_returns_existing_user_when_called_twice() {
    let pool = common::create_test_pool().await;
    let email = format!("twice_{}@test.com", uuid::Uuid::new_v4());

    let first =
        deesl::user::create_user_if_not_exists(&pool, deesl::models::NewUser::for_email(&email))
            .await
            .unwrap();
    let second =
        deesl::user::create_user_if_not_exists(&pool, deesl::models::NewUser::for_email(&email))
            .await
            .unwrap();

    assert_eq!(first.id, second.id);
    assert_eq!(count_users_with_email(&pool, &email).await, 1);
}

#[tokio::test]
async fn test_create_user_if_not_exists_keeps_existing_preferences_when_user_present() {
    let pool = common::create_test_pool().await;
    let email = format!("keep_{}@test.com", uuid::Uuid::new_v4());

    let created = common::create_test_user_db(&pool, &email).await;
    let conn = pool.get().await.unwrap();
    conn.interact(move |conn| {
        diesel::update(deesl::schema::users::table.filter(deesl::schema::users::id.eq(created.id)))
            .set(deesl::schema::users::currency.eq("GBP"))
            .execute(conn)
    })
    .await
    .unwrap()
    .unwrap();

    let user =
        deesl::user::create_user_if_not_exists(&pool, deesl::models::NewUser::for_email(&email))
            .await
            .unwrap();

    assert_eq!(user.id, created.id);
    assert_eq!(user.currency, "GBP");
    assert_eq!(count_users_with_email(&pool, &email).await, 1);
}

#[tokio::test]
async fn test_create_user_if_not_exists_returns_error_when_google_id_conflicts() {
    let pool = common::create_test_pool().await;
    let google_id = format!("gid_{}", uuid::Uuid::new_v4());

    let conn = pool.get().await.unwrap();
    let owner_email = format!("owner_{}@test.com", uuid::Uuid::new_v4());
    let seed_email = owner_email.clone();
    let seed_google_id = google_id.clone();
    conn.interact(move |conn| {
        diesel::insert_into(deesl::schema::users::table)
            .values(deesl::models::NewUser {
                google_id: Some(seed_google_id),
                ..deesl::models::NewUser::for_email(&seed_email)
            })
            .execute(conn)
    })
    .await
    .unwrap()
    .unwrap();

    let result = deesl::user::create_user_if_not_exists(
        &pool,
        deesl::models::NewUser {
            google_id: Some(google_id.clone()),
            ..deesl::models::NewUser::for_email(format!("other_{}@test.com", uuid::Uuid::new_v4()))
        },
    )
    .await;

    assert!(matches!(
        result,
        Err(deesl::AppError::Database(
            deesl::error::DatabaseError::UniqueViolation(_)
        ))
    ));
}
