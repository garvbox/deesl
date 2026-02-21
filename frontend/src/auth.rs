use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub user_id: i32,
    pub email: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

pub async fn login(email: String, password: String) -> Result<AuthResponse, String> {
    let client = reqwest::Client::new();
    let response = client
        .post("/api/auth/login")
        .json(&LoginRequest { email, password })
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    if response.status().is_success() {
        response
            .json::<AuthResponse>()
            .await
            .map_err(|e| format!("Parse error: {}", e))
    } else {
        let text = response.text().await.unwrap_or_default();
        Err(format!("Login failed: {}", text))
    }
}

pub async fn register(email: String, password: String) -> Result<AuthResponse, String> {
    let client = reqwest::Client::new();
    let response = client
        .post("/api/auth/register")
        .json(&RegisterRequest { email, password })
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    if response.status().is_success() {
        response
            .json::<AuthResponse>()
            .await
            .map_err(|e| format!("Parse error: {}", e))
    } else {
        let text = response.text().await.unwrap_or_default();
        Err(format!("Registration failed: {}", text))
    }
}

pub fn store_auth(auth: &AuthResponse) {
    if let Some(window) = web_sys::window()
        && let Ok(Some(storage)) = window.local_storage()
    {
        let _ = storage.set_item("auth_token", &auth.token);
        let _ = storage.set_item("user_id", &auth.user_id.to_string());
        let _ = storage.set_item("user_email", &auth.email);
    }
}

pub fn get_stored_auth() -> Option<AuthResponse> {
    let window = web_sys::window()?;
    let storage = window.local_storage().ok()??;
    let token = storage.get_item("auth_token").ok()??;
    let user_id: i32 = storage.get_item("user_id").ok()??.parse().ok()?;
    let email = storage.get_item("user_email").ok()??;
    Some(AuthResponse {
        token,
        user_id,
        email,
    })
}

pub fn clear_auth() {
    if let Some(window) = web_sys::window()
        && let Ok(Some(storage)) = window.local_storage()
    {
        let _ = storage.remove_item("auth_token");
        let _ = storage.remove_item("user_id");
        let _ = storage.remove_item("user_email");
    }
}
