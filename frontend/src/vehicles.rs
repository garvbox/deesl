use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vehicle {
    pub id: i32,
    pub make: String,
    pub model: String,
    pub registration: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateVehicleRequest {
    pub make: String,
    pub model: String,
    pub registration: String,
    pub owner_id: i32,
}

pub async fn list_vehicles(user_id: i32) -> Result<Vec<Vehicle>, String> {
    let client = reqwest::Client::new();
    let response = client
        .get(&format!("/api/vehicles?user_id={}", user_id))
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    if response.status().is_success() {
        response
            .json::<Vec<Vehicle>>()
            .await
            .map_err(|e| format!("Parse error: {}", e))
    } else {
        let text = response.text().await.unwrap_or_default();
        Err(format!("Failed to fetch vehicles: {}", text))
    }
}

pub async fn create_vehicle(
    make: String,
    model: String,
    registration: String,
    owner_id: i32,
) -> Result<Vehicle, String> {
    let client = reqwest::Client::new();
    let response = client
        .post("/api/vehicles")
        .json(&CreateVehicleRequest {
            make,
            model,
            registration,
            owner_id,
        })
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    if response.status().is_success() {
        response
            .json::<Vehicle>()
            .await
            .map_err(|e| format!("Parse error: {}", e))
    } else {
        let text = response.text().await.unwrap_or_default();
        Err(format!("Failed to create vehicle: {}", text))
    }
}

pub async fn delete_vehicle(id: i32) -> Result<(), String> {
    let client = reqwest::Client::new();
    let response = client
        .delete(&format!("/api/vehicles/{}", id))
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    if response.status().is_success() {
        Ok(())
    } else {
        let text = response.text().await.unwrap_or_default();
        Err(format!("Failed to delete vehicle: {}", text))
    }
}
