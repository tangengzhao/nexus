//! HSB Common - 医院服务总线公共类型和工具
//!
//! 本模块提供 HSB 系统的基础类型定义、错误处理和公共工具。

pub mod config;
pub mod constants;
pub mod error;
pub mod sso_client;
pub mod types;
pub mod utils;

pub use error::{HsbError, HsbResult};
pub use sso_client::{CASClient, CASValidationResponse, SSOClient, TokenResponse, UserInfo};
pub use types::*;
