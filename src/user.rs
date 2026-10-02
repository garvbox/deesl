use deadpool_diesel::postgres::Pool;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, SelectableHelper};

use crate::models;
use crate::{AppError, schema::users};

pub async fn create_user_if_not_exists(
    pool: &Pool,
    user: models::NewUser,
) -> Result<models::User, AppError> {
    let conn = pool.get().await?;
    let email = user.email.clone();

    // `DO NOTHING` leaves zero rows in the RETURNING clause when the user is already
    // present, which surfaces as `NotFound` and is the signal to read the row back.
    let user: models::User = conn
        .interact(move |conn| {
            diesel::insert_into(users::table)
                .values(user)
                .on_conflict(users::email)
                .do_nothing()
                .returning(models::User::as_returning())
                .get_result(conn)
                .or_else(|err| match err {
                    diesel::result::Error::NotFound => users::table
                        .filter(users::email.eq(&email))
                        .first::<models::User>(conn),
                    other => Err(other),
                })
        })
        .await??;

    tracing::trace!("user ready: {:?}", user.email);

    Ok(user)
}
