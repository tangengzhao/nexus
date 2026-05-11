//! SSO OAuth 2.0 / CAS 客户端。
//!
//! 该模块从原始示例工程整合而来，用于服务端统一处理 SSO 登录跳转、
//! 回调换票、用户信息获取以及 CAS 登录集成。

use std::collections::HashMap;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{HsbError, HsbResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub sub: String,
    pub username: String,
    pub name: String,
    pub email: String,
    pub email_verified: Option<bool>,
    pub roles: Option<Vec<String>>,
    pub applications: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct SSOClient {
    server_url: String,
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    http: Client,
}

impl SSOClient {
    /// 创建新的 SSO 客户端。
    pub fn new(
        server_url: &str,
        client_id: &str,
        client_secret: &str,
        redirect_uri: &str,
    ) -> HsbResult<Self> {
        let http = Client::builder()
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .map_err(|e| HsbError::ConnectionError {
                endpoint: server_url.to_string(),
                message: format!("Failed to create SSO HTTP client: {}", e),
            })?;

        Ok(Self {
            server_url: server_url.trim_end_matches('/').to_string(),
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            redirect_uri: redirect_uri.to_string(),
            http,
        })
    }

    /// 生成 OAuth 授权跳转地址与 state。
    pub fn get_authorization_url(&self, scope: &str) -> (String, String) {
        let state = Self::generate_state();
        let url = format!(
            "{}/oauth/authorize?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}",
            self.server_url,
            urlencoding::encode(&self.client_id),
            urlencoding::encode(&self.redirect_uri),
            urlencoding::encode(scope),
            urlencoding::encode(&state),
        );

        (url, state)
    }

    /// 使用授权码换取访问令牌。
    pub async fn exchange_code_for_token(&self, code: &str) -> HsbResult<TokenResponse> {
        let params = [
            ("grant_type", "authorization_code".to_string()),
            ("code", code.to_string()),
            ("redirect_uri", self.redirect_uri.clone()),
            ("client_id", self.client_id.clone()),
            ("client_secret", self.client_secret.clone()),
        ];

        let resp = self
            .http
            .post(format!("{}/oauth/token", self.server_url))
            .form(&params)
            .send()
            .await
            .map_err(|e| HsbError::ConnectionError {
                endpoint: self.server_url.clone(),
                message: format!("Failed to exchange OAuth code: {}", e),
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(HsbError::AuthenticationError {
                message: format!("Token exchange failed: status={}, body={}", status, body),
            });
        }

        resp.json::<TokenResponse>()
            .await
            .map_err(|e| HsbError::SerializationError {
                message: format!("Failed to deserialize token response: {}", e),
            })
    }

    /// 刷新访问令牌。
    pub async fn refresh_access_token(&self, refresh_token: &str) -> HsbResult<TokenResponse> {
        let params = [
            ("grant_type", "refresh_token".to_string()),
            ("refresh_token", refresh_token.to_string()),
            ("client_id", self.client_id.clone()),
            ("client_secret", self.client_secret.clone()),
        ];

        let resp = self
            .http
            .post(format!("{}/oauth/token", self.server_url))
            .form(&params)
            .send()
            .await
            .map_err(|e| HsbError::ConnectionError {
                endpoint: self.server_url.clone(),
                message: format!("Failed to refresh OAuth token: {}", e),
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(HsbError::AuthenticationError {
                message: format!("Token refresh failed: status={}, body={}", status, body),
            });
        }

        resp.json::<TokenResponse>()
            .await
            .map_err(|e| HsbError::SerializationError {
                message: format!("Failed to deserialize refreshed token: {}", e),
            })
    }

    /// 获取当前登录用户信息。
    pub async fn get_user_info(&self, access_token: &str) -> HsbResult<UserInfo> {
        let resp = self
            .http
            .get(format!("{}/api/user/profile", self.server_url))
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| HsbError::ConnectionError {
                endpoint: self.server_url.clone(),
                message: format!("Failed to get user info: {}", e),
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(HsbError::AuthenticationError {
                message: format!("Failed to get user info: status={}, body={}", status, body),
            });
        }

        resp.json::<UserInfo>()
            .await
            .map_err(|e| HsbError::SerializationError {
                message: format!("Failed to deserialize user info: {}", e),
            })
    }

    /// 调用 SSO 登出接口。
    pub async fn logout(&self, access_token: Option<&str>) -> HsbResult<()> {
        let mut req = self.http.get(format!("{}/api/logout", self.server_url));

        if let Some(token) = access_token {
            req = req.bearer_auth(token);
        }

        let resp = req.send().await.map_err(|e| HsbError::ConnectionError {
            endpoint: self.server_url.clone(),
            message: format!("Failed to request logout: {}", e),
        })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(HsbError::AuthenticationError {
                message: format!("Logout failed: status={}, body={}", status, body),
            });
        }

        Ok(())
    }

    fn generate_state() -> String {
        let mut bytes = [0_u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        URL_SAFE_NO_PAD.encode(bytes)
    }
}

#[derive(Debug, Clone)]
pub struct CASClient {
    server_url: String,
    service_url: String,
    http: Client,
}

#[derive(Debug, Clone)]
pub struct CASValidationResponse {
    pub success: bool,
    pub username: Option<String>,
    pub attributes: HashMap<String, String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

impl CASClient {
    pub fn new(server_url: &str, service_url: &str) -> Self {
        Self {
            server_url: server_url.trim_end_matches('/').to_string(),
            service_url: service_url.to_string(),
            http: Client::new(),
        }
    }

    pub fn get_login_url(&self) -> String {
        format!(
            "{}/cas/login?service={}",
            self.server_url,
            urlencoding::encode(&self.service_url)
        )
    }

    pub async fn validate_ticket(&self, ticket: &str) -> HsbResult<CASValidationResponse> {
        let url = format!(
            "{}/cas/serviceValidate?service={}&ticket={}",
            self.server_url,
            urlencoding::encode(&self.service_url),
            urlencoding::encode(ticket),
        );

        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| HsbError::ConnectionError {
                endpoint: self.server_url.clone(),
                message: format!("Failed to validate CAS ticket: {}", e),
            })?;

        let body = resp.text().await.map_err(|e| HsbError::TransportError {
            message: format!("Failed to read CAS validation response: {}", e),
        })?;

        if body.contains("authenticationSuccess") {
            let username = Self::extract_xml_value(&body, "cas:user")
                .or_else(|| Self::extract_xml_value(&body, "user"));
            let mut attributes = HashMap::new();

            if let Some(name) = Self::extract_xml_value(&body, "cas:name") {
                attributes.insert("name".to_string(), name);
            }

            if let Some(email) = Self::extract_xml_value(&body, "cas:email") {
                attributes.insert("email".to_string(), email);
            }

            Ok(CASValidationResponse {
                success: true,
                username,
                attributes,
                error_code: None,
                error_message: None,
            })
        } else {
            Ok(CASValidationResponse {
                success: false,
                username: None,
                attributes: HashMap::new(),
                error_code: Some("INVALID_TICKET".to_string()),
                error_message: Some("Ticket validation failed".to_string()),
            })
        }
    }

    pub fn get_logout_url(&self) -> String {
        format!(
            "{}/cas/logout?service={}",
            self.server_url,
            urlencoding::encode(&self.service_url)
        )
    }

    fn extract_xml_value(xml: &str, tag: &str) -> Option<String> {
        let open = format!("<{}>", tag);
        let close = format!("</{}>", tag);

        if let Some(start) = xml.find(&open) {
            let start = start + open.len();
            if let Some(end) = xml[start..].find(&close) {
                return Some(xml[start..start + end].to_string());
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::{CASClient, SSOClient};

    fn callback_url() -> String {
        std::env::var("HSB_SSO_CALLBACK_URL")
            .unwrap_or_else(|_| "http://hsb-server:8080/auth/callback".to_string())
    }

    fn protected_url() -> String {
        std::env::var("HSB_SSO_PROTECTED_URL")
            .unwrap_or_else(|_| "http://hsb-server:8080/protected".to_string())
    }

    #[test]
    fn authorization_url_contains_callback_and_state() {
        let callback_url = callback_url();
        let client = SSOClient::new(
            "https://rust-sso.example.internal",
            "hsb-web",
            "secret",
            &callback_url,
        )
        .expect("client should build");

        let (url, state) = client.get_authorization_url("openid profile email");

        assert!(url.contains("response_type=code"));
        assert!(url.contains(&format!(
            "redirect_uri={}",
            urlencoding::encode(&callback_url)
        )));
        assert!(url.contains("scope=openid%20profile%20email"));
        assert!(!state.is_empty());
    }

    #[test]
    fn cas_login_url_is_generated() {
        let client = CASClient::new("https://rust-sso.example.internal", &protected_url());

        let url = client.get_login_url();
        assert!(url.contains("/cas/login?service="));
    }
}
