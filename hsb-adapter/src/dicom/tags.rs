//! DICOM 标签定义

/// 常用 DICOM 标签
pub mod tag {
    /// Patient ID (0010,0020)
    pub const PATIENT_ID: (u16, u16) = (0x0010, 0x0020);
    /// Patient Name (0010,0010)
    pub const PATIENT_NAME: (u16, u16) = (0x0010, 0x0010);
    /// Patient Birth Date (0010,0030)
    pub const PATIENT_BIRTH_DATE: (u16, u16) = (0x0010, 0x0030);
    /// Patient Sex (0010,0040)
    pub const PATIENT_SEX: (u16, u16) = (0x0010, 0x0040);

    /// Study Instance UID (0020,000D)
    pub const STUDY_INSTANCE_UID: (u16, u16) = (0x0020, 0x000D);
    /// Series Instance UID (0020,000E)
    pub const SERIES_INSTANCE_UID: (u16, u16) = (0x0020, 0x000E);
    /// SOP Instance UID (0008,0018)
    pub const SOP_INSTANCE_UID: (u16, u16) = (0x0008, 0x0018);
    /// SOP Class UID (0008,0016)
    pub const SOP_CLASS_UID: (u16, u16) = (0x0008, 0x0016);

    /// Modality (0008,0060)
    pub const MODALITY: (u16, u16) = (0x0008, 0x0060);
    /// Study Date (0008,0020)
    pub const STUDY_DATE: (u16, u16) = (0x0008, 0x0020);
    /// Study Time (0008,0030)
    pub const STUDY_TIME: (u16, u16) = (0x0008, 0x0030);
    /// Accession Number (0008,0050)
    pub const ACCESSION_NUMBER: (u16, u16) = (0x0008, 0x0050);
    /// Study Description (0008,1030)
    pub const STUDY_DESCRIPTION: (u16, u16) = (0x0008, 0x1030);

    /// Referring Physician Name (0008,0090)
    pub const REFERRING_PHYSICIAN_NAME: (u16, u16) = (0x0008, 0x0090);
    /// Performing Physician Name (0008,1050)
    pub const PERFORMING_PHYSICIAN_NAME: (u16, u16) = (0x0008, 0x1050);

    /// Series Number (0020,0011)
    pub const SERIES_NUMBER: (u16, u16) = (0x0020, 0x0011);
    /// Instance Number (0020,0013)
    pub const INSTANCE_NUMBER: (u16, u16) = (0x0020, 0x0013);

    /// Scheduled Procedure Step Start Date (0040,0002)
    pub const SCHEDULED_PROCEDURE_STEP_START_DATE: (u16, u16) = (0x0040, 0x0002);
    /// Scheduled Procedure Step Start Time (0040,0003)
    pub const SCHEDULED_PROCEDURE_STEP_START_TIME: (u16, u16) = (0x0040, 0x0003);
    /// Scheduled Station AE Title (0040,0001)
    pub const SCHEDULED_STATION_AE_TITLE: (u16, u16) = (0x0040, 0x0001);
}

/// 模态类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modality {
    /// Computed Tomography
    CT,
    /// Magnetic Resonance
    MR,
    /// Ultrasound
    US,
    /// X-Ray
    XA,
    /// Digital Radiography
    DX,
    /// Computed Radiography
    CR,
    /// Mammography
    MG,
    /// Nuclear Medicine
    NM,
    /// Positron Emission Tomography
    PT,
    /// Secondary Capture
    SC,
    /// Other
    OT,
}

impl Modality {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CT => "CT",
            Self::MR => "MR",
            Self::US => "US",
            Self::XA => "XA",
            Self::DX => "DX",
            Self::CR => "CR",
            Self::MG => "MG",
            Self::NM => "NM",
            Self::PT => "PT",
            Self::SC => "SC",
            Self::OT => "OT",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "CT" => Some(Self::CT),
            "MR" => Some(Self::MR),
            "US" => Some(Self::US),
            "XA" => Some(Self::XA),
            "DX" => Some(Self::DX),
            "CR" => Some(Self::CR),
            "MG" => Some(Self::MG),
            "NM" => Some(Self::NM),
            "PT" => Some(Self::PT),
            "SC" => Some(Self::SC),
            "OT" => Some(Self::OT),
            _ => None,
        }
    }
}
