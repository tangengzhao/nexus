//! HL7 类型定义

use serde::{Deserialize, Serialize};

/// HL7 编码字符
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodingCharacters {
    /// 组件分隔符（默认 ^）
    pub component: char,
    /// 重复分隔符（默认 ~）
    pub repetition: char,
    /// 转义字符（默认 \）
    pub escape: char,
    /// 子组件分隔符（默认 &）
    pub subcomponent: char,
}

impl Default for EncodingCharacters {
    fn default() -> Self {
        Self {
            component: '^',
            repetition: '~',
            escape: '\\',
            subcomponent: '&',
        }
    }
}

impl EncodingCharacters {
    pub fn from_string(s: &str) -> Option<Self> {
        if s.len() < 4 {
            return None;
        }
        let chars: Vec<char> = s.chars().collect();
        Some(Self {
            component: chars[0],
            repetition: chars[1],
            escape: chars[2],
            subcomponent: chars[3],
        })
    }

    pub fn to_string(&self) -> String {
        format!(
            "{}{}{}{}",
            self.component, self.repetition, self.escape, self.subcomponent
        )
    }
}

/// 编码标识符（CX 类型）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtendedCompositeId {
    /// ID
    pub id: String,
    /// 检查位
    pub check_digit: Option<String>,
    /// 检查位方案
    pub check_digit_scheme: Option<String>,
    /// 分配机构
    pub assigning_authority: Option<HierarchicDesignator>,
    /// 标识符类型代码
    pub identifier_type_code: Option<String>,
    /// 分配设施
    pub assigning_facility: Option<HierarchicDesignator>,
}

/// 层级指示器（HD 类型）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HierarchicDesignator {
    /// 命名空间 ID
    pub namespace_id: Option<String>,
    /// 通用 ID
    pub universal_id: Option<String>,
    /// 通用 ID 类型
    pub universal_id_type: Option<String>,
}

/// 编码元素（CE 类型）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CodedElement {
    /// 标识符
    pub identifier: String,
    /// 文本
    pub text: Option<String>,
    /// 编码系统名称
    pub name_of_coding_system: Option<String>,
    /// 替代标识符
    pub alternate_identifier: Option<String>,
    /// 替代文本
    pub alternate_text: Option<String>,
    /// 替代编码系统名称
    pub name_of_alternate_coding_system: Option<String>,
}

impl CodedElement {
    pub fn new(identifier: &str, text: &str, coding_system: &str) -> Self {
        Self {
            identifier: identifier.to_string(),
            text: Some(text.to_string()),
            name_of_coding_system: Some(coding_system.to_string()),
            ..Default::default()
        }
    }

    pub fn from_hl7(s: &str) -> Self {
        let parts: Vec<&str> = s.split('^').collect();
        Self {
            identifier: parts.first().unwrap_or(&"").to_string(),
            text: parts.get(1).map(|s| s.to_string()),
            name_of_coding_system: parts.get(2).map(|s| s.to_string()),
            alternate_identifier: parts.get(3).map(|s| s.to_string()),
            alternate_text: parts.get(4).map(|s| s.to_string()),
            name_of_alternate_coding_system: parts.get(5).map(|s| s.to_string()),
        }
    }
}

/// 时间戳（TS 类型）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Timestamp {
    /// 时间（格式：YYYYMMDDHHMMSS.SSSS±HHMM）
    pub time: String,
    /// 精度
    pub degree_of_precision: Option<String>,
}

impl Timestamp {
    pub fn now() -> Self {
        let now = chrono::Utc::now();
        Self {
            time: now.format("%Y%m%d%H%M%S").to_string(),
            degree_of_precision: None,
        }
    }

    pub fn from_hl7(s: &str) -> Self {
        Self {
            time: s.to_string(),
            degree_of_precision: None,
        }
    }
}

/// 扩展地址（XAD 类型）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtendedAddress {
    /// 街道地址
    pub street_address: Option<String>,
    /// 其他地址
    pub other_designation: Option<String>,
    /// 城市
    pub city: Option<String>,
    /// 州/省
    pub state_or_province: Option<String>,
    /// 邮政编码
    pub zip_or_postal_code: Option<String>,
    /// 国家
    pub country: Option<String>,
    /// 地址类型
    pub address_type: Option<String>,
}

/// 扩展电话号码（XTN 类型）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtendedTelecommunication {
    /// 电话号码
    pub telephone_number: Option<String>,
    /// 电信使用代码
    pub telecommunication_use_code: Option<String>,
    /// 电信设备类型
    pub telecommunication_equipment_type: Option<String>,
    /// 电子邮件地址
    pub email_address: Option<String>,
    /// 国家代码
    pub country_code: Option<String>,
    /// 区号
    pub area_code: Option<String>,
    /// 本地号码
    pub local_number: Option<String>,
    /// 分机号
    pub extension: Option<String>,
}

/// 扩展人员名称（XPN 类型）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtendedPersonName {
    /// 姓
    pub family_name: String,
    /// 名
    pub given_name: Option<String>,
    /// 中间名或首字母
    pub second_name: Option<String>,
    /// 后缀
    pub suffix: Option<String>,
    /// 前缀
    pub prefix: Option<String>,
    /// 学位
    pub degree: Option<String>,
    /// 名称类型代码
    pub name_type_code: Option<String>,
}

/// 扩展组织标识符（XON 类型）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtendedOrganizationId {
    /// 组织名称
    pub organization_name: Option<String>,
    /// 组织名称类型代码
    pub organization_name_type_code: Option<String>,
    /// ID 号
    pub id_number: Option<String>,
    /// 检查位
    pub check_digit: Option<String>,
    /// 分配机构
    pub assigning_authority: Option<HierarchicDesignator>,
}
