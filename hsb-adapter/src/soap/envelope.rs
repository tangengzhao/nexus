//! SOAP Envelope 类型定义

use serde::{Deserialize, Serialize};

/// SOAP Envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoapEnvelope<B> {
    /// Header
    pub header: Option<SoapHeader>,
    /// Body
    pub body: SoapBody<B>,
}

/// SOAP Header
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SoapHeader {
    /// WS-Addressing headers
    pub addressing: Option<WsAddressing>,
    /// WS-Security header
    pub security: Option<WsSecurity>,
    /// 自定义 headers
    pub custom: Vec<CustomHeader>,
}

/// SOAP Body
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoapBody<T> {
    /// 内容
    pub content: T,
    /// Fault（如果是错误响应）
    pub fault: Option<SoapFault>,
}

/// SOAP Fault
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoapFault {
    /// 错误代码
    pub code: FaultCode,
    /// 原因
    pub reason: String,
    /// 详情
    pub detail: Option<String>,
    /// 角色
    pub role: Option<String>,
}

/// Fault 代码
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaultCode {
    /// 值
    pub value: String,
    /// 子代码
    pub subcode: Option<Box<FaultCode>>,
}

impl FaultCode {
    pub fn client(subcode: Option<&str>) -> Self {
        Self {
            value: "soap:Client".to_string(),
            subcode: subcode.map(|s| {
                Box::new(FaultCode {
                    value: s.to_string(),
                    subcode: None,
                })
            }),
        }
    }

    pub fn server(subcode: Option<&str>) -> Self {
        Self {
            value: "soap:Server".to_string(),
            subcode: subcode.map(|s| {
                Box::new(FaultCode {
                    value: s.to_string(),
                    subcode: None,
                })
            }),
        }
    }
}

/// WS-Addressing 头
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WsAddressing {
    /// To
    pub to: Option<String>,
    /// Action
    pub action: Option<String>,
    /// MessageID
    pub message_id: Option<String>,
    /// ReplyTo
    pub reply_to: Option<String>,
    /// RelatesTo
    pub relates_to: Option<String>,
    /// From
    pub from: Option<String>,
}

impl WsAddressing {
    pub fn new(action: &str) -> Self {
        Self {
            action: Some(action.to_string()),
            message_id: Some(format!("urn:ulid:{}", ulid::Ulid::new())),
            ..Default::default()
        }
    }

    pub fn with_to(mut self, to: &str) -> Self {
        self.to = Some(to.to_string());
        self
    }

    pub fn with_reply_to(mut self, reply_to: &str) -> Self {
        self.reply_to = Some(reply_to.to_string());
        self
    }
}

/// WS-Security 头
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WsSecurity {
    /// UsernameToken
    pub username_token: Option<UsernameToken>,
    /// 时间戳
    pub timestamp: Option<SecurityTimestamp>,
}

/// UsernameToken
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsernameToken {
    /// 用户名
    pub username: String,
    /// 密码
    pub password: String,
    /// 密码类型
    pub password_type: String,
    /// Nonce
    pub nonce: Option<String>,
    /// 创建时间
    pub created: Option<String>,
}

/// 安全时间戳
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityTimestamp {
    /// 创建时间
    pub created: String,
    /// 过期时间
    pub expires: Option<String>,
}

/// 自定义 Header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomHeader {
    /// 命名空间
    pub namespace: String,
    /// 名称
    pub name: String,
    /// 值
    pub value: String,
    /// 是否必须理解
    pub must_understand: bool,
}

/// SOAP 请求构建器
#[allow(dead_code)]
pub struct SoapRequestBuilder {
    action: String,
    body: String,
    addressing: Option<WsAddressing>,
    security: Option<WsSecurity>,
}

impl SoapRequestBuilder {
    pub fn new(action: &str) -> Self {
        Self {
            action: action.to_string(),
            body: String::new(),
            addressing: None,
            security: None,
        }
    }

    pub fn body(mut self, body: &str) -> Self {
        self.body = body.to_string();
        self
    }

    pub fn with_addressing(mut self, addressing: WsAddressing) -> Self {
        self.addressing = Some(addressing);
        self
    }

    pub fn with_security(mut self, security: WsSecurity) -> Self {
        self.security = Some(security);
        self
    }

    pub fn build_soap11(&self) -> String {
        let header = self.build_header();
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
  {}
  <soap:Body>
    {}
  </soap:Body>
</soap:Envelope>"#,
            header, self.body
        )
    }

    pub fn build_soap12(&self) -> String {
        let header = self.build_header();
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope">
  {}
  <soap:Body>
    {}
  </soap:Body>
</soap:Envelope>"#,
            header, self.body
        )
    }

    fn build_header(&self) -> String {
        let mut parts = Vec::new();

        if let Some(ref addr) = self.addressing {
            if let Some(ref action) = addr.action {
                parts.push(format!(
                    r#"<wsa:Action xmlns:wsa="http://www.w3.org/2005/08/addressing">{}</wsa:Action>"#,
                    action
                ));
            }
            if let Some(ref msg_id) = addr.message_id {
                parts.push(format!(
                    r#"<wsa:MessageID xmlns:wsa="http://www.w3.org/2005/08/addressing">{}</wsa:MessageID>"#,
                    msg_id
                ));
            }
            if let Some(ref to) = addr.to {
                parts.push(format!(
                    r#"<wsa:To xmlns:wsa="http://www.w3.org/2005/08/addressing">{}</wsa:To>"#,
                    to
                ));
            }
        }

        if parts.is_empty() {
            "<soap:Header/>".to_string()
        } else {
            format!(
                "<soap:Header>\n    {}\n  </soap:Header>",
                parts.join("\n    ")
            )
        }
    }
}
