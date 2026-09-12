use leo_llm::ToolSpec;

use crate::error::Error;
use crate::get_weather;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "get_forecast".into(),
        description: "Pronóstico diario. Pasa `place` o `latitude` y `longitude`.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "place": { "type": "string", "description": "Ciudad o lugar" },
                "latitude": { "type": "number" },
                "longitude": { "type": "number" },
                "days": { "type": "integer", "description": "Días (1-16, por defecto 7)" }
            }
        }),
    }
}

pub async fn run(
    http: &reqwest::Client,
    place: Option<&str>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    days: Option<u64>,
) -> Result<serde_json::Value, Error> {
    let (lat, lon, label) = get_weather::resolve(http, place, latitude, longitude).await?;
    let days = days.unwrap_or(7).clamp(1, 16);
    let v: serde_json::Value = http
        .get("https://api.open-meteo.com/v1/forecast")
        .query(&[
            ("latitude", lat.to_string()),
            ("longitude", lon.to_string()),
            (
                "daily",
                "weather_code,temperature_2m_max,temperature_2m_min,precipitation_sum,wind_speed_10m_max".into(),
            ),
            ("forecast_days", days.to_string()),
            ("timezone", "auto".into()),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(serde_json::json!({
        "ok": true,
        "place": label,
        "latitude": lat,
        "longitude": lon,
        "daily": v.get("daily"),
        "timezone": v.get("timezone"),
    }))
}
