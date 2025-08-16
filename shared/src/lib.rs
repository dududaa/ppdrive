pub use tracing;

pub enum AuthType {
    Client,
    User,
    None,
}

impl AuthType {
    /// record keeping is only required when authtype is not `None`
    pub fn keep_record(&self) -> bool {
        match self {
            AuthType::None => false,
            _ => true,
        }
    }
}
