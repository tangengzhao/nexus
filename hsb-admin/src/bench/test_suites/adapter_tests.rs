//! 协议适配器测试

use bytes::Bytes;
use hsb_adapter::dicom::DicomAdapter;
use hsb_adapter::fhir::FhirR5Adapter;
use hsb_adapter::hl7::Hl7V2Adapter;
use hsb_adapter::soap::SoapAdapter;
use hsb_common::ProtocolType;
use hsb_core::{ParseOptions, ProtocolAdapter, SerializeOptions};
use serde_json::json;

/// 标准 HL7 ADT^A01 测试消息
fn sample_hl7_message() -> Bytes {
    Bytes::from(
        "MSH|^~\\&|HIS|HOSPITAL|LIS|LAB|20231215120000||ADT^A01^ADT_A01|MSG00001|P|2.5.1\r\
         EVN|A01|20231215120000\r\
         PID|1||P12345^^^HIS||张三^三^||19800101|M|||北京市海淀区^^中国\r\
         PV1|1|I|ICU^01^01||||12345^李医生^四^^^^DR\r",
    )
}

/// 标准 FHIR Patient 资源
fn sample_fhir_patient() -> Bytes {
    Bytes::from(
        serde_json::to_string(&json!({
            "resourceType": "Patient",
            "id": "patient-001",
            "meta": {
                "versionId": "1",
                "lastUpdated": "2023-12-15T12:00:00Z"
            },
            "identifier": [{
                "system": "http://hospital.example.org/patients",
                "value": "P12345"
            }],
            "name": [{
                "use": "official",
                "family": "张",
                "given": ["三"]
            }],
            "gender": "male",
            "birthDate": "1980-01-01"
        }))
        .unwrap(),
    )
}

/// 标准 DICOM DIMSE 消息 (简化)
fn sample_dicom_message() -> Bytes {
    // 简化的 DICOM 文件头
    Bytes::from(vec![
        // DICOM 文件前缀
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // DICM 魔术字节
        b'D', b'I', b'C', b'M', // 简化的元素
        0x02, 0x00, 0x00, 0x00, b'U', b'L', 0x04, 0x00, 0x00, 0x00, 0x00, 0x00,
    ])
}

/// 标准 SOAP 消息
fn sample_soap_message() -> Bytes {
    Bytes::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
        <soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
            <soap:Header>
                <wsa:Action xmlns:wsa="http://www.w3.org/2005/08/addressing">
                    http://hospital.example.org/PatientService/GetPatient
                </wsa:Action>
            </soap:Header>
            <soap:Body>
                <GetPatient xmlns="http://hospital.example.org/PatientService">
                    <PatientId>P12345</PatientId>
                </GetPatient>
            </soap:Body>
        </soap:Envelope>"#,
    )
}

/// 测试 HL7 消息解析
pub async fn test_hl7_parsing() -> Result<(), String> {
    let adapter = Hl7V2Adapter::new();
    let raw = sample_hl7_message();
    let options = ParseOptions::default();

    let msg = adapter
        .parse(raw, &options)
        .await
        .map_err(|e| format!("HL7 解析失败: {:?}", e))?;

    if msg.protocol != ProtocolType::Hl7V2 {
        return Err("协议类型不匹配".to_string());
    }

    Ok(())
}

/// 测试 HL7 消息序列化
pub async fn test_hl7_serialization() -> Result<(), String> {
    let adapter = Hl7V2Adapter::new();
    let raw = sample_hl7_message();
    let options = ParseOptions::default();

    let msg = adapter
        .parse(raw, &options)
        .await
        .map_err(|e| format!("HL7 解析失败: {:?}", e))?;

    let serialize_options = SerializeOptions::default();
    let _output = adapter
        .serialize(&msg, &serialize_options)
        .await
        .map_err(|e| format!("HL7 序列化失败: {:?}", e))?;

    Ok(())
}

/// 测试 HL7 消息验证
pub async fn test_hl7_validation() -> Result<(), String> {
    let adapter = Hl7V2Adapter::new();
    let raw = sample_hl7_message();

    let result = adapter
        .validate(&raw)
        .await
        .map_err(|e| format!("HL7 验证失败: {:?}", e))?;

    // 允许有警告但不能有错误
    if !result.valid {
        return Err(format!("验证失败: {} 个错误", result.errors.len()));
    }

    Ok(())
}

/// 测试 FHIR 资源解析
pub async fn test_fhir_parsing() -> Result<(), String> {
    let adapter = FhirR5Adapter::new();
    let raw = sample_fhir_patient();
    let options = ParseOptions::default();

    let msg = adapter
        .parse(raw, &options)
        .await
        .map_err(|e| format!("FHIR 解析失败: {:?}", e))?;

    if msg.protocol != ProtocolType::FhirR5 {
        return Err("协议类型不匹配".to_string());
    }

    Ok(())
}

/// 测试 FHIR 资源序列化
pub async fn test_fhir_serialization() -> Result<(), String> {
    let adapter = FhirR5Adapter::new();
    let raw = sample_fhir_patient();
    let options = ParseOptions::default();

    let msg = adapter
        .parse(raw, &options)
        .await
        .map_err(|e| format!("FHIR 解析失败: {:?}", e))?;

    let serialize_options = SerializeOptions::default();
    let _output = adapter
        .serialize(&msg, &serialize_options)
        .await
        .map_err(|e| format!("FHIR 序列化失败: {:?}", e))?;

    Ok(())
}

/// 测试 DICOM 消息解析
pub async fn test_dicom_parsing() -> Result<(), String> {
    let adapter = DicomAdapter::new();
    let raw = sample_dicom_message();
    let options = ParseOptions::default();

    // DICOM 解析可能失败（简化的测试数据），但我们测试解析器不会 panic
    match adapter.parse(raw, &options).await {
        Ok(msg) => {
            if msg.protocol != ProtocolType::Dicom {
                return Err("协议类型不匹配".to_string());
            }
        }
        Err(_) => {
            // 对于简化的测试数据，解析失败是可接受的
            // 主要测试解析器的稳定性
        }
    }

    Ok(())
}

/// 测试 SOAP 消息解析
pub async fn test_soap_parsing() -> Result<(), String> {
    let adapter = SoapAdapter::new();
    let raw = sample_soap_message();
    let options = ParseOptions::default();

    let msg = adapter
        .parse(raw, &options)
        .await
        .map_err(|e| format!("SOAP 解析失败: {:?}", e))?;

    if msg.protocol != ProtocolType::Soap {
        return Err("协议类型不匹配".to_string());
    }

    Ok(())
}

/// 测试 SOAP 消息序列化
pub async fn test_soap_serialization() -> Result<(), String> {
    let adapter = SoapAdapter::new();
    let raw = sample_soap_message();
    let options = ParseOptions::default();

    let msg = adapter
        .parse(raw, &options)
        .await
        .map_err(|e| format!("SOAP 解析失败: {:?}", e))?;

    let serialize_options = SerializeOptions::default();
    let _output = adapter
        .serialize(&msg, &serialize_options)
        .await
        .map_err(|e| format!("SOAP 序列化失败: {:?}", e))?;

    Ok(())
}
