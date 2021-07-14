pub mod option_pin;
pub mod transport;

pub(crate) type Error = Box<dyn std::error::Error + Send + Sync>;
