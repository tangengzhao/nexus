//! HSB 集群通信模块
//!
//! 为 K8s 多副本部署提供副本间通信、服务发现和状态同步能力。
//! 基于 NATS 实现副本间的事件广播、领导选举和分布式协调。

pub mod discovery;
pub mod sync;

pub use discovery::{PeerInfo, PeerStatus, ServiceDiscovery};
pub use sync::{ClusterEvent, ClusterState, ClusterSync};
