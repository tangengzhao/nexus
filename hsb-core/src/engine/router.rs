//! 路由匹配器实现

use crate::{Message, Route};
use async_trait::async_trait;
use hsb_common::HsbResult;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::Router;

/// 内存路由器
pub struct InMemoryRouter {
    routes: Arc<RwLock<HashMap<String, Route>>>,
}

impl InMemoryRouter {
    pub fn new() -> Self {
        Self {
            routes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 检查消息是否匹配路由 - 使用 Route 自带的 matches 方法
    fn matches(&self, msg: &Message, route: &Route) -> bool {
        route.matches(msg)
    }
}

impl Default for InMemoryRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Router for InMemoryRouter {
    async fn find_routes(&self, msg: &Message) -> HsbResult<Vec<Route>> {
        let routes = self.routes.read().await;

        let mut matched: Vec<Route> = routes
            .values()
            .filter(|route| self.matches(msg, route))
            .cloned()
            .collect();

        // 按优先级排序（数字越大优先级越高）
        matched.sort_by_key(|r| std::cmp::Reverse(r.priority));

        Ok(matched)
    }

    async fn add_route(&self, route: Route) -> HsbResult<()> {
        let mut routes = self.routes.write().await;
        routes.insert(route.id.to_string(), route);
        Ok(())
    }

    async fn remove_route(&self, route_id: &str) -> HsbResult<()> {
        let mut routes = self.routes.write().await;
        routes.remove(route_id);
        Ok(())
    }

    async fn list_routes(&self) -> HsbResult<Vec<Route>> {
        let routes = self.routes.read().await;
        Ok(routes.values().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MessageBuilder, RouteBuilder, RouteTarget, SourceMatch};
    use hsb_common::ProtocolType;

    #[tokio::test]
    async fn test_route_matching() {
        let router = InMemoryRouter::new();

        // 添加路由
        let route = RouteBuilder::new()
            .id("route1")
            .name("Test Route")
            .source(SourceMatch::system("HIS"))
            .target(RouteTarget::primary("LIS"))
            .build()
            .expect("Should build route");

        router.add_route(route).await.expect("Should add route");

        // 创建消息
        let msg = MessageBuilder::new()
            .source_system("HIS")
            .protocol(ProtocolType::Hl7V2)
            .message_type("ADT^A01")
            .raw_payload("test")
            .build()
            .expect("Should build message");

        // 查找路由
        let routes = router.find_routes(&msg).await.expect("Should find routes");
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].name, "Test Route");
    }
}
