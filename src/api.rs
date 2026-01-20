use std::env;
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct TranslateRequest<'a> {
    text: Vec<&'a str>,
    source_lang: &'a str,
    target_lang: &'a str,
}

#[derive(Debug, Deserialize)]
struct TranslateResponse {
    translations: Vec<TranslationItem>,
}

#[derive(Debug, Deserialize)]
struct TranslationItem {
    text: String,
}

pub struct PtruiApi {
    pub client: reqwest::blocking::Client,
    pub url: String,
    pub auth_header: Option<String>,
    pub auth_value: Option<String>,
}

impl PtruiApi {
    pub fn from_env() -> Result<Self, String> {
        let url = env::var("TRANSLATION_API_URL")
            .map_err(|_| "Missing TRANSLATION_API_URL environment variable".to_string())?;
        let auth_key = env::var("TRANSLATION_API_KEY").ok();
        let auth_header = env::var("TRANSLATION_API_AUTH_HEADER").ok();

        let (header_name, header_value) = match auth_key {
            Some(key) => {
                let header = auth_header.unwrap_or_else(|| "Authorization".to_string());
                let value = if header.eq_ignore_ascii_case("Authorization") {
                    format!("DeepL-Auth-Key {}", key)
                } else {
                    key
                };
                (Some(header), Some(value))
            }
            None => (None, None),
        };

        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|err| format!("Failed to build HTTP client: {}", err))?;

        Ok(Self {
            client,
            url,
            auth_header: header_name,
            auth_value: header_value,
        })
    }
}

pub fn translate_via_api(
    api: &PtruiApi,
    text: &str,
    source_lang: &str,
    target_lang: &str,
) -> Result<String, String> {
    let payload = TranslateRequest {
        text: vec![text],
        source_lang,
        target_lang,
    };
    let mut request = api.client.post(&api.url).json(&payload);
    if let (Some(header), Some(value)) = (&api.auth_header, &api.auth_value) {
        request = request.header(header, value);
    }
    let response = request
        .send()
        .map_err(|err| format!("Failed to call translation API: {}", err))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("Translation API error ({}): {}", status, body));
    }

    let response: TranslateResponse = response
        .json()
        .map_err(|err| format!("Invalid API response: {}", err))?;
    response
        .translations
        .into_iter()
        .next()
        .map(|item| item.text)
        .ok_or_else(|| "API response missing translations".to_string())
}
