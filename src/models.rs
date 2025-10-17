use chrono::NaiveDateTime;
use diesel::prelude::*;

#[derive(Queryable, Selectable, serde::Serialize)]
#[diesel(table_name = crate::schema::vehicles)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Vehicle {
    pub id: i32,
    pub make: String,
    pub model: String,
    pub registration: String,
    pub created: NaiveDateTime,
}

#[derive(Insertable, serde::Deserialize)]
#[diesel(table_name = crate::schema::vehicles)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewVehicle {
    pub make: String,
    pub model: String,
    pub registration: String,
}
