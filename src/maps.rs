use crate::profile::Location;

use futures::TryStreamExt;
use mongodb::bson::doc;
use mongodb::options::ClientOptions;

use std::sync::Arc;
use thiserror::Error;

/// Performs all geocoding operations.
///
/// Using Yandex Maps API
pub struct Maps {
    client: mongodb::Client,
    api_key: String,
}

impl Maps {
    pub async fn init(api_key: String) -> Arc<Self> {
        let options = ClientOptions::parse("mongodb://localhost:27017/deafbot")
            .await
            .unwrap();

        Arc::new(Self {
            client: mongodb::Client::with_options(options).unwrap(),
            api_key,
        })
    }

    /// Gets location from cache or fetches it using Yandex Maps API
    pub async fn get_actual_city(&self, input: &str) -> Result<Location, FetchingError> {
        if let Some(location) = self.get_cached_location(input).await {
            return Ok(location);
        }

        let geocode_json = self.fetch_geocode(input).await?;

        let actual_city = match Self::city_name_from_geocode_json(geocode_json) {
            Some(city) => city,
            None => return Err(FetchingError::CityNotFound { name: input.to_owned() })
        };

        let location = Location::new(input.to_owned(), actual_city, None);

        self.cache_location(&location).await;

        Ok(location)
    }

    async fn fetch_geocode(&self, geocode: &str) -> Result<serde_json::Value, FetchingError> {
        let payload = format!(
            "https://geocode-maps.yandex.ru/1.x\
            ?apikey={}\
            &format=json\
            &results=1\
            &geocode={geocode}",
            self.api_key
        );

        let response = match reqwest::get(payload).await {
            Ok(response) => response,
            Err(error) => return Err(FetchingError::ReqwestError { error }),
        };

        if !response.status().is_success() {
            return Err(FetchingError::UnsuccessfulRequest {
                response_code: response.status().as_u16(),
            });
        }

        match response.json::<serde_json::Value>().await {
            Ok(data) => Ok(data),
            Err(_) => Err(FetchingError::JsonNotFound),
        }
    }

    async fn cache_location(&self, location: &Location) {
        self.location_cache_collection()
            .insert_one(location, None)
            .await
            .unwrap();
    }

    /// Tries to get location from `location_cache` collection. Returns `None` if not found.
    async fn get_cached_location(&self, location: &str) -> Option<Location> {
        let mut cursor = match self
            .location_cache_collection()
            .find(doc! { "displayed": location }, None)
            .await
        {
            Ok(curs) => curs,
            Err(_) => return None,
        };

        cursor.try_next().await.map_or(None, |cl| cl)
    }

    /// Tries to get city name from a geocode.
    fn city_name_from_geocode_json(geocode_json: serde_json::Value) -> Option<String> {
        let intersection = &geocode_json["response"]["GeoObjectCollection"]["featureMember"][0]
            ["GeoObject"]["metaDataProperty"]["GeocoderMetaData"]["AddressDetails"]["Country"]
            ["AdministrativeArea"];

        if let Some(city) =
            intersection["SubAdministrativeArea"]["Locality"]["LocalityName"].as_str()
        {
            return Some(city.to_owned());
        }

        if let Some(city) = intersection["Locality"]["LocalityName"].as_str() {
            return Some(city.to_owned());
        }

        intersection["AdministrativeAreaName"].as_str()
            .map(|el| el.to_owned())
    }

    #[inline]
    fn location_cache_collection(&self) -> mongodb::Collection<Location> {
        self.local().collection("location_cache")
    }

    #[inline]
    fn local(&self) -> mongodb::Database {
        self.client.database("deafbot")
    }
}

#[derive(Debug, Error)]
pub enum FetchingError {
    #[error("reqwest error: {error:?}")]
    ReqwestError {
        #[from]
        error: reqwest::Error,
    },

    #[error("unsuccessful request. response code {response_code}")]
    UnsuccessfulRequest { response_code: u16 },

    #[error("json serialization error")]
    JsonNotFound,

    #[error("city `{name}` doesn't exist")]
    CityNotFound { name: String }
}
