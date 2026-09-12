use leo_llm::ToolSpec;

use crate::error::Error;
use crate::geocode;

pub fn spec() -> ToolSpec {
    ToolSpec {
        name: "get_weather".into(),
        description: "Tiempo actual. Pasa `place` (ciudad) o `latitude` y `longitude`.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "place": { "type": "string", "description": "Ciudad o lugar" },
                "latitude": { "type": "number" },
                "longitude": { "type": "number" }
            }
        }),
    }
}

pub async fn run(
    http: &reqwest::Client,
    place: Option<&str>,
    latitude: Option<f64>,
    longitude: Option<f64>,
) -> Result<serde_json::Value, Error> {
    let (lat, lon, label) = resolve(http, place, latitude, longitude).await?;
    let v: serde_json::Value = http
        .get("https://api.open-meteo.com/v1/forecast")
        .query(&[
            ("latitude", lat.to_string()),
            ("longitude", lon.to_string()),
            (
                "current",
                "temperature_2m,relative_humidity_2m,apparent_temperature,weather_code,wind_speed_10m,precipitation".into(),
            ),
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
        "current": v.get("current"),
        "timezone": v.get("timezone"),
    }))
}

pub(crate) async fn resolve(
    http: &reqwest::Client,
    place: Option<&str>,
    latitude: Option<f64>,
    longitude: Option<f64>,
) -> Result<(f64, f64, String), Error> {
    if let Some((lat, lon)) = geocode::coords_from_args(latitude, longitude) {
        let label = place.unwrap_or("coordenadas").to_string();
        return Ok((lat, lon, label));
    }
    let place = place.ok_or_else(|| Error::msg("falta place o latitude/longitude"))?;
    let found = geocode::lookup(http, place).await?;
    let label = match found.country {
        Some(c) => format!("{}, {c}", found.name),
        None => found.name,
    };
    Ok((found.latitude, found.longitude, label))
}
