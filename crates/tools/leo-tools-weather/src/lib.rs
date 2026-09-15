//! Weather and forecast tools backed by Open-Meteo, without API credentials.
//!
//! Location lookup is internal; public operations expose schemas and typed
//! execution for registration in `leo-tools`.

mod error;
mod geocode;
/// Retrieves a weather forecast for a location.
pub mod get_forecast;
/// Retrieves current weather for a location.
pub mod get_weather;

pub use error::Error;
