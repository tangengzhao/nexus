//! 机构/系统目录模型

use chrono::{DateTime, Utc};
use hsb_common::{MedicalSystemType, OrganizationId, OrganizationType, SystemId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 机构对象
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub id: OrganizationId,
    pub name: String,
    pub description: Option<String>,
    pub organization_type: OrganizationType,
    pub parent_organization_id: Option<OrganizationId>,
    pub enabled: bool,
    pub properties: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Organization {
    pub fn new(
        id: impl Into<OrganizationId>,
        name: impl Into<String>,
        organization_type: OrganizationType,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            organization_type,
            parent_organization_id: None,
            enabled: true,
            properties: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }
}

/// 集成系统对象
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationSystem {
    pub id: SystemId,
    pub organization_id: OrganizationId,
    pub name: String,
    pub description: Option<String>,
    pub system_type: MedicalSystemType,
    pub topic_namespace: Option<String>,
    pub topic_prefix: Option<String>,
    pub enabled: bool,
    pub properties: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl IntegrationSystem {
    pub fn new(
        id: impl Into<SystemId>,
        organization_id: impl Into<OrganizationId>,
        name: impl Into<String>,
        system_type: MedicalSystemType,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            organization_id: organization_id.into(),
            name: name.into(),
            description: None,
            system_type,
            topic_namespace: None,
            topic_prefix: None,
            enabled: true,
            properties: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }
}
