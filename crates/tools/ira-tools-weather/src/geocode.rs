use crate::error::Error;

#[derive(Clone, Debug)]
pub struct Place {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub country: Option<String>,
}

pub async fn lookup(http: &reqwest::Client, query: &str) -> Result<Place, Error> {
    let url = "https://geocoding-api.open-meteo.com/v1/search";
    let v: serde_json::Value = http
        .get(url)
        .query(&[("name", query), ("count", "1"), ("language", "es")])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let first = v
        .get("results")
        .and_then(|r| r.as_array())
        .and_then(|a| a.first())
        .ok_or_else(|| Error::msg(format!("no se encontró el lugar `{query}`")))?;
    Ok(Place {
        name: first
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(query)
            .to_string(),
        latitude: first
            .get("latitude")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| Error::msg("geocoding sin latitud"))?,
        longitude: first
            .get("longitude")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| Error::msg("geocoding sin longitud"))?,
        country: first
            .get("country")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    })
}

pub fn coords_from_args(lat: Option<f64>, lon: Option<f64>) -> Option<(f64, f64)> {
    match (lat, lon) {
        (Some(latitude), Some(longitude)) => Some((latitude, longitude)),
        _ => None,
    }
}
