//! Engine 审计上下文模块
//!
//! 本模块定义了 Engine 审计事件的上下文信息，包括序列号和生成时间。
//! 这些信息用于追踪和排序审计事件。

use crate::Sequence;
use chrono::{DateTime, Utc};
use derive_more::Constructor;
use serde::{Deserialize, Serialize};

/// `Engine` 生成 [`AuditTick`](super::AuditTick) 时的上下文信息。
///
/// EngineContext 包含审计事件生成时的关键元数据，用于追踪事件的时间顺序和生成时间。
///
/// ## 字段说明
///
/// - **sequence**: 事件序列号，用于确定事件的顺序
/// - **time**: 事件生成时间（UTC）
///
/// ## 使用场景
///
/// - 事件排序和去重
/// - 时间序列分析
/// - 审计日志记录
///
/// # 使用示例
///
/// ```rust,ignore
/// let context = EngineContext {
///     sequence: Sequence::new(100),
///     time: Utc::now(),
/// };
///
/// let audit_tick = AuditTick {
///     event: some_event,
///     context,
/// };
/// ```
#[derive(
    Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, Constructor,
)]
pub struct EngineContext {
    /// 事件序列号，用于确定事件的顺序。
    pub sequence: Sequence,
    /// 事件生成时间（UTC）。
    pub time: DateTime<Utc>,
}
