//! HL7 段定义

use serde::{Deserialize, Serialize};

/// MSH 段（消息头）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MshSegment {
    /// MSH-1: 字段分隔符
    pub field_separator: char,
    /// MSH-2: 编码字符
    pub encoding_characters: String,
    /// MSH-3: 发送应用
    pub sending_application: String,
    /// MSH-4: 发送设施
    pub sending_facility: String,
    /// MSH-5: 接收应用
    pub receiving_application: String,
    /// MSH-6: 接收设施
    pub receiving_facility: String,
    /// MSH-7: 消息日期时间
    pub message_datetime: String,
    /// MSH-8: 安全
    pub security: Option<String>,
    /// MSH-9: 消息类型
    pub message_type: MessageType,
    /// MSH-10: 消息控制 ID
    pub message_control_id: String,
    /// MSH-11: 处理 ID
    pub processing_id: String,
    /// MSH-12: 版本 ID
    pub version_id: String,
}

/// 消息类型（MSH-9）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageType {
    /// 消息类型（如 ADT）
    pub message_type: String,
    /// 触发事件（如 A01）
    pub trigger_event: String,
    /// 消息结构（如 ADT_A01）
    pub message_structure: Option<String>,
}

impl MessageType {
    pub fn new(message_type: &str, trigger_event: &str) -> Self {
        Self {
            message_type: message_type.to_string(),
            trigger_event: trigger_event.to_string(),
            message_structure: None,
        }
    }

    pub fn to_string(&self) -> String {
        match &self.message_structure {
            Some(structure) => {
                format!("{}^{}^{}", self.message_type, self.trigger_event, structure)
            }
            None => format!("{}^{}", self.message_type, self.trigger_event),
        }
    }
}

/// PID 段（患者标识）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PidSegment {
    /// PID-1: 集合 ID
    pub set_id: Option<String>,
    /// PID-2: 患者 ID（外部）
    pub patient_id_external: Option<String>,
    /// PID-3: 患者标识符列表
    pub patient_identifier_list: Vec<PatientIdentifier>,
    /// PID-4: 替代患者 ID
    pub alternate_patient_id: Option<String>,
    /// PID-5: 患者姓名
    pub patient_name: PersonName,
    /// PID-6: 母亲婚前姓名
    pub mother_maiden_name: Option<PersonName>,
    /// PID-7: 出生日期
    pub date_of_birth: Option<String>,
    /// PID-8: 性别
    pub administrative_sex: Option<String>,
    /// PID-9: 患者别名
    pub patient_alias: Option<PersonName>,
    /// PID-10: 种族
    pub race: Option<String>,
    /// PID-11: 患者地址
    pub patient_address: Option<Address>,
}

/// 患者标识符
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PatientIdentifier {
    /// 患者 ID
    pub id: String,
    /// 检查位
    pub check_digit: Option<String>,
    /// 检查位方案
    pub check_digit_scheme: Option<String>,
    /// 分配机构
    pub assigning_authority: Option<String>,
    /// 标识符类型代码
    pub identifier_type_code: Option<String>,
}

/// 人员姓名
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersonName {
    /// 姓
    pub family_name: String,
    /// 名
    pub given_name: Option<String>,
    /// 中间名
    pub middle_name: Option<String>,
    /// 后缀
    pub suffix: Option<String>,
    /// 前缀
    pub prefix: Option<String>,
    /// 学位
    pub degree: Option<String>,
}

impl PersonName {
    pub fn from_hl7(s: &str) -> Self {
        let parts: Vec<&str> = s.split('^').collect();
        Self {
            family_name: parts.first().unwrap_or(&"").to_string(),
            given_name: parts.get(1).map(|s| s.to_string()),
            middle_name: parts.get(2).map(|s| s.to_string()),
            suffix: parts.get(3).map(|s| s.to_string()),
            prefix: parts.get(4).map(|s| s.to_string()),
            degree: parts.get(5).map(|s| s.to_string()),
        }
    }

    pub fn to_hl7(&self) -> String {
        format!(
            "{}^{}^{}^{}^{}^{}",
            self.family_name,
            self.given_name.as_deref().unwrap_or(""),
            self.middle_name.as_deref().unwrap_or(""),
            self.suffix.as_deref().unwrap_or(""),
            self.prefix.as_deref().unwrap_or(""),
            self.degree.as_deref().unwrap_or("")
        )
        .trim_end_matches('^')
        .to_string()
    }
}

/// 地址
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Address {
    /// 街道地址
    pub street_address: Option<String>,
    /// 其他地址
    pub other_designation: Option<String>,
    /// 城市
    pub city: Option<String>,
    /// 州/省
    pub state: Option<String>,
    /// 邮政编码
    pub postal_code: Option<String>,
    /// 国家
    pub country: Option<String>,
    /// 地址类型
    pub address_type: Option<String>,
}

/// PV1 段（就诊信息）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Pv1Segment {
    /// PV1-1: 集合 ID
    pub set_id: Option<String>,
    /// PV1-2: 患者类别
    pub patient_class: String,
    /// PV1-3: 分配的患者位置
    pub assigned_patient_location: Option<PatientLocation>,
    /// PV1-4: 入院类型
    pub admission_type: Option<String>,
    /// PV1-7: 主治医生
    pub attending_doctor: Option<String>,
    /// PV1-19: 就诊号
    pub visit_number: Option<String>,
    /// PV1-44: 入院日期时间
    pub admit_datetime: Option<String>,
}

/// 患者位置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PatientLocation {
    /// 护理单元
    pub point_of_care: Option<String>,
    /// 房间
    pub room: Option<String>,
    /// 床位
    pub bed: Option<String>,
    /// 设施
    pub facility: Option<String>,
    /// 楼层
    pub floor: Option<String>,
    /// 建筑
    pub building: Option<String>,
}

/// ORC 段（通用医嘱）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OrcSegment {
    /// ORC-1: 医嘱控制
    pub order_control: String,
    /// ORC-2: 申请方医嘱号
    pub placer_order_number: Option<String>,
    /// ORC-3: 执行方医嘱号
    pub filler_order_number: Option<String>,
    /// ORC-5: 医嘱状态
    pub order_status: Option<String>,
    /// ORC-9: 事务日期时间
    pub datetime_of_transaction: Option<String>,
    /// ORC-12: 下医嘱医生
    pub ordering_provider: Option<String>,
}

/// OBR 段（观察请求）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObrSegment {
    /// OBR-1: 集合 ID
    pub set_id: Option<String>,
    /// OBR-2: 申请方医嘱号
    pub placer_order_number: Option<String>,
    /// OBR-3: 执行方医嘱号
    pub filler_order_number: Option<String>,
    /// OBR-4: 通用服务标识符
    pub universal_service_identifier: Option<String>,
    /// OBR-7: 观察日期时间
    pub observation_datetime: Option<String>,
    /// OBR-22: 结果状态
    pub result_status: Option<String>,
}

/// OBX 段（观察结果）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObxSegment {
    /// OBX-1: 集合 ID
    pub set_id: Option<String>,
    /// OBX-2: 值类型
    pub value_type: String,
    /// OBX-3: 观察标识符
    pub observation_identifier: String,
    /// OBX-4: 观察子 ID
    pub observation_sub_id: Option<String>,
    /// OBX-5: 观察值
    pub observation_value: String,
    /// OBX-6: 单位
    pub units: Option<String>,
    /// OBX-7: 参考范围
    pub reference_range: Option<String>,
    /// OBX-8: 异常标志
    pub abnormal_flags: Option<String>,
    /// OBX-11: 观察结果状态
    pub observation_result_status: String,
}
