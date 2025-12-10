//! Execution 执行模块
//!
//! 本模块定义了 Engine 的执行管理系统，负责处理订单请求并与交易所通信。
//! 执行管理器是 Engine 与交易所之间的桥梁，处理订单执行、账户事件等。
//!
//! # 核心概念
//!
//! - **Execution**: 执行系统的主要结构，包含执行通道和任务句柄
//! - **ExecutionManager**: 每个交易所的执行管理器
//! - **ExecutionRequest**: Engine 与 ExecutionManager 之间的通信请求
//! - **AccountStreamEvent**: 账户事件流事件
//!
//! # 工作流程
//!
//! 1. Engine 发送 ExecutionRequest 到 ExecutionManager
//! 2. ExecutionManager 处理请求并发送到交易所
//! 3. ExecutionManager 接收交易所响应并转发回 Engine
//! 4. AccountStreamEvent 用于传递账户相关事件

use crate::{engine::execution_tx::MultiExchangeTxMap, execution::builder::ExecutionHandles};
use barter_data::streams::reconnect;
use barter_execution::AccountEvent;
use barter_instrument::{
    asset::AssetIndex,
    exchange::{ExchangeId, ExchangeIndex},
    instrument::InstrumentIndex,
};
use barter_integration::channel::Channel;

/// 提供执行管理器构建器，用于方便地初始化到模拟和真实交易所的多个执行链接。
pub mod builder;

/// 提供表示执行链接生成的所有错误的错误类型。
pub mod error;

/// 每个交易所的执行管理器，处理来自 Engine 的订单请求并转发响应。
pub mod manager;

/// 定义 `Engine` 用于与 `ExecutionManager` 通信的 `ExecutionRequest`。
pub mod request;

/// 表示由 [`AccountEvent`] 流产生的 [`reconnect::Event`] 的便捷类型别名。
///
/// AccountStreamEvent 用于在 Engine 和 ExecutionManager 之间传递账户相关事件，
/// 包括账户更新、重连事件等。
///
/// ## 类型参数
///
/// - `ExchangeKey`: 交易所键类型，默认为 `ExchangeIndex`
/// - `AssetKey`: 资产键类型，默认为 `AssetIndex`
/// - `InstrumentKey`: 交易对键类型，默认为 `InstrumentIndex`
pub type AccountStreamEvent<
    ExchangeKey = ExchangeIndex,
    AssetKey = AssetIndex,
    InstrumentKey = InstrumentIndex,
> = reconnect::Event<ExchangeId, AccountEvent<ExchangeKey, AssetKey, InstrumentKey>>;

/// 已初始化的 [`ExecutionBuild`](builder::ExecutionBuild)。
///
/// Execution 结构包含执行系统的所有组件，包括执行通道、账户事件通道和任务句柄。
/// 这是执行系统的完整配置，用于与多个交易所进行通信。
///
/// ## 字段说明
///
/// - **execution_txs**: 多交易所执行请求通道映射
/// - **account_channel**: 账户事件通道
/// - **handles**: 执行组件任务句柄
///
/// ## 使用场景
///
/// - 初始化执行系统
/// - 管理多个交易所的执行链接
/// - 处理账户事件流
///
/// # 使用示例
///
/// ```rust,ignore
/// let execution = ExecutionBuilder::new()
///     .add_exchange(exchange_config)
///     .build()
///     .await?;
///
/// // 使用执行通道发送请求
/// execution.execution_txs.find(&exchange_id)?.send(request)?;
///
/// // 接收账户事件
/// while let Some(event) = execution.account_channel.recv().await? {
///     // 处理账户事件
/// }
/// ```
#[allow(missing_debug_implementations)]
pub struct Execution {
    /// 多交易所执行请求通道映射。
    pub execution_txs: MultiExchangeTxMap,
    /// 账户事件通道。
    pub account_channel: Channel<AccountStreamEvent>,
    /// 执行组件任务句柄。
    pub handles: ExecutionHandles,
}
