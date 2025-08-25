use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct CreateUserClient {
    pub max_bucket: Option<u64>,
}

#[derive(Deserialize, Serialize)]
pub struct LoginUserClient {
    pub id: String,
    pub access_exp: Option<i64>,
    pub refresh_exp: Option<i64>,
}

#[derive(Serialize, Deserialize)]
pub struct LoginToken {
    pub access: Option<(String, i64)>,
    pub refresh: Option<(String, i64)>,
}