use deadpool_diesel::postgres::Pool;

use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};

use crate::models;
use crate::{AppError, schema::users};

pub async fn create_user_if_not_exists(pool: &Pool, user: models::NewUser) -> Result<(), AppError> {
    let conn = pool.get().await?;

    let _ = conn
        .interact(move |conn| {
            let exists = users::table
                .filter(users::email.eq(&user.email))
                .first::<models::User>(conn)
                .is_ok();

            if !exists {
                tracing::trace!("Creating user: {:?}", user.email);
                let _ = diesel::insert_into(users::table).values(user).execute(conn);
            } else {
                tracing::trace!("user exists already: {:?}", user.email);
            }
        })
        .await;

    Ok(())
}
