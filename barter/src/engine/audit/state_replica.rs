//! Engine 状态副本管理器模块
//!
//! 本模块定义了 StateReplicaManager，用于通过处理 Engine 产生的 AuditStream 事件
//! 来维护 EngineState 的副本。这对于支持非热路径的交易系统组件（如 UI、Web 应用等）
//! 非常有用。
//!
//! # 核心概念
//!
//! - **StateReplicaManager**: 状态副本管理器，维护 EngineState 的副本
//! - **工作流程**: 接收审计事件 → 验证序列号 → 更新状态副本
//!
//! # 使用场景
//!
//! - UI 显示当前交易状态
//! - Web 应用提供实时状态查询
//! - 监控和日志系统
//! - 回放和调试

use crate::{
    EngineEvent,
    engine::{
        EngineMeta, EngineOutput, Processor,
        audit::{AuditTick, EngineAudit, context::EngineContext},
        state::{EngineState, instrument::data::InstrumentDataState},
    },
    execution::AccountStreamEvent,
};
use barter_data::{event::MarketEvent, streams::consumer::MarketStreamEvent};
use barter_execution::AccountEvent;
use barter_instrument::instrument::InstrumentIndex;
use barter_integration::Terminal;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use tracing::{info, info_span};

/// 审计副本状态更新的 Tracing Span 名称。
pub const AUDIT_REPLICA_STATE_UPDATE_SPAN_NAME: &str = "audit_replica_state_update_span";

/// 通过处理 `Engine` 产生的 AuditStream 事件来管理 `EngineState` 实例的副本。
///
/// StateReplicaManager 维护一个 EngineState 的副本，通过处理审计流事件来保持同步。
/// 这对于支持非热路径的交易系统组件（如 UI、Web 应用等）非常有用。
///
/// ## 工作原理
///
/// 1. 使用初始状态快照作为种子
/// 2. 处理审计流中的事件
/// 3. 验证事件序列号的连续性
/// 4. 根据事件更新状态副本
///
/// ## 类型参数
///
/// - `State`: 状态类型，通常是 `EngineState`
/// - `Updates`: 更新事件迭代器类型
///
/// ## 使用场景
///
/// - UI 显示当前交易状态
/// - Web 应用提供实时状态查询
/// - 监控和日志系统
/// - 回放和调试
///
/// # 使用示例
///
/// ```rust,ignore
/// // 创建状态副本管理器
/// let mut replica_manager = StateReplicaManager::new(
///     initial_snapshot,
///     audit_stream,
/// );
///
/// // 运行副本管理器
/// replica_manager.run()?;
///
/// // 获取当前状态副本
/// let current_state = replica_manager.replica_engine_state();
/// ```
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub struct StateReplicaManager<State, Updates> {
    /// Engine 元数据起始信息。
    pub meta_start: EngineMeta,
    /// 状态副本（包含状态和上下文）。
    pub state_replica: AuditTick<State, EngineContext>,
    /// 更新事件迭代器。
    pub updates: Updates,
}

impl<State, Updates> StateReplicaManager<State, Updates> {
    /// 使用提供的 `EngineState` 快照作为种子构造新的 `StateReplicaManager`。
    ///
    /// 此构造函数使用初始状态快照来初始化状态副本管理器。快照的上下文信息
    /// 会被用于初始化 Engine 元数据。
    ///
    /// # 参数
    ///
    /// - `snapshot`: 初始状态快照（包含状态和上下文）
    /// - `updates`: 更新事件迭代器
    ///
    /// # 返回值
    ///
    /// 返回新创建的 `StateReplicaManager` 实例。
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let snapshot = engine.audit_snapshot();
    /// let replica_manager = StateReplicaManager::new(snapshot, audit_stream);
    /// ```
    pub fn new(snapshot: AuditTick<State>, updates: Updates) -> Self {
        Self {
            meta_start: EngineMeta {
                time_start: snapshot.context.time,
                sequence: snapshot.context.sequence,
            },
            state_replica: snapshot,
            updates,
        }
    }
}

impl<GlobalData, InstrumentData, Updates>
    StateReplicaManager<EngineState<GlobalData, InstrumentData>, Updates>
where
    InstrumentData: InstrumentDataState,
    GlobalData: for<'a> Processor<&'a AccountEvent>
        + for<'a> Processor<&'a MarketEvent<InstrumentIndex, InstrumentData::MarketEventKind>>,
{
    /// 运行 `StateReplicaManager`，通过处理 `Engine` 产生的 AuditStream 事件来管理
    /// `EngineState` 实例的副本。
    ///
    /// 此方法处理审计流中的所有事件，验证序列号的连续性，并根据事件更新状态副本。
    /// 当遇到终端事件（如 FeedEnded 或 Shutdown）时，方法会返回。
    ///
    /// ## 工作流程
    ///
    /// 1. 创建 Tracing Span 用于过滤重复的日志
    /// 2. 循环处理审计事件
    /// 3. 验证事件序列号的连续性
    /// 4. 根据事件更新状态副本
    /// 5. 检查是否为终端事件
    ///
    /// ## 错误处理
    ///
    /// 如果检测到序列号不连续（乱序事件），方法会返回错误。
    ///
    /// # 类型参数
    ///
    /// - `OnDisable`: 交易禁用输出类型
    /// - `OnDisconnect`: 断开连接输出类型
    ///
    /// # 返回值
    ///
    /// - `Ok(())`: 成功完成
    /// - `Err(String)`: 如果检测到序列号不连续
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let mut replica_manager = StateReplicaManager::new(snapshot, audit_stream);
    /// replica_manager.run()?;
    /// ```
    pub fn run<OnDisable, OnDisconnect>(&mut self) -> Result<(), String>
    where
        Updates: Iterator<
            Item = AuditTick<
                EngineAudit<
                    EngineEvent<InstrumentData::MarketEventKind>,
                    EngineOutput<OnDisable, OnDisconnect>,
                >,
            >,
        >,
        OnDisable: Debug,
        OnDisconnect: Debug,
    {
        info!("StateReplicaManager running");

        // 创建 Tracing Span 用于过滤重复的副本 EngineState 更新日志
        let audit_span = info_span!(AUDIT_REPLICA_STATE_UPDATE_SPAN_NAME);
        let audit_span_guard = audit_span.enter();

        let shutdown_audit = loop {
            // 获取下一个审计事件
            let Some(AuditTick {
                event: EngineAudit::Process(audit),
                context,
            }) = self.updates.next()
            else {
                break "FeedEnded";
            };

            // 检查序列号，跳过已处理的事件
            if self.state_replica.context.sequence >= context.sequence {
                continue;
            } else {
                // 验证并更新上下文
                self.validate_and_update_context(context)?;
            }

            // 检查是否为终端事件
            let shutdown = audit.is_terminal();

            // 根据事件更新状态副本
            self.update_from_event(audit.event);

            if shutdown {
                break "EngineEvent::Shutdown";
            }
        };

        // 结束 Tracing Span
        drop(audit_span_guard);

        info!(%shutdown_audit, "AuditManager stopped");

        Ok(())
    }

    /// 验证并更新上下文。
    ///
    /// 此方法验证下一个事件的序列号是否连续（应该是当前序列号 + 1）。
    /// 如果序列号不连续，返回错误。
    ///
    /// # 参数
    ///
    /// - `next`: 下一个事件的上下文
    ///
    /// # 返回值
    ///
    /// - `Ok(())`: 序列号连续，上下文已更新
    /// - `Err(String)`: 序列号不连续（乱序事件）
    fn validate_and_update_context(&mut self, next: EngineContext) -> Result<(), String> {
        if self.state_replica.context.sequence.value() != next.sequence.value() - 1 {
            return Err(format!(
                "AuditManager | out-of-order AuditStream | next: {:?} does not follow from {:?}",
                next.sequence, self.state_replica.context.sequence,
            ));
        }

        self.state_replica.context = next;
        Ok(())
    }

    /// 使用提供的 `EngineEvent` 更新内部的 `EngineState`。
    ///
    /// 此方法根据事件类型更新状态副本的不同部分：
    ///
    /// - **TradingStateUpdate**: 更新交易状态
    /// - **Account**: 更新账户相关状态（包括重连状态）
    /// - **Market**: 更新市场相关状态（包括重连状态）
    /// - **Shutdown/Command**: 不需要更新状态
    ///
    /// # 参数
    ///
    /// - `event`: Engine 事件
    pub fn update_from_event(&mut self, event: EngineEvent<InstrumentData::MarketEventKind>) {
        match event {
            EngineEvent::Shutdown(_) | EngineEvent::Command(_) => {
                // 不需要操作
            }
            EngineEvent::TradingStateUpdate(trading_state) => {
                // 更新交易状态
                let _audit = self
                    .replica_engine_state_mut()
                    .trading
                    .update(trading_state);
            }
            EngineEvent::Account(event) => match event {
                AccountStreamEvent::Reconnecting(exchange) => {
                    // 更新账户重连状态
                    self.replica_engine_state_mut()
                        .connectivity
                        .update_from_account_reconnecting(&exchange);
                }
                AccountStreamEvent::Item(event) => {
                    // 更新账户状态
                    self.replica_engine_state_mut().update_from_account(&event);
                }
            },
            EngineEvent::Market(event) => match event {
                MarketStreamEvent::Reconnecting(exchange) => {
                    // 更新市场重连状态
                    self.replica_engine_state_mut()
                        .connectivity
                        .update_from_market_reconnecting(&exchange);
                }
                MarketStreamEvent::Item(event) => {
                    // 更新市场状态
                    self.replica_engine_state_mut().update_from_market(&event);
                }
            },
        }
    }

    /// 返回 `EngineState` 副本的引用。
    ///
    /// 此方法返回当前状态副本的不可变引用，用于查询状态。
    ///
    /// # 返回值
    ///
    /// 返回状态副本的引用。
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let state = replica_manager.replica_engine_state();
    /// println!("Current positions: {}", state.instruments.positions().count());
    /// ```
    pub fn replica_engine_state(&self) -> &EngineState<GlobalData, InstrumentData> {
        &self.state_replica.event
    }

    /// 返回 `EngineState` 副本的可变引用。
    ///
    /// 此方法用于内部更新状态副本。
    fn replica_engine_state_mut(&mut self) -> &mut EngineState<GlobalData, InstrumentData> {
        &mut self.state_replica.event
    }
}
