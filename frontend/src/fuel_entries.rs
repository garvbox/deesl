use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FuelStation {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuelEntry {
    pub id: i32,
    pub vehicle_id: i32,
    pub station_id: Option<i32>,
    pub station_name: Option<String>,
    pub mileage_km: i32,
    pub litres: f64,
    pub cost: f64,
    pub filled_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateFuelEntryRequest {
    pub vehicle_id: i32,
    pub station_id: Option<i32>,
    pub mileage_km: i32,
    pub litres: f64,
    pub cost: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateFuelStationRequest {
    pub name: String,
}

pub async fn list_fuel_stations() -> Result<Vec<FuelStation>, String> {
    let client = reqwest::Client::new();
    let response = client
        .get("/api/fuel-stations")
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    if response.status().is_success() {
        response
            .json::<Vec<FuelStation>>()
            .await
            .map_err(|e| format!("Parse error: {}", e))
    } else {
        let text = response.text().await.unwrap_or_default();
        Err(format!("Failed to fetch stations: {}", text))
    }
}

pub async fn create_fuel_station(name: String) -> Result<FuelStation, String> {
    let client = reqwest::Client::new();
    let response = client
        .post("/api/fuel-stations")
        .json(&CreateFuelStationRequest { name })
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    if response.status().is_success() {
        response
            .json::<FuelStation>()
            .await
            .map_err(|e| format!("Parse error: {}", e))
    } else {
        let text = response.text().await.unwrap_or_default();
        Err(format!("Failed to create station: {}", text))
    }
}

pub async fn list_fuel_entries(vehicle_id: i32) -> Result<Vec<FuelEntry>, String> {
    let client = reqwest::Client::new();
    let response = client
        .get(&format!("/api/fuel-entries?vehicle_id={}", vehicle_id))
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    if response.status().is_success() {
        response
            .json::<Vec<FuelEntry>>()
            .await
            .map_err(|e| format!("Parse error: {}", e))
    } else {
        let text = response.text().await.unwrap_or_default();
        Err(format!("Failed to fetch entries: {}", text))
    }
}

pub async fn create_fuel_entry(
    vehicle_id: i32,
    station_id: Option<i32>,
    mileage_km: i32,
    litres: f64,
    cost: f64,
) -> Result<FuelEntry, String> {
    let client = reqwest::Client::new();
    let response = client
        .post("/api/fuel-entries")
        .json(&CreateFuelEntryRequest {
            vehicle_id,
            station_id,
            mileage_km,
            litres,
            cost,
        })
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    if response.status().is_success() {
        response
            .json::<FuelEntry>()
            .await
            .map_err(|e| format!("Parse error: {}", e))
    } else {
        let text = response.text().await.unwrap_or_default();
        Err(format!("Failed to create entry: {}", text))
    }
}

pub async fn delete_fuel_entry(id: i32) -> Result<(), String> {
    let client = reqwest::Client::new();
    let response = client
        .delete(&format!("/api/fuel-entries/{}", id))
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    if response.status().is_success() {
        Ok(())
    } else {
        let text = response.text().await.unwrap_or_default();
        Err(format!("Failed to delete entry: {}", text))
    }
}
