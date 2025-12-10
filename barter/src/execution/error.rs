//! Execution 错误模块
//!
//! 本模块定义了执行链接生成的所有错误类型。
//! ExecutionError 用于表示执行系统中的各种错误情况。
//!
//! # 错误类型
//!
//! - **Config**: 执行管理器配置无效
//! - **Index**: 索引映射错误
//! - **Client**: 执行客户端错误

use barter_execution::error::ClientError;
use barter_instrument::index::error::IndexError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 表示执行链接生成的所有错误。
///
/// ExecutionError 枚举包含执行系统中可能发生的所有错误类型。
/// 这些错误可能来自配置问题、索引映射问题或执行客户端问题。
///
/// ## 错误类型
///
/// - **Config**: 执行管理器配置无效
/// - **Index**: 索引映射错误（从交易所数据结构映射到索引数据结构时发生）
/// - **Client**: 执行客户端错误（来自 [`ExecutionClient`](barter_execution::client::ExecutionClient)）
///
/// # 使用示例
///
/// ```rust,ignore
/// match execution_result {
///     Ok(response) => {
///         // 处理成功响应
///     }
///     Err(ExecutionError::Config(msg)) => {
///         error!("Configuration error: {}", msg);
///     }
///     Err(ExecutionError::Index(err)) => {
///         error!("Index mapping error: {}", err);
///     }
///     Err(ExecutionError::Client(err)) => {
///         error!("Client error: {}", err);
///     }
/// }
/// ```
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, Error)]
pub enum ExecutionError {
    /// 表示执行管理器配置无效。
    #[error("ExecutionManager config invalid: {0}")]
    Config(String),

    /// 表示在将交易所中心数据结构映射到其索引对应结构时发生的错误。
    #[error("IndexError: {0}")]
    Index(#[from] IndexError),

    /// 表示由 [`ExecutionClient`](barter_execution::client::ExecutionClient) 产生的所有错误。
    #[error("{0}")]
    Client(#[from] ClientError),
}
