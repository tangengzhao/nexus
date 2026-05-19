//! HTTP 客户端

use bytes::Bytes;
use hsb_common::{HsbError, HsbResult};
use reqwest::Client;
use std::collections::HashMap;
use std::time::Duration;

use crate::HttpTransportConfig;

/// HTTP 客户端
pub struct HttpClient {
    client: Client,
    config: HttpTransportConfig,
}

impl HttpClient {
    pub fn new(config: HttpTransportConfig) -> HsbResult<Self> {
        let mut builder = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .pool_max_idle_per_host(config.pool_config.max_connections as usize);

        if config.disable_certificate_validation && insecure_tls_override_enabled() {
            builder = builder.danger_accept_invalid_certs(true);
        }

        let client = builder.build().map_err(|e| HsbError::ConfigError {
            message: format!("Failed to create HTTP client: {}", e),
        })?;

        Ok(Self { client, config })
    }

    /// 发送 GET 请求
    pub async fn get(&self, url: &str) -> HsbResult<HttpResponse> {
        self.request(HttpMethod::Get, url, None, HashMap::new())
            .await
    }

    /// 发送 POST 请求
    pub async fn post(&self, url: &str, body: Bytes) -> HsbResult<HttpResponse> {
        self.request(HttpMethod::Post, url, Some(body), HashMap::new())
            .await
    }

    /// 发送请求
    pub async fn request(
        &self,
        method: HttpMethod,
        url: &str,
        body: Option<Bytes>,
        headers: HashMap<String, String>,
    ) -> HsbResult<HttpResponse> {
        let full_url = if let Some(ref base) = self.config.base_url {
            format!("{}{}", base, url)
        } else {
            url.to_string()
        };

        let mut req_builder = match method {
            HttpMethod::Get => self.client.get(&full_url),
            HttpMethod::Post => self.client.post(&full_url),
            HttpMethod::Put => self.client.put(&full_url),
            HttpMethod::Delete => self.client.delete(&full_url),
            HttpMethod::Patch => self.client.patch(&full_url),
        };

        // 添加默认头
        for (key, value) in &self.config.default_headers {
            req_builder = req_builder.header(key, value);
        }

        // 添加自定义头
        for (key, value) in &headers {
            req_builder = req_builder.header(key, value);
        }

        if let Some(body) = body {
            req_builder = req_builder.body(body);
        }

        let resp = req_builder
            .send()
            .await
            .map_err(|e| HsbError::TransportError {
                message: format!("HTTP request failed: {}", e),
            })?;

        let status = resp.status().as_u16();
        let headers = resp
            .headers()
            .iter()
            .filter_map(|(k, v)| {
                v.to_str()
                    .ok()
                    .map(|v| (k.as_str().to_string(), v.to_string()))
            })
            .collect();

        let body = resp.bytes().await.map_err(|e| HsbError::TransportError {
            message: format!("Failed to read response: {}", e),
        })?;

        Ok(HttpResponse {
            status,
            headers,
            body,
        })
    }
}

fn insecure_tls_override_enabled() -> bool {
    std::env::var("HSB_ALLOW_INSECURE_TLS")
        .map(|value| value.eq_ignore_ascii_case("true") || value == "1")
        .unwrap_or(false)
}

/// HTTP 方法
#[derive(Debug, Clone, Copy)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

/// HTTP 响应
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Bytes,
}

impl HttpResponse {
    pub fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }

    pub fn text(&self) -> HsbResult<String> {
        String::from_utf8(self.body.to_vec()).map_err(|e| HsbError::ParseError {
            message: format!("Invalid UTF-8 in response: {}", e),
        })
    }
}
