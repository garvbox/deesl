use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

#[derive(OpenApi)]
#[openapi(
    components(
        schemas(
            RegisterRequest,
            LoginRequest,
            AuthResponse,
        )
    ),
    tags(
        (name = "auth", description = "Authentication endpoints")
    )
)]
pub struct ApiDoc;

#[derive(Deserialize, ToSchema)]
#[allow(dead_code)]
pub struct RegisterRequest {
    #[schema(example = "user@example.com")]
    pub email: String,
    #[schema(example = "password123")]
    pub password: String,
}

#[derive(Deserialize, ToSchema)]
#[allow(dead_code)]
pub struct LoginRequest {
    #[schema(example = "user@example.com")]
    pub email: String,
    #[schema(example = "password123")]
    pub password: String,
}

#[derive(Serialize, ToSchema)]
pub struct AuthResponse {
    #[schema(example = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...")]
    pub token: String,
    #[schema(example = 1)]
    pub user_id: i32,
    #[schema(example = "user@example.com")]
    pub email: String,
}
