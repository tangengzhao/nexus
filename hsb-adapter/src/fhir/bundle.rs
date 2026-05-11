//! FHIR Bundle 处理

use serde::{Deserialize, Serialize};

use super::resources::{Identifier, Meta};

/// FHIR Bundle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    /// 资源类型
    #[serde(rename = "resourceType")]
    pub resource_type: String,
    /// ID
    pub id: Option<String>,
    /// 元数据
    pub meta: Option<Meta>,
    /// 标识符
    pub identifier: Option<Identifier>,
    /// 类型
    #[serde(rename = "type")]
    pub bundle_type: BundleType,
    /// 时间戳
    pub timestamp: Option<String>,
    /// 总数
    pub total: Option<u32>,
    /// 链接
    pub link: Option<Vec<BundleLink>>,
    /// 条目
    pub entry: Option<Vec<BundleEntry>>,
    /// 签名
    pub signature: Option<serde_json::Value>,
}

impl Bundle {
    pub fn new(bundle_type: BundleType) -> Self {
        Self {
            resource_type: "Bundle".to_string(),
            id: None,
            meta: None,
            identifier: None,
            bundle_type,
            timestamp: None,
            total: None,
            link: None,
            entry: None,
            signature: None,
        }
    }

    pub fn transaction() -> Self {
        Self::new(BundleType::Transaction)
    }

    pub fn batch() -> Self {
        Self::new(BundleType::Batch)
    }

    pub fn message() -> Self {
        Self::new(BundleType::Message)
    }

    pub fn with_entry(mut self, entry: BundleEntry) -> Self {
        self.entry.get_or_insert_with(Vec::new).push(entry);
        self
    }

    pub fn entry_count(&self) -> usize {
        self.entry.as_ref().map(|e| e.len()).unwrap_or(0)
    }
}

/// Bundle 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BundleType {
    Document,
    Message,
    Transaction,
    #[serde(rename = "transaction-response")]
    TransactionResponse,
    Batch,
    #[serde(rename = "batch-response")]
    BatchResponse,
    History,
    Searchset,
    Collection,
    #[serde(rename = "subscription-notification")]
    SubscriptionNotification,
}

/// Bundle 链接
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleLink {
    /// 关系
    pub relation: String,
    /// URL
    pub url: String,
}

/// Bundle 条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleEntry {
    /// 链接
    pub link: Option<Vec<BundleLink>>,
    /// 完整 URL
    #[serde(rename = "fullUrl")]
    pub full_url: Option<String>,
    /// 资源
    pub resource: Option<serde_json::Value>,
    /// 搜索
    pub search: Option<BundleEntrySearch>,
    /// 请求
    pub request: Option<BundleEntryRequest>,
    /// 响应
    pub response: Option<BundleEntryResponse>,
}

impl BundleEntry {
    pub fn new(resource: serde_json::Value) -> Self {
        Self {
            link: None,
            full_url: None,
            resource: Some(resource),
            search: None,
            request: None,
            response: None,
        }
    }

    pub fn with_request(mut self, method: HttpMethod, url: &str) -> Self {
        self.request = Some(BundleEntryRequest {
            method,
            url: url.to_string(),
            if_none_match: None,
            if_modified_since: None,
            if_match: None,
            if_none_exist: None,
        });
        self
    }
}

/// Bundle 条目搜索
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleEntrySearch {
    /// 模式
    pub mode: Option<String>,
    /// 分数
    pub score: Option<f64>,
}

/// Bundle 条目请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleEntryRequest {
    /// 方法
    pub method: HttpMethod,
    /// URL
    pub url: String,
    /// If-None-Match
    #[serde(rename = "ifNoneMatch")]
    pub if_none_match: Option<String>,
    /// If-Modified-Since
    #[serde(rename = "ifModifiedSince")]
    pub if_modified_since: Option<String>,
    /// If-Match
    #[serde(rename = "ifMatch")]
    pub if_match: Option<String>,
    /// If-None-Exist
    #[serde(rename = "ifNoneExist")]
    pub if_none_exist: Option<String>,
}

/// HTTP 方法
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Head,
    Post,
    Put,
    Delete,
    Patch,
}

/// Bundle 条目响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleEntryResponse {
    /// 状态
    pub status: String,
    /// 位置
    pub location: Option<String>,
    /// ETag
    pub etag: Option<String>,
    /// 最后修改时间
    #[serde(rename = "lastModified")]
    pub last_modified: Option<String>,
    /// 结果
    pub outcome: Option<serde_json::Value>,
}

/// OperationOutcome（操作结果）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationOutcome {
    /// 资源类型
    #[serde(rename = "resourceType")]
    pub resource_type: String,
    /// ID
    pub id: Option<String>,
    /// 问题
    pub issue: Vec<OperationOutcomeIssue>,
}

impl OperationOutcome {
    pub fn success() -> Self {
        Self {
            resource_type: "OperationOutcome".to_string(),
            id: None,
            issue: vec![OperationOutcomeIssue {
                severity: IssueSeverity::Information,
                code: IssueType::Informational,
                details: None,
                diagnostics: Some("Operation completed successfully".to_string()),
                location: None,
                expression: None,
            }],
        }
    }

    pub fn error(message: &str) -> Self {
        Self {
            resource_type: "OperationOutcome".to_string(),
            id: None,
            issue: vec![OperationOutcomeIssue {
                severity: IssueSeverity::Error,
                code: IssueType::Processing,
                details: None,
                diagnostics: Some(message.to_string()),
                location: None,
                expression: None,
            }],
        }
    }
}

/// OperationOutcome 问题
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationOutcomeIssue {
    /// 严重性
    pub severity: IssueSeverity,
    /// 代码
    pub code: IssueType,
    /// 详情
    pub details: Option<serde_json::Value>,
    /// 诊断信息
    pub diagnostics: Option<String>,
    /// 位置
    pub location: Option<Vec<String>>,
    /// 表达式
    pub expression: Option<Vec<String>>,
}

/// 问题严重性
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueSeverity {
    Fatal,
    Error,
    Warning,
    Information,
}

/// 问题类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueType {
    Invalid,
    Structure,
    Required,
    Value,
    Invariant,
    Security,
    Login,
    Unknown,
    Expired,
    Forbidden,
    Suppressed,
    Processing,
    #[serde(rename = "not-supported")]
    NotSupported,
    Duplicate,
    #[serde(rename = "multiple-matches")]
    MultipleMatches,
    #[serde(rename = "not-found")]
    NotFound,
    #[serde(rename = "deleted")]
    Deleted,
    #[serde(rename = "too-long")]
    TooLong,
    #[serde(rename = "code-invalid")]
    CodeInvalid,
    Extension,
    #[serde(rename = "too-costly")]
    TooCostly,
    #[serde(rename = "business-rule")]
    BusinessRule,
    Conflict,
    Transient,
    #[serde(rename = "lock-error")]
    LockError,
    #[serde(rename = "no-store")]
    NoStore,
    Exception,
    Timeout,
    Incomplete,
    Throttled,
    Informational,
}
