use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct AuthConfig {
    /// setting `url` means we first attempt to authenticate the user from the url and if fails
    /// we perform local authentication.
    url: Option<String>,
    access_exp: i64,
    refresh_exp: i64,
}

impl AuthConfig {
    pub fn url(&self) -> &Option<String> {
        &self.url
    }

    pub fn access_exp(&self) -> &i64 {
        &self.access_exp
    }

    pub fn refresh_exp(&self) -> &i64 {
        &self.refresh_exp
    }
}
