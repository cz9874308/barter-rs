//! Engine 错误处理模块
//!
//! 本模块定义了 Engine 可能遇到的所有错误类型。错误被分为可恢复错误和不可恢复错误，
//! 这种分类对于 Engine 的错误处理策略至关重要。
//!
//! # 核心概念
//!
//! - **EngineError**: 顶层错误枚举，包含所有可能的错误
//! - **RecoverableEngineError**: 可恢复错误，Engine 可以继续运行
//! - **UnrecoverableEngineError**: 不可恢复错误，Engine 需要优雅关闭
//!
//! # 错误分类
//!
//! ## 可恢复错误
//!
//! 这些错误通常是临时性的，Engine 可以从中恢复并继续运行：
//!
//! - 网络连接问题
//! - 执行通道暂时不可用
//! - 临时性的资源问题
//!
//! ## 不可恢复错误
//!
//! 这些错误是致命性的，需要人工干预或系统重启：
//!
//! - 索引错误（数据结构损坏）
//! - 执行通道永久终止
//! - 自定义致命错误
//!
//! # 错误处理策略
//!
//! - **可恢复错误**: Engine 记录错误并继续运行
//! - **不可恢复错误**: Engine 触发优雅关闭流程

use barter_instrument::index::error::IndexError;
use barter_integration::Unrecoverable;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 表示 [`Engine`](super::Engine) 可能遇到的所有错误。
///
/// EngineError 是顶层错误枚举，包含了 Engine 可能遇到的所有错误类型。
/// 错误被分为可恢复错误和不可恢复错误，这种分类决定了 Engine 的错误处理策略。
///
/// ## 错误分类
///
/// - **可恢复错误**: 不会导致 Engine 终止，Engine 可以继续运行
/// - **不可恢复错误**: 会导致 Engine 优雅关闭，需要人工干预或系统重启
///
/// ## 为什么需要错误分类？
///
/// 不同的错误需要不同的处理策略：
///
/// - **可恢复错误**: 通常是临时性的（如网络问题），Engine 可以等待并重试
/// - **不可恢复错误**: 通常是致命性的（如数据结构损坏），需要立即停止以避免数据损坏
///
/// ## 使用场景
///
/// - Engine 内部错误处理
/// - 错误日志记录
/// - 错误传播和报告
///
/// # 使用示例
///
/// ```rust,ignore
/// // 处理可恢复错误
/// match engine_error {
///     EngineError::Recoverable(err) => {
///         // 记录错误，继续运行
///         warn!("Recoverable error: {}", err);
///     }
///     EngineError::Unrecoverable(err) => {
///         // 触发优雅关闭
///         error!("Unrecoverable error: {}", err);
///         engine.shutdown().await;
///     }
/// }
///
/// // 检查是否为不可恢复错误
/// if engine_error.is_unrecoverable() {
///     // 触发关闭流程
/// }
/// ```
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, Error)]
pub enum EngineError {
    /// 可恢复错误。
    ///
    /// 这些错误通常是临时性的，Engine 可以从中恢复并继续运行。
    ///
    /// # 常见场景
    ///
    /// - 执行通道暂时不可用
    /// - 网络连接问题
    /// - 临时性的资源问题
    ///
    /// # 处理方式
    ///
    /// Engine 会记录错误并继续运行，通常会等待一段时间后重试。
    #[error("recoverable error: {0}")]
    Recoverable(#[from] RecoverableEngineError),

    /// 不可恢复错误。
    ///
    /// 这些错误是致命性的，需要人工干预或系统重启才能解决。
    ///
    /// # 常见场景
    ///
    /// - 索引错误（数据结构损坏）
    /// - 执行通道永久终止
    /// - 自定义致命错误
    ///
    /// # 处理方式
    ///
    /// Engine 会触发优雅关闭流程，停止所有操作并清理资源。
    #[error("unrecoverable error: {0}")]
    Unrecoverable(#[from] UnrecoverableEngineError),
}

/// 表示 [`Engine`](super::Engine) 可以从中恢复的临时错误条件。
///
/// RecoverableEngineError 表示那些通常是临时性的错误，Engine 可以等待并重试，
/// 而不需要立即终止。这些错误通常代表瞬态问题，如网络问题、临时资源不可用等。
///
/// ## 为什么这些错误是可恢复的？
///
/// - **临时性**: 这些错误通常是临时性的，稍后可能会自动解决
/// - **不影响数据完整性**: 这些错误不会导致数据损坏或状态不一致
/// - **可以重试**: Engine 可以等待一段时间后重试操作
///
/// ## 处理策略
///
/// Engine 遇到可恢复错误时：
///
/// 1. 记录错误日志
/// 2. 继续运行（不终止）
/// 3. 等待一段时间后重试
/// 4. 如果持续失败，可能会升级为不可恢复错误
///
/// # 使用示例
///
/// ```rust,ignore
/// match error {
///     RecoverableEngineError::ExecutionChannelUnhealthy(msg) => {
///         warn!("Execution channel unhealthy: {}", msg);
///         // 等待一段时间后重试
///         tokio::time::sleep(Duration::from_secs(1)).await;
///     }
/// }
/// ```
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, Error)]
pub enum RecoverableEngineError {
    /// 执行请求通道不健康。
    ///
    /// 此错误表示执行请求通道（ExecutionRequest channel）处于不健康状态，
    /// 但可能稍后会恢复。这通常是临时性的网络问题或连接问题。
    ///
    /// # 参数
    ///
    /// - `String`: 错误消息，描述通道不健康的原因
    ///
    /// # 常见原因
    ///
    /// - 网络连接暂时中断
    /// - 执行服务暂时不可用
    /// - 通道缓冲区满（临时性）
    ///
    /// # 处理方式
    ///
    /// Engine 会记录错误并等待一段时间后重试。如果持续失败，可能会升级为不可恢复错误。
    #[error("ExecutionRequest channel unhealthy: {0}")]
    ExecutionChannelUnhealthy(String),
}

/// 表示 [`Engine`](super::Engine) 无法从中恢复的致命错误条件。
///
/// UnrecoverableEngineError 表示那些致命性的错误，Engine 无法自动恢复，
/// 需要人工干预或系统重启才能解决。这些错误通常代表根本性问题，如数据结构损坏、
/// 永久性的资源不可用等。
///
/// ## 为什么这些错误是不可恢复的？
///
/// - **根本性问题**: 这些错误通常代表系统配置或数据结构的根本性问题
/// - **可能导致数据损坏**: 继续运行可能导致数据损坏或状态不一致
/// - **需要人工干预**: 通常需要人工检查并修复问题
///
/// ## 处理策略
///
/// Engine 遇到不可恢复错误时：
///
/// 1. 记录错误日志（error 级别）
/// 2. 触发优雅关闭流程
/// 3. 停止所有操作
/// 4. 清理资源
/// 5. 通知外部系统
///
/// # 使用示例
///
/// ```rust,ignore
/// match error {
///     UnrecoverableEngineError::IndexError(err) => {
///         error!("Index error: {}", err);
///         // 触发优雅关闭
///         engine.shutdown().await;
///     }
///     UnrecoverableEngineError::ExecutionChannelTerminated(msg) => {
///         error!("Execution channel terminated: {}", msg);
///         // 触发优雅关闭
///         engine.shutdown().await;
///     }
///     UnrecoverableEngineError::Custom(msg) => {
///         error!("Custom fatal error: {}", msg);
///         // 触发优雅关闭
///         engine.shutdown().await;
///     }
/// }
/// ```
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, Error)]
pub enum UnrecoverableEngineError {
    /// 索引错误。
    ///
    /// 此错误表示索引数据结构出现问题，可能是索引损坏、索引键不存在等。
    /// 这通常是致命性的，因为索引是 Engine 的核心数据结构。
    ///
    /// # 参数
    ///
    /// - `IndexError`: 底层索引错误
    ///
    /// # 常见原因
    ///
    /// - 索引键不存在
    /// - 索引数据结构损坏
    /// - 索引初始化失败
    ///
    /// # 处理方式
    ///
    /// Engine 会立即触发优雅关闭，因为索引错误可能导致数据不一致。
    #[error("IndexError: {0}")]
    IndexError(#[from] IndexError),

    /// 执行请求通道已终止。
    ///
    /// 此错误表示执行请求通道（ExecutionRequest channel）已永久终止，
    /// 无法再使用。这通常是致命性的，因为 Engine 无法再发送订单请求。
    ///
    /// # 参数
    ///
    /// - `String`: 错误消息，描述通道终止的原因
    ///
    /// # 常见原因
    ///
    /// - 执行服务永久关闭
    /// - 通道被显式关闭
    /// - 系统资源耗尽
    ///
    /// # 处理方式
    ///
    /// Engine 会立即触发优雅关闭，因为无法发送订单请求意味着 Engine 无法正常工作。
    #[error("ExecutionRequest channel terminated: {0}")]
    ExecutionChannelTerminated(String),

    /// 自定义致命错误。
    ///
    /// 此错误用于表示其他类型的致命错误，允许外部代码定义自定义的致命错误条件。
    ///
    /// # 参数
    ///
    /// - `String`: 自定义错误消息
    ///
    /// # 使用场景
    ///
    /// - 外部系统检测到致命问题
    /// - 自定义业务逻辑检测到致命条件
    /// - 需要传递自定义错误消息的致命错误
    ///
    /// # 处理方式
    ///
    /// Engine 会立即触发优雅关闭，并记录自定义错误消息。
    #[error("{0}")]
    Custom(String),
}

impl Unrecoverable for EngineError {
    /// 检查错误是否为不可恢复错误。
    ///
    /// 此方法用于快速判断错误是否需要触发优雅关闭流程。
    ///
    /// # 返回值
    ///
    /// - `true`: 错误是不可恢复的，需要触发优雅关闭
    /// - `false`: 错误是可恢复的，Engine 可以继续运行
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// if engine_error.is_unrecoverable() {
    ///     // 触发优雅关闭
    ///     engine.shutdown().await;
    /// } else {
    ///     // 记录错误，继续运行
    ///     warn!("Recoverable error: {}", engine_error);
    /// }
    /// ```
    fn is_unrecoverable(&self) -> bool {
        matches!(self, EngineError::Unrecoverable(_))
    }
}
