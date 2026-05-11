//! DICOM 服务定义

use serde::{Deserialize, Serialize};

/// Modality Worklist 查询
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModalityWorklistQuery {
    /// 患者 ID
    pub patient_id: Option<String>,
    /// 患者姓名
    pub patient_name: Option<String>,
    /// 检查日期（起始）
    pub scheduled_date_from: Option<String>,
    /// 检查日期（结束）
    pub scheduled_date_to: Option<String>,
    /// 设备 AE Title
    pub station_ae_title: Option<String>,
    /// 模态
    pub modality: Option<String>,
    /// Accession Number
    pub accession_number: Option<String>,
}

/// Modality Worklist 项
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModalityWorklistItem {
    /// 患者 ID
    pub patient_id: String,
    /// 患者姓名
    pub patient_name: String,
    /// 患者性别
    pub patient_sex: Option<String>,
    /// 出生日期
    pub patient_birth_date: Option<String>,
    /// Accession Number
    pub accession_number: String,
    /// Study Instance UID
    pub study_instance_uid: String,
    /// 请求的程序描述
    pub requested_procedure_description: Option<String>,
    /// 计划程序步骤 ID
    pub scheduled_procedure_step_id: Option<String>,
    /// 计划开始日期
    pub scheduled_start_date: Option<String>,
    /// 计划开始时间
    pub scheduled_start_time: Option<String>,
    /// 模态
    pub modality: String,
    /// 设备 AE Title
    pub scheduled_station_ae_title: Option<String>,
}

/// MPPS 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MppsStatus {
    /// 进行中
    InProgress,
    /// 已完成
    Completed,
    /// 已中断
    Discontinued,
}

impl MppsStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InProgress => "IN PROGRESS",
            Self::Completed => "COMPLETED",
            Self::Discontinued => "DISCONTINUED",
        }
    }
}

/// MPPS 创建请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MppsCreate {
    /// SOP Instance UID
    pub sop_instance_uid: String,
    /// 患者 ID
    pub patient_id: String,
    /// 患者姓名
    pub patient_name: String,
    /// Study Instance UID
    pub study_instance_uid: String,
    /// 程序步骤开始日期
    pub procedure_step_start_date: String,
    /// 程序步骤开始时间
    pub procedure_step_start_time: String,
    /// 模态
    pub modality: String,
    /// 设备 AE Title
    pub station_ae_title: String,
}

/// MPPS 完成请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MppsComplete {
    /// SOP Instance UID
    pub sop_instance_uid: String,
    /// 程序步骤结束日期
    pub procedure_step_end_date: String,
    /// 程序步骤结束时间
    pub procedure_step_end_time: String,
    /// 状态
    pub status: MppsStatus,
    /// Series 列表
    pub series: Vec<PerformedSeries>,
}

/// 执行的 Series
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformedSeries {
    /// Series Instance UID
    pub series_instance_uid: String,
    /// 协议名称
    pub protocol_name: Option<String>,
    /// 操作者姓名
    pub operator_name: Option<String>,
    /// 图像数量
    pub number_of_images: u32,
}

/// Storage Commitment 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageCommitmentRequest {
    /// Transaction UID
    pub transaction_uid: String,
    /// 引用的 SOP 序列
    pub referenced_sop_sequence: Vec<ReferencedSop>,
}

/// 引用的 SOP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferencedSop {
    /// SOP Class UID
    pub sop_class_uid: String,
    /// SOP Instance UID
    pub sop_instance_uid: String,
}

/// Storage Commitment 结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageCommitmentResult {
    /// Transaction UID
    pub transaction_uid: String,
    /// 成功的 SOP 序列
    pub success_sop_sequence: Vec<ReferencedSop>,
    /// 失败的 SOP 序列
    pub failed_sop_sequence: Vec<FailedSop>,
}

/// 失败的 SOP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedSop {
    /// SOP Class UID
    pub sop_class_uid: String,
    /// SOP Instance UID
    pub sop_instance_uid: String,
    /// 失败原因
    pub failure_reason: u16,
}
