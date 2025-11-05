use validator::Validate;

use crate::{AppResult, errors::Error};

#[cfg(feature = "api")]
pub mod api;

pub mod internal;


pub trait OptionValidator: Validate {
    fn validate_data(&self) -> AppResult<()> {
        self.validate().map_err(|err| Error::ValidationError(err.to_string()))?;
        Ok(())
    }
}

#[macro_export]
macro_rules! impl_validator {
    ($( $model:ty ),+) => {
        $(
            impl OptionValidator for $model {}
        )*
    };
}
