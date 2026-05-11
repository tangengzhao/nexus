//! 组件注册表

use crate::Transformer;
use crate::adapter::ProtocolAdapter;
use hsb_common::{HsbError, HsbResult, ProtocolType};
use std::collections::HashMap;
use std::sync::Arc;

use super::MessageProcessor;

/// 适配器注册表
pub struct AdapterRegistry {
    adapters: HashMap<ProtocolType, Arc<dyn ProtocolAdapter>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    /// 注册适配器
    pub fn register(&mut self, adapter: Arc<dyn ProtocolAdapter>) {
        self.adapters.insert(adapter.protocol(), adapter);
    }

    /// 获取适配器
    pub fn get(&self, protocol: ProtocolType) -> Option<Arc<dyn ProtocolAdapter>> {
        self.adapters.get(&protocol).cloned()
    }

    /// 获取适配器（必须存在）
    pub fn get_required(&self, protocol: ProtocolType) -> HsbResult<Arc<dyn ProtocolAdapter>> {
        self.get(protocol).ok_or_else(|| HsbError::ConfigError {
            message: format!("Adapter not found for protocol: {:?}", protocol),
        })
    }

    /// 列出所有支持的协议
    pub fn supported_protocols(&self) -> Vec<ProtocolType> {
        self.adapters.keys().cloned().collect()
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 转换器注册表
pub struct TransformerRegistry {
    transformers: HashMap<String, Arc<dyn Transformer>>,
}

impl TransformerRegistry {
    pub fn new() -> Self {
        Self {
            transformers: HashMap::new(),
        }
    }

    /// 注册转换器
    pub fn register(&mut self, name: &str, transformer: Arc<dyn Transformer>) {
        self.transformers.insert(name.to_string(), transformer);
    }

    /// 获取转换器
    pub fn get(&self, name: &str) -> Option<Arc<dyn Transformer>> {
        self.transformers.get(name).cloned()
    }

    /// 获取转换器（必须存在）
    pub fn get_required(&self, name: &str) -> HsbResult<Arc<dyn Transformer>> {
        self.get(name).ok_or_else(|| HsbError::ConfigError {
            message: format!("Transformer not found: {}", name),
        })
    }

    /// 列出所有转换器
    pub fn list(&self) -> Vec<String> {
        self.transformers.keys().cloned().collect()
    }
}

impl Default for TransformerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 处理器注册表
pub struct ProcessorRegistry {
    processors: HashMap<String, Arc<dyn MessageProcessor>>,
}

impl ProcessorRegistry {
    pub fn new() -> Self {
        Self {
            processors: HashMap::new(),
        }
    }

    /// 注册处理器
    pub fn register(&mut self, processor: Arc<dyn MessageProcessor>) {
        self.processors
            .insert(processor.name().to_string(), processor);
    }

    /// 获取处理器
    pub fn get(&self, name: &str) -> Option<Arc<dyn MessageProcessor>> {
        self.processors.get(name).cloned()
    }

    /// 列出所有处理器
    pub fn list(&self) -> Vec<String> {
        self.processors.keys().cloned().collect()
    }

    /// 获取所有处理器（按优先级排序）
    pub fn all_sorted(&self) -> Vec<Arc<dyn MessageProcessor>> {
        let mut processors: Vec<_> = self.processors.values().cloned().collect();
        processors.sort_by_key(|p| p.priority());
        processors
    }
}

impl Default for ProcessorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 端点注册表
pub struct EndpointRegistry {
    endpoints: HashMap<String, EndpointInfo>,
}

impl EndpointRegistry {
    pub fn new() -> Self {
        Self {
            endpoints: HashMap::new(),
        }
    }

    /// 注册端点
    pub fn register(&mut self, info: EndpointInfo) {
        self.endpoints.insert(info.id.clone(), info);
    }

    /// 获取端点
    pub fn get(&self, id: &str) -> Option<&EndpointInfo> {
        self.endpoints.get(id)
    }

    /// 移除端点
    pub fn remove(&mut self, id: &str) -> Option<EndpointInfo> {
        self.endpoints.remove(id)
    }

    /// 列出所有端点
    pub fn list(&self) -> Vec<&EndpointInfo> {
        self.endpoints.values().collect()
    }

    /// 按协议过滤端点
    pub fn by_protocol(&self, protocol: ProtocolType) -> Vec<&EndpointInfo> {
        self.endpoints
            .values()
            .filter(|e| e.protocol == protocol)
            .collect()
    }
}

impl Default for EndpointRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 端点信息
#[derive(Debug, Clone)]
pub struct EndpointInfo {
    /// 端点 ID
    pub id: String,
    /// 端点名称
    pub name: String,
    /// 协议类型
    pub protocol: ProtocolType,
    /// 地址
    pub address: String,
    /// 是否启用
    pub enabled: bool,
    /// 健康状态
    pub healthy: bool,
    /// 最后心跳时间
    pub last_heartbeat: Option<chrono::DateTime<chrono::Utc>>,
}

impl EndpointInfo {
    pub fn new(id: &str, name: &str, protocol: ProtocolType, address: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            protocol,
            address: address.to_string(),
            enabled: true,
            healthy: true,
            last_heartbeat: None,
        }
    }
}
