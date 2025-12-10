//! System 交易系统模块
//!
//! 本模块提供了用于组合交易引擎和执行组件的顶级交易系统架构。
//! 系统框架抽象了底层并发和通信机制，让用户专注于实现交易策略。
//!
//! # 核心概念
//!
//! - **System**: 初始化和运行中的 Barter 交易系统
//! - **SystemBuilder**: 用于构建交易系统的构建器
//! - **SystemConfig**: 用于定义交易系统的配置
//! - **SystemAuxillaryHandles**: 辅助系统组件的任务句柄集合
//!
//! # 系统架构
//!
//! 交易系统由以下组件组成：
//! - Engine 处理器核心
//! - 执行组件（ExecutionManager）
//! - 市场数据流
//! - 账户事件流
//! - 审计流（可选）

use crate::{
    engine::{
        Processor,
        audit::{AuditTick, Auditor, context::EngineContext},
        command::Command,
        state::{instrument::filter::InstrumentFilter, trading::TradingState},
    },
    execution::builder::ExecutionHandles,
    shutdown::{AsyncShutdown, Shutdown},
};
use barter_execution::order::request::{OrderRequestCancel, OrderRequestOpen};
use barter_integration::{
    channel::{Tx, UnboundedRx, UnboundedTx},
    collection::one_or_many::OneOrMany,
    snapshot::SnapUpdates,
};
use std::fmt::Debug;
use tokio::task::{JoinError, JoinHandle};

/// 提供用于构建 Barter 交易系统的 `SystemBuilder` 及相关类型。
pub mod builder;

/// 提供用于定义 Barter 交易系统的便捷 `SystemConfig`。
pub mod config;

/// 已初始化并运行中的 Barter 交易系统。
///
/// System 包含 `Engine` 和所有辅助系统任务的句柄。它提供了与系统交互的方法，
/// 如发送 `Engine` [`Command`]、管理 [`TradingState`] 和优雅关闭。
///
/// ## 类型参数
///
/// - `Engine`: Engine 类型，必须实现 `Processor<Event>` 和 `Auditor`
/// - `Event`: Engine 事件类型
///
/// ## 系统组件
///
/// - **engine**: 运行中的 Engine 任务句柄
/// - **handles**: 辅助系统组件句柄（执行组件、事件转发等）
/// - **feed_tx**: 用于向 Engine 发送事件的发送器
/// - **audit**: 可选的审计快照和更新流（启用审计时存在）
///
/// ## 使用场景
///
/// - 发送命令到 Engine
/// - 管理交易状态
/// - 关闭系统
/// - 访问审计流
///
/// # 使用示例
///
/// ```rust,ignore
/// // 构建并初始化系统
/// let system = SystemBuilder::new()
///     .with_engine(engine_config)
///     .with_execution(execution_config)
///     .build()
///     .init()
///     .await?;
///
/// // 发送命令
/// system.send_cancel_requests(requests);
///
/// // 关闭系统
/// let (engine, audit) = system.shutdown().await?;
/// ```
#[allow(missing_debug_implementations)]
pub struct System<Engine, Event>
where
    Engine: Processor<Event> + Auditor<Engine::Audit, Context = EngineContext>,
{
    /// 运行中的 `Engine` 任务句柄。
    pub engine: JoinHandle<(Engine, Engine::Audit)>,

    /// 辅助系统组件的句柄（执行组件、事件转发等）。
    pub handles: SystemAuxillaryHandles,

    /// 用于向 `Engine` 发送事件的发送器。
    pub feed_tx: UnboundedTx<Event>,

    /// 可选的审计快照和更新（启用审计发送时存在）。
    pub audit:
        Option<SnapUpdates<AuditTick<Engine::Snapshot>, UnboundedRx<AuditTick<Engine::Audit>>>>,
}

impl<Engine, Event> System<Engine, Event>
where
    Engine: Processor<Event> + Auditor<Engine::Audit, Context = EngineContext>,
    Event: Debug + Clone + Send,
{
    /// 优雅地关闭 `System`。
    ///
    /// 此方法发送关闭信号到 Engine，等待 Engine 完成，然后关闭所有辅助组件。
    ///
    /// ## 关闭流程
    ///
    /// 1. 发送 `Shutdown` 事件到 Engine
    /// 2. 等待 Engine 任务完成
    /// 3. 关闭所有辅助组件
    ///
    /// # 返回值
    ///
    /// 返回 Engine 实例和关闭审计信息。
    pub async fn shutdown(mut self) -> Result<(Engine, Engine::Audit), JoinError>
    where
        Event: From<Shutdown>,
    {
        self.send(Shutdown);

        let (engine, shutdown_audit) = self.engine.await?;

        self.handles.shutdown().await?;

        Ok((engine, shutdown_audit))
    }

    /// 非优雅地关闭 `System`。
    ///
    /// 此方法发送关闭信号到 Engine，等待 Engine 完成，然后立即中止所有辅助组件。
    ///
    /// ## 关闭流程
    ///
    /// 1. 发送 `Shutdown` 事件到 Engine
    /// 2. 等待 Engine 任务完成
    /// 3. 立即中止所有辅助组件（不等待优雅关闭）
    ///
    /// # 返回值
    ///
    /// 返回 Engine 实例和关闭审计信息。
    pub async fn abort(self) -> Result<(Engine, Engine::Audit), JoinError>
    where
        Event: From<Shutdown>,
    {
        self.send(Shutdown);

        let (engine, shutdown_audit) = self.engine.await?;

        self.handles.abort();

        Ok((engine, shutdown_audit))
    }

    /// 在 `MarketStreamEvent` 流结束后优雅地关闭回测 `System`。
    ///
    /// 此方法专门用于回测场景，等待市场数据流结束后再关闭系统。
    ///
    /// **注意：对于实盘和模拟交易，市场流永远不会结束，因此请使用 System::shutdown() 方法**。
    ///
    /// ## 关闭流程
    ///
    /// 1. 等待市场数据流完成转发到 Engine
    /// 2. 发送 `Shutdown` 事件到 Engine
    /// 3. 等待 Engine 任务完成
    /// 4. 中止账户事件转发任务
    /// 5. 关闭执行组件
    ///
    /// # 返回值
    ///
    /// 返回 Engine 实例和关闭审计信息。
    pub async fn shutdown_after_backtest(self) -> Result<(Engine, Engine::Audit), JoinError>
    where
        Event: From<Shutdown>,
    {
        let Self {
            engine,
            handles:
                SystemAuxillaryHandles {
                    mut execution,
                    market_to_engine,
                    account_to_engine,
                },
            feed_tx,
            audit: _,
        } = self;

        // Wait for MarketStream to finish forwarding to Engine before initiating Shutdown
        market_to_engine.await?;

        feed_tx
            .send(Shutdown)
            .expect("Engine cannot drop Feed receiver");
        drop(feed_tx);

        let (engine, shutdown_audit) = engine.await?;

        account_to_engine.abort();
        execution.shutdown().await?;

        Ok((engine, shutdown_audit))
    }

    /// 发送 [`OrderRequestCancel`] 到 `Engine` 执行。
    ///
    /// # 参数
    ///
    /// - `requests`: 取消订单请求（单个或多个）
    pub fn send_cancel_requests(&self, requests: OneOrMany<OrderRequestCancel>)
    where
        Event: From<Command>,
    {
        self.send(Command::SendCancelRequests(requests))
    }

    /// 发送 [`OrderRequestOpen`] 到 `Engine` 执行。
    ///
    /// # 参数
    ///
    /// - `requests`: 开仓订单请求（单个或多个）
    pub fn send_open_requests(&self, requests: OneOrMany<OrderRequestOpen>)
    where
        Event: From<Command>,
    {
        self.send(Command::SendOpenRequests(requests))
    }

    /// 指示 `Engine` 平仓开放仓位。
    ///
    /// 使用 `InstrumentFilter` 配置要平仓的仓位。
    ///
    /// # 参数
    ///
    /// - `filter`: 交易对过滤器，用于筛选要平仓的仓位
    pub fn close_positions(&self, filter: InstrumentFilter)
    where
        Event: From<Command>,
    {
        self.send(Command::ClosePositions(filter))
    }

    /// 指示 `Engine` 取消开放订单。
    ///
    /// 使用 `InstrumentFilter` 配置要取消的订单。
    ///
    /// # 参数
    ///
    /// - `filter`: 交易对过滤器，用于筛选要取消的订单
    pub fn cancel_orders(&self, filter: InstrumentFilter)
    where
        Event: From<Command>,
    {
        self.send(Command::CancelOrders(filter))
    }

    /// 更新 `Engine` 的算法 `TradingState`。
    ///
    /// # 参数
    ///
    /// - `trading_state`: 新的交易状态
    pub fn trading_state(&self, trading_state: TradingState)
    where
        Event: From<TradingState>,
    {
        self.send(trading_state)
    }

    /// 如果存在，获取审计快照和更新的所有权。
    ///
    /// 注意：如果 `System` 是在 [`AuditMode::Disabled`](builder::AuditMode)（默认）模式下构建的，
    /// 则此方法将返回 `None`。
    ///
    /// # 返回值
    ///
    /// 返回审计快照和更新流，如果未启用审计则返回 `None`。
    pub fn take_audit(
        &mut self,
    ) -> Option<SnapUpdates<AuditTick<Engine::Snapshot>, UnboundedRx<AuditTick<Engine::Audit>>>>
    {
        self.audit.take()
    }

    /// 向 `Engine` 发送 `Event`。
    ///
    /// 这是一个内部辅助方法，用于将事件发送到 Engine 的事件流。
    ///
    /// # 参数
    ///
    /// - `event`: 要发送的事件（会被转换为 `Event` 类型）
    fn send<T>(&self, event: T)
    where
        T: Into<Event>,
    {
        self.feed_tx
            .send(event)
            .expect("Engine cannot drop Feed receiver")
    }
}

/// 支持 `Engine` 的辅助系统组件任务句柄集合。
///
/// 由 [`System`] 用于关闭辅助组件。
///
/// ## 组件说明
///
/// - **execution**: 运行执行组件的句柄
/// - **market_to_engine**: 将市场事件转发到 Engine 的任务
/// - **account_to_engine**: 将账户事件转发到 Engine 的任务
#[allow(missing_debug_implementations)]
pub struct SystemAuxillaryHandles {
    /// 运行执行组件的句柄。
    pub execution: ExecutionHandles,

    /// 将市场事件转发到 Engine 的任务。
    pub market_to_engine: JoinHandle<()>,

    /// 将账户事件转发到 Engine 的任务。
    pub account_to_engine: JoinHandle<()>,
}

impl AsyncShutdown for SystemAuxillaryHandles {
    type Result = Result<(), JoinError>;

    /// 优雅地关闭所有辅助组件。
    ///
    /// 此方法会中止事件转发任务（不需要优雅关闭），然后等待执行组件优雅关闭。
    async fn shutdown(&mut self) -> Self::Result {
        // 事件 -> Engine 任务不需要优雅关闭，直接中止
        self.market_to_engine.abort();
        self.account_to_engine.abort();

        // 并发等待执行组件关闭
        self.execution.shutdown().await
    }
}

impl SystemAuxillaryHandles {
    /// 立即中止所有辅助组件。
    ///
    /// 此方法会立即中止所有任务，不等待优雅关闭。
    pub fn abort(self) {
        self.execution
            .into_iter()
            .chain(std::iter::once(self.market_to_engine))
            .chain(std::iter::once(self.account_to_engine))
            .for_each(|handle| handle.abort());
    }
}
