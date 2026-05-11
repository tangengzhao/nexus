//! FHIR 资源类型定义

use serde::{Deserialize, Serialize};

/// FHIR 资源基类
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    /// 资源类型
    #[serde(rename = "resourceType")]
    pub resource_type: String,
    /// 资源 ID
    pub id: Option<String>,
    /// 元数据
    pub meta: Option<Meta>,
}

/// 元数据
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Meta {
    /// 版本 ID
    #[serde(rename = "versionId")]
    pub version_id: Option<String>,
    /// 最后更新时间
    #[serde(rename = "lastUpdated")]
    pub last_updated: Option<String>,
    /// 配置文件
    pub profile: Option<Vec<String>>,
    /// 安全标签
    pub security: Option<Vec<Coding>>,
    /// 标签
    pub tag: Option<Vec<Coding>>,
}

/// 编码
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Coding {
    /// 系统
    pub system: Option<String>,
    /// 版本
    pub version: Option<String>,
    /// 代码
    pub code: Option<String>,
    /// 显示
    pub display: Option<String>,
    /// 用户选择
    #[serde(rename = "userSelected")]
    pub user_selected: Option<bool>,
}

/// 可编码概念
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CodeableConcept {
    /// 编码
    pub coding: Option<Vec<Coding>>,
    /// 文本
    pub text: Option<String>,
}

/// 引用
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Reference {
    /// 引用
    pub reference: Option<String>,
    /// 类型
    #[serde(rename = "type")]
    pub reference_type: Option<String>,
    /// 标识符
    pub identifier: Option<Identifier>,
    /// 显示
    pub display: Option<String>,
}

impl Reference {
    pub fn new(reference: &str) -> Self {
        Self {
            reference: Some(reference.to_string()),
            ..Default::default()
        }
    }

    pub fn patient(id: &str) -> Self {
        Self::new(&format!("Patient/{}", id))
    }

    pub fn encounter(id: &str) -> Self {
        Self::new(&format!("Encounter/{}", id))
    }
}

/// 标识符
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Identifier {
    /// 使用
    #[serde(rename = "use")]
    pub use_: Option<String>,
    /// 类型
    #[serde(rename = "type")]
    pub type_: Option<CodeableConcept>,
    /// 系统
    pub system: Option<String>,
    /// 值
    pub value: Option<String>,
    /// 周期
    pub period: Option<Period>,
    /// 分配者
    pub assigner: Option<Box<Reference>>,
}

/// 周期
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Period {
    /// 开始
    pub start: Option<String>,
    /// 结束
    pub end: Option<String>,
}

/// 人名
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HumanName {
    /// 使用
    #[serde(rename = "use")]
    pub use_: Option<String>,
    /// 文本
    pub text: Option<String>,
    /// 姓
    pub family: Option<String>,
    /// 名
    pub given: Option<Vec<String>>,
    /// 前缀
    pub prefix: Option<Vec<String>>,
    /// 后缀
    pub suffix: Option<Vec<String>>,
    /// 周期
    pub period: Option<Period>,
}

/// 地址
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Address {
    /// 使用
    #[serde(rename = "use")]
    pub use_: Option<String>,
    /// 类型
    #[serde(rename = "type")]
    pub type_: Option<String>,
    /// 文本
    pub text: Option<String>,
    /// 行
    pub line: Option<Vec<String>>,
    /// 城市
    pub city: Option<String>,
    /// 区
    pub district: Option<String>,
    /// 州
    pub state: Option<String>,
    /// 邮政编码
    #[serde(rename = "postalCode")]
    pub postal_code: Option<String>,
    /// 国家
    pub country: Option<String>,
    /// 周期
    pub period: Option<Period>,
}

/// 联系点
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContactPoint {
    /// 系统
    pub system: Option<String>,
    /// 值
    pub value: Option<String>,
    /// 使用
    #[serde(rename = "use")]
    pub use_: Option<String>,
    /// 排序
    pub rank: Option<u32>,
    /// 周期
    pub period: Option<Period>,
}

/// 附件
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Attachment {
    /// 内容类型
    #[serde(rename = "contentType")]
    pub content_type: Option<String>,
    /// 语言
    pub language: Option<String>,
    /// 数据
    pub data: Option<String>,
    /// URL
    pub url: Option<String>,
    /// 大小
    pub size: Option<u64>,
    /// 哈希
    pub hash: Option<String>,
    /// 标题
    pub title: Option<String>,
    /// 创建时间
    pub creation: Option<String>,
}

/// 数量
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Quantity {
    /// 值
    pub value: Option<f64>,
    /// 比较器
    pub comparator: Option<String>,
    /// 单位
    pub unit: Option<String>,
    /// 系统
    pub system: Option<String>,
    /// 代码
    pub code: Option<String>,
}

/// 范围
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Range {
    /// 低值
    pub low: Option<Quantity>,
    /// 高值
    pub high: Option<Quantity>,
}

/// 比率
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Ratio {
    /// 分子
    pub numerator: Option<Quantity>,
    /// 分母
    pub denominator: Option<Quantity>,
}

/// 注解
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Annotation {
    /// 作者引用
    #[serde(rename = "authorReference")]
    pub author_reference: Option<Reference>,
    /// 作者字符串
    #[serde(rename = "authorString")]
    pub author_string: Option<String>,
    /// 时间
    pub time: Option<String>,
    /// 文本
    pub text: Option<String>,
}
