//! HSB 协议适配器
//!
//! 所有医疗协议适配器的统一入口。基础 trait 定义在 `hsb-core::adapter` 中，
//! 本 crate 提供 HL7 v2.x、HL7 v3、FHIR R5、DICOM、SOAP 五种协议的具体实现。

pub mod dicom;
pub mod fhir;
pub mod hl7;
pub mod hl7v3;
pub mod soap;

// Re-export commonly used types from implementations
pub use dicom::DicomAdapter;
pub use fhir::FhirR5Adapter;
pub use hl7::Hl7V2Adapter;
pub use hl7v3::Hl7V3Adapter;
pub use soap::SoapAdapter;
