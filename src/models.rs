use chrono::NaiveDateTime;
use diesel::prelude::*;
use std::collections::HashMap;

#[derive(Queryable, Selectable, serde::Serialize, AsChangeset)]
#[diesel(table_name = crate::schema::users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub id: i32,
    pub email: String,
    pub password_hash: Option<String>,
    pub created_at: NaiveDateTime,
    pub currency: String,
    pub google_id: Option<String>,
    pub distance_unit: String,
    pub volume_unit: String,
}

#[derive(AsChangeset, serde::Deserialize)]
#[diesel(table_name = crate::schema::users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UpdateUser {
    pub currency: Option<String>,
    pub distance_unit: Option<String>,
    pub volume_unit: Option<String>,
}

#[derive(Insertable, serde::Deserialize)]
#[diesel(table_name = crate::schema::users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewUser {
    pub email: String,
    pub password_hash: Option<String>,
    pub currency: String,
    pub google_id: Option<String>,
    pub distance_unit: String,
    pub volume_unit: String,
}

#[derive(Queryable, Selectable, serde::Serialize, Clone)]
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

#[derive(Queryable, Selectable, serde::Serialize, Clone)]
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
    pub station_id: Option<i32>,
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

#[derive(Queryable, Selectable, serde::Serialize)]
#[diesel(table_name = crate::schema::temp_imports)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct TempImport {
    pub id: uuid::Uuid,
    pub user_id: i32,
    pub vehicle_id: i32,
    pub csv_data: Vec<u8>,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, serde::Deserialize)]
#[diesel(table_name = crate::schema::temp_imports)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NewTempImport {
    pub user_id: i32,
    pub vehicle_id: i32,
    pub csv_data: Vec<u8>,
}

impl FuelEntry {
    pub fn cost_per_litre(&self) -> f64 {
        if self.litres > 0.0 {
            self.cost / self.litres
        } else {
            0.0
        }
    }

    pub fn km_since(&self, prev_mileage: i32) -> f64 {
        (self.mileage_km - prev_mileage) as f64
    }

    pub fn efficiency_km_per_litre(&self, prev_mileage: i32) -> f64 {
        let km = self.km_since(prev_mileage);
        if km > 0.0 && self.litres > 0.0 {
            km / self.litres
        } else {
            0.0
        }
    }

    pub fn cost_per_km(&self, prev_mileage: i32) -> f64 {
        let km = self.km_since(prev_mileage);
        if km > 0.0 { self.cost / km } else { 0.0 }
    }
}

impl Vehicle {
    pub fn display_name(&self) -> String {
        format!("{} {}", self.make, self.model)
    }
}

pub struct VehicleStats {
    pub vehicle: Vehicle,
    pub entries: Vec<FuelEntry>,
    pub total_entries: i64,
    pub total_km: f64,
    pub total_litres: f64,
    pub total_cost: f64,
    pub avg_efficiency: f64,
}

impl VehicleStats {
    pub fn calculate(vehicle: Vehicle, entries: Vec<FuelEntry>) -> Self {
        let total_entries = entries.len() as i64;
        let total_litres: f64 = entries.iter().map(|e| e.litres).sum();
        let total_cost: f64 = entries.iter().map(|e| e.cost).sum();

        let mut total_km = 0.0;
        let mut efficiency_sum = 0.0;
        let mut efficiency_count = 0;
        let mut prev_mileage: Option<i32> = None;

        for entry in &entries {
            if let Some(prev) = prev_mileage {
                let km = entry.km_since(prev);
                if km > 0.0 && entry.litres > 0.0 {
                    let efficiency = entry.efficiency_km_per_litre(prev);
                    efficiency_sum += efficiency;
                    efficiency_count += 1;
                    total_km += km;
                }
            }
            prev_mileage = Some(entry.mileage_km);
        }

        let avg_efficiency = if efficiency_count > 0 {
            efficiency_sum / efficiency_count as f64
        } else {
            0.0
        };

        VehicleStats {
            vehicle,
            entries,
            total_entries,
            total_km,
            total_litres,
            total_cost,
            avg_efficiency,
        }
    }
}

pub struct StatsAggregator {
    pub total_entries: usize,
    pub total_litres: f64,
    pub total_cost: f64,
    pub avg_efficiency: f64,
    pub avg_cost_per_km: f64,
    pub vehicle_stats: Vec<VehicleStats>,
}

impl StatsAggregator {
    pub fn calculate(vehicle_data: &HashMap<i32, Vec<(FuelEntry, Vehicle)>>) -> Self {
        let mut vehicle_stats = Vec::new();
        let mut total_entries = 0;

        for vehicle_entries in vehicle_data.values() {
            let vehicle = &vehicle_entries[0].1;
            let entries: Vec<FuelEntry> = vehicle_entries.iter().map(|(e, _)| e).cloned().collect();
            total_entries += entries.len();

            let stats = VehicleStats::calculate(vehicle.clone(), entries);
            vehicle_stats.push(stats);
        }

        let total_litres: f64 = vehicle_stats.iter().map(|v| v.total_litres).sum();
        let total_cost: f64 = vehicle_stats.iter().map(|v| v.total_cost).sum();
        let total_km: f64 = vehicle_stats.iter().map(|v| v.total_km).sum();

        let avg_efficiency = if !vehicle_stats.is_empty() {
            vehicle_stats.iter().map(|v| v.avg_efficiency).sum::<f64>() / vehicle_stats.len() as f64
        } else {
            0.0
        };

        let avg_cost_per_km = if total_km > 0.0 {
            total_cost / total_km
        } else {
            0.0
        };

        StatsAggregator {
            total_entries,
            total_litres,
            total_cost,
            avg_efficiency,
            avg_cost_per_km,
            vehicle_stats,
        }
    }
}
