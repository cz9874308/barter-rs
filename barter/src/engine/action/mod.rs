//! Engine 操作模块
//!
//! 本模块定义了 Engine 执行的各种操作，包括生成算法订单、取消订单、开仓订单、平仓等。
//! 这些操作是 Engine 响应 [`Command`](super::command::Command) 时执行的具体动作。
//!
//! # 核心概念
//!
//! - **ActionOutput**: 操作输出枚举，包含所有操作的结果
//! - **generate_algo_orders**: 生成算法订单操作
//! - **cancel_orders**: 取消订单操作
//! - **close_positions**: 平仓操作
//! - **send_requests**: 发送执行请求操作
//!
//! # 操作流程
//!
//! 1. Engine 接收 Command
//! 2. 根据 Command 类型执行相应的 Action
//! 3. 返回 ActionOutput，包含操作结果和可能的错误

use crate::engine::{
    action::{
        generate_algo_orders::GenerateAlgoOrdersOutput,
        send_requests::{SendCancelsAndOpensOutput, SendRequestsOutput},
    },
    error::UnrecoverableEngineError,
};
use barter_execution::order::request::{RequestCancel, RequestOpen};
use barter_instrument::{exchange::ExchangeIndex, instrument::InstrumentIndex};
use barter_integration::collection::one_or_many::OneOrMany;
use derive_more::From;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// 定义 `Engine` 的取消开仓订单请求操作。
pub mod cancel_orders;

/// 定义 `Engine` 的生成和发送平仓订单请求操作。
pub mod close_positions;

/// 定义 `Engine` 的生成和发送算法订单请求操作。
pub mod generate_algo_orders;

/// 定义 `Engine` 的向执行管理器发送订单 `ExecutionRequests` 操作。
pub mod send_requests;

/// `Engine` 在执行 [`Command`](super::command::Command) 后的输出。
///
/// ActionOutput 枚举包含所有可能的操作结果。每个变体对应一种操作类型，包含该操作的
/// 具体输出信息。
///
/// ## 操作类型
///
/// - **GenerateAlgoOrders**: 生成算法订单操作的输出
/// - **CancelOrders**: 取消订单操作的输出
/// - **OpenOrders**: 开仓订单操作的输出
/// - **ClosePositions**: 平仓操作的输出
///
/// ## 类型参数
///
/// - `ExchangeKey`: 交易所键类型，默认为 `ExchangeIndex`
/// - `InstrumentKey`: 交易对键类型，默认为 `InstrumentIndex`
///
/// ## 错误处理
///
/// 所有操作输出都支持提取不可恢复错误，通过 `unrecoverable_errors()` 方法。
///
/// # 使用示例
///
/// ```rust,ignore
/// // 执行命令并获取输出
/// let output = engine.action(&command)?;
///
/// // 检查是否有不可恢复错误
/// if let Some(errors) = output.unrecoverable_errors() {
///     // 处理错误
/// }
///
/// // 根据输出类型处理
/// match output {
///     ActionOutput::GenerateAlgoOrders(algo) => {
///         // 处理算法订单生成结果
///     }
///     ActionOutput::CancelOrders(cancels) => {
///         // 处理取消订单结果
///     }
///     // ...
/// }
/// ```
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, From)]
#[allow(clippy::large_enum_variant)]
pub enum ActionOutput<ExchangeKey = ExchangeIndex, InstrumentKey = InstrumentIndex> {
    /// 生成算法订单操作的输出。
    GenerateAlgoOrders(GenerateAlgoOrdersOutput<ExchangeKey, InstrumentKey>),
    /// 取消订单操作的输出。
    CancelOrders(SendRequestsOutput<RequestCancel, ExchangeKey, InstrumentKey>),
    /// 开仓订单操作的输出。
    OpenOrders(SendRequestsOutput<RequestOpen, ExchangeKey, InstrumentKey>),
    /// 平仓操作的输出。
    ClosePositions(SendCancelsAndOpensOutput<ExchangeKey, InstrumentKey>),
}

impl<ExchangeKey, InstrumentKey> ActionOutput<ExchangeKey, InstrumentKey> {
    /// 返回在执行 `Engine` 操作期间发生的任何不可恢复错误。
    ///
    /// 此方法从所有操作输出中提取不可恢复错误。如果操作过程中没有发生错误，返回 `None`。
    /// 如果发生了一个或多个错误，返回包含错误的 `OneOrMany`。
    ///
    /// ## 错误类型
    ///
    /// 只返回不可恢复错误（`UnrecoverableEngineError`），这些错误通常需要立即处理或
    /// 触发 Engine 关闭。
    ///
    /// # 返回值
    ///
    /// - `Some(OneOrMany<UnrecoverableEngineError>)`: 如果发生了不可恢复错误
    /// - `None`: 如果没有发生错误
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let output = engine.action(&command)?;
    ///
    /// // 检查是否有不可恢复错误
    /// if let Some(errors) = output.unrecoverable_errors() {
    ///     match errors {
    ///         OneOrMany::One(error) => {
    ///             error!("Unrecoverable error: {}", error);
    ///             engine.shutdown().await;
    ///         }
    ///         OneOrMany::Many(errors) => {
    ///             for error in errors {
    ///                 error!("Unrecoverable error: {}", error);
    ///             }
    ///             engine.shutdown().await;
    ///         }
    ///     }
    /// }
    /// ```
    pub fn unrecoverable_errors(&self) -> Option<OneOrMany<UnrecoverableEngineError>> {
        match self {
            ActionOutput::GenerateAlgoOrders(algo) => algo.cancels_and_opens.unrecoverable_errors(),
            ActionOutput::CancelOrders(cancels) => cancels.unrecoverable_errors(),
            ActionOutput::OpenOrders(opens) => opens.unrecoverable_errors(),
            ActionOutput::ClosePositions(requests) => requests.unrecoverable_errors(),
        }
        .into_option()
    }
}
