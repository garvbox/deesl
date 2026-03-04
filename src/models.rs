use chrono::NaiveDateTime;
use diesel::prelude::*;

#[derive(Queryable, Selectable, serde::Serialize)]
#[diesel(table_name = crate::schema::users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub id: i32,
    pub email: String,
    pub password_hash: Option<String>,
    pub created_at: NaiveDateTime,
    pub currency: String,
    pub google_id: Option<String>,
}

#[derive(Insertable, serde::Deserialize)]
#[diesel(table_name = crate::schema::users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewUser {
    pub email: String,
    pub password_hash: Option<String>,
    pub currency: String,
    pub google_id: Option<String>,
}

#[derive(Queryable, Selectable, serde::Serialize)]
#[diesel(table_name = crate::schema::vehicles)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Vehicle {
    pub id: i32,
    pub make: String,
    pub model: String,
    pub registration: String,
    pub created: NaiveDateTime,
    pub owner_id: i32,
}

#[derive(Insertable, serde::Deserialize)]
#[diesel(table_name = crate::schema::vehicles)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewVehicle {
    pub make: String,
    pub model: String,
    pub registration: String,
    pub owner_id: i32,
}

#[derive(Queryable, Selectable, serde::Serialize)]
#[diesel(table_name = crate::schema::fuel_stations)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct FuelStation {
    pub id: i32,
    pub name: String,
    pub created_at: NaiveDateTime,
    pub user_id: Option<i32>,
}

#[derive(Insertable, serde::Deserialize)]
#[diesel(table_name = crate::schema::fuel_stations)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewFuelStation {
    pub name: String,
    pub user_id: Option<i32>,
}

#[derive(AsChangeset, serde::Deserialize)]
#[diesel(table_name = crate::schema::fuel_stations)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateFuelStation {
    pub name: Option<String>,
}

#[derive(Queryable, Selectable, serde::Serialize)]
#[diesel(table_name = crate::schema::fuel_entries)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct FuelEntry {
    pub id: i32,
    pub vehicle_id: i32,
    pub station_id: Option<i32>,
    pub mileage_km: i32,
    pub litres: f64,
    pub cost: f64,
    pub filled_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, serde::Deserialize)]
#[diesel(table_name = crate::schema::fuel_entries)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewFuelEntry {
    pub vehicle_id: i32,
    pub station_id: Option<i32>,
    pub mileage_km: i32,
    pub litres: f64,
    pub cost: f64,
    pub filled_at: Option<NaiveDateTime>,
}

#[derive(AsChangeset, serde::Deserialize)]
#[diesel(table_name = crate::schema::fuel_entries)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateFuelEntry {
    pub mileage_km: Option<i32>,
    pub litres: Option<f64>,
    pub cost: Option<f64>,
    pub filled_at: Option<NaiveDateTime>,
}

#[derive(Queryable, Selectable, serde::Serialize)]
#[diesel(table_name = crate::schema::vehicle_shares)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[allow(dead_code)]
pub struct VehicleShare {
    pub id: i32,
    pub vehicle_id: i32,
    pub shared_with_user_id: i32,
    pub permission_level: String,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, serde::Deserialize)]
#[diesel(table_name = crate::schema::vehicle_shares)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewVehicleShare {
    pub vehicle_id: i32,
    pub shared_with_user_id: i32,
    pub permission_level: Option<String>,
}
