//! 传输层测试

use bytes::Bytes;
use hsb_core::{
    ConnectionPoolConfig, TransportRegistry, TransportRequest, TransportResponse,
    TransportRetryConfig,
};
use std::time::Duration;

fn test_http_target() -> String {
    std::env::var("HSB_TEST_HTTP_MESSAGES_TARGET")
        .unwrap_or_else(|_| "http://gateway.internal:8080/api/messages".to_string())
}

/// 测试传输配置创建
pub fn test_transport_config() -> Result<(), String> {
    // 连接池配置
    let pool_config = ConnectionPoolConfig::default();

    if pool_config.max_connections != 10 {
        return Err("默认最大连接数应为10".to_string());
    }

    if pool_config.min_connections != 1 {
        return Err("默认最小连接数应为1".to_string());
    }

    // 重试配置
    let retry_config = TransportRetryConfig::default();

    if retry_config.max_retries != 3 {
        return Err("默认最大重试次数应为3".to_string());
    }

    Ok(())
}

/// 测试传输注册表
pub fn test_transport_registry() -> Result<(), String> {
    let registry = TransportRegistry::new();

    // 验证初始状态
    if !registry.list().is_empty() {
        return Err("注册表应该为空".to_string());
    }

    // 注册表创建成功
    Ok(())
}

/// 测试请求/响应构建
pub fn test_request_response() -> Result<(), String> {
    // 创建请求
    let target = test_http_target();
    let request = TransportRequest::new(&target, Bytes::from(r#"{"test": "data"}"#))
        .with_header("Content-Type", "application/json")
        .with_header("X-Trace-Id", "trace-001")
        .with_timeout(Duration::from_secs(5));

    if request.target != target {
        return Err("请求目标不匹配".to_string());
    }

    if request.timeout != Some(Duration::from_secs(5)) {
        return Err("请求超时不匹配".to_string());
    }

    // 创建响应
    let response = TransportResponse::success(
        Bytes::from(r#"{"status": "ok"}"#),
        Duration::from_millis(150),
    );

    if response.status_code != 200 {
        return Err("响应状态码不匹配".to_string());
    }

    if !response.is_success() {
        return Err("响应应该标记为成功".to_string());
    }

    Ok(())
}

/// 测试连接池模拟
pub fn test_connection_pool() -> Result<(), String> {
    use std::collections::VecDeque;
    use std::sync::Mutex;

    // 模拟连接池
    struct MockConnectionPool {
        connections: Mutex<VecDeque<u32>>,
        max_size: usize,
    }

    impl MockConnectionPool {
        fn new(max_size: usize) -> Self {
            Self {
                connections: Mutex::new(VecDeque::with_capacity(max_size)),
                max_size,
            }
        }

        fn acquire(&self) -> Option<u32> {
            let mut connections = self.connections.lock().unwrap();
            if connections.is_empty() {
                // 创建新连接
                Some(1)
            } else {
                connections.pop_front()
            }
        }

        fn release(&self, conn: u32) {
            let mut connections = self.connections.lock().unwrap();
            if connections.len() < self.max_size {
                connections.push_back(conn);
            }
        }
    }

    let pool = MockConnectionPool::new(10);

    // 获取连接
    let conn = pool.acquire().ok_or("无法获取连接")?;

    // 释放连接
    pool.release(conn);

    Ok(())
}
