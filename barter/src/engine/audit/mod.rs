//! Engine 审计模块
//!
//! 本模块定义了 Engine 的审计系统，用于记录和追踪 Engine 的状态变化和事件处理。
//! 审计系统通过 AuditStream 发送审计事件，支持非热路径组件（如 UI、Web 应用等）的
//! 状态同步和监控。
//!
//! # 核心概念
//!
//! - **Auditor**: Trait，定义生成审计事件的接口
//! - **AuditTick**: 审计事件及其上下文
//! - **EngineAudit**: Engine 生成的审计事件类型
//! - **ProcessAudit**: 处理事件时的审计信息
//! - **EngineContext**: 审计事件的上下文（序列号和时间戳）
//!
//! # 使用场景
//!
//! - 状态同步：通过 StateReplicaManager 维护 EngineState 的副本
//! - 监控和调试：追踪 Engine 的事件处理流程
//! - UI 更新：为 Web 应用和 UI 提供实时状态更新

use crate::{
    engine::{
        Engine, EngineOutput, UpdateFromAccountOutput, UpdateFromMarketOutput,
        audit::context::EngineContext, clock::EngineClock, error::UnrecoverableEngineError,
    },
    strategy::{on_disconnect::OnDisconnectStrategy, on_trading_disabled::OnTradingDisabled},
};
use barter_integration::{FeedEnded, Terminal, collection::none_one_or_many::NoneOneOrMany};
use derive_more::Constructor;
use serde::{Deserialize, Serialize};

/// 定义表示 `Engine` [`AuditTick`] 生成上下文的数据结构。
pub mod context;

/// 定义可用于维护 `EngineState` 副本的 `StateReplicaManager`。
///
/// 用于支持非热路径交易系统组件，如 UI、Web 应用等。
pub mod state_replica;

/// 定义组件（例如 `Engine`）如何生成 [`AuditTick`] 的接口。
///
/// Auditor trait 定义了生成审计事件的标准接口。实现此 trait 的组件可以生成
/// 包含完整状态快照或特定事件的审计标记。
///
/// ## 关联类型
///
/// - `Snapshot`: 完整状态快照类型
/// - `Context`: `AuditTick` 上下文类型（例如，`Engine` 使用 [`EngineContext`]）
///
/// ## 方法
///
/// - `audit_snapshot()`: 生成包含完整状态快照的审计标记
/// - `audit()`: 从提供的类型生成审计标记
///
/// # 使用示例
///
/// ```rust,ignore
/// // Engine 实现了 Auditor
/// let audit_tick = engine.audit_snapshot();
/// println!("State snapshot at sequence: {}", audit_tick.context.sequence);
///
/// // 生成特定事件的审计标记
/// let event_audit = engine.audit(some_event);
/// ```
pub trait Auditor<AuditKind> {
    /// 完整状态快照类型。
    type Snapshot;

    /// `AuditTick` 上下文类型。
    ///
    /// 例如，`Engine` 使用 [`EngineContext`]。
    type Context;

    /// 构建包含完整状态快照的 `AuditTick`。
    ///
    /// 此方法生成一个包含当前完整状态的审计标记，通常用于初始化状态副本或
    /// 定期状态同步。
    ///
    /// # 返回值
    ///
    /// 返回包含完整状态快照的 `AuditTick`。
    fn audit_snapshot(&mut self) -> AuditTick<Self::Snapshot, Self::Context>;

    /// 从提供的 `Kind` 构建 `AuditTick`。
    ///
    /// 此方法将提供的类型转换为审计事件类型，并生成包含该事件的审计标记。
    ///
    /// # 类型参数
    ///
    /// - `Kind`: 要转换为审计事件的类型
    ///
    /// # 参数
    ///
    /// - `kind`: 要审计的事件或数据
    ///
    /// # 返回值
    ///
    /// 返回包含转换后事件的 `AuditTick`。
    fn audit<Kind>(&mut self, kind: Kind) -> AuditTick<AuditKind, Self::Context>
    where
        AuditKind: From<Kind>;
}

impl<Audit, Clock, State, ExecutionTxs, Strategy, Risk> Auditor<Audit>
    for Engine<Clock, State, ExecutionTxs, Strategy, Risk>
where
    Clock: EngineClock,
    State: Clone,
    Strategy: OnTradingDisabled<Clock, State, ExecutionTxs, Risk>
        + OnDisconnectStrategy<Clock, State, ExecutionTxs, Risk>,
{
    type Snapshot = State;
    type Context = EngineContext;

    /// Engine 生成状态快照的实现。
    ///
    /// 此实现克隆当前状态并生成包含该状态的审计事件。
    fn audit_snapshot(&mut self) -> AuditTick<Self::Snapshot, Self::Context> {
        self.audit(self.state.clone())
    }

    /// Engine 生成审计事件的实现。
    ///
    /// 此实现将提供的值转换为审计事件，并附加当前序列号和时间戳。
    fn audit<Kind>(&mut self, kind: Kind) -> AuditTick<Audit, Self::Context>
    where
        Audit: From<Kind>,
    {
        AuditTick {
            event: Audit::from(kind),
            context: EngineContext {
                sequence: self.meta.sequence.fetch_add(),
                time: self.clock.time(),
            },
        }
    }
}

/// `Engine` 审计事件及其关联上下文。通过 AuditStream 发送。
///
/// AuditTick 是审计系统的基本单元，包含一个事件和生成该事件时的上下文信息。
/// 上下文通常包括序列号和时间戳，用于追踪事件的顺序和时间。
///
/// ## 类型参数
///
/// - `Kind`: 审计事件的类型
/// - `Context`: 上下文类型，默认为 `EngineContext`
///
/// ## 使用场景
///
/// - 通过 AuditStream 发送审计事件
/// - 状态副本同步
/// - 事件追踪和调试
///
/// # 使用示例
///
/// ```rust,ignore
/// let audit_tick = AuditTick {
///     event: some_event,
///     context: EngineContext {
///         sequence: Sequence::new(1),
///         time: Utc::now(),
///     },
/// };
///
/// // 发送到 AuditStream
/// audit_stream.send(audit_tick).await?;
/// ```
#[derive(
    Debug,
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Default,
    Deserialize,
    Serialize,
    Constructor,
)]
pub struct AuditTick<Kind, Context = EngineContext> {
    /// 审计事件。
    pub event: Kind,
    /// 事件生成的上下文（序列号、时间戳等）。
    pub context: Context,
}

/// 表示由 `Engine` 生成并通过 AuditStream 发送的 [`AuditTick`] 类型。
///
/// EngineAudit 枚举包含 Engine 生成的所有审计事件类型。它用于区分不同类型的
/// 审计事件，如输入流结束和处理事件。
///
/// ## 变体
///
/// - **FeedEnded**: 输入事件流已结束
/// - **Process**: Engine 处理了一个事件
///
/// ## 类型参数
///
/// - `Event`: Engine 事件类型
/// - `Output`: Engine 输出类型
///
/// ## 终端事件
///
/// EngineAudit 实现了 `Terminal` trait，用于标识终端事件（如流结束）。
///
/// # 使用示例
///
/// ```rust,ignore
/// // 创建处理事件的审计
/// let audit = EngineAudit::process(some_event);
///
/// // 创建带输出的审计
/// let audit = EngineAudit::process_with_output(event, output);
///
/// // 检查是否为终端事件
/// if audit.is_terminal() {
///     // 处理终端事件
/// }
/// ```
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub enum EngineAudit<Event, Output> {
    /// 输入事件流已结束。
    FeedEnded,
    /// `Engine` 处理了一个 `Event`。
    Process(ProcessAudit<Event, Output>),
}

impl<Event, Output> Terminal for EngineAudit<Event, Output>
where
    Event: Terminal,
{
    /// 检查是否为终端事件。
    ///
    /// 如果事件流已结束，或者处理的事件本身是终端事件，则返回 `true`。
    fn is_terminal(&self) -> bool {
        match self {
            EngineAudit::FeedEnded => true,
            EngineAudit::Process(audit) => audit.is_terminal(),
        }
    }
}

impl<Event, Output> From<FeedEnded> for EngineAudit<Event, Output> {
    /// 从 `FeedEnded` 转换为 `EngineAudit::FeedEnded`。
    fn from(_: FeedEnded) -> Self {
        Self::FeedEnded
    }
}

impl<Event, Output> EngineAudit<Event, Output> {
    /// 创建处理事件的审计。
    ///
    /// 此方法创建一个仅包含事件的 ProcessAudit，不包含输出或错误。
    ///
    /// # 类型参数
    ///
    /// - `E`: 可转换为 `Event` 的类型
    ///
    /// # 参数
    ///
    /// - `event`: 要审计的事件
    ///
    /// # 返回值
    ///
    /// 返回包含事件的 `EngineAudit::Process`。
    pub fn process<E>(event: E) -> Self
    where
        E: Into<Event>,
    {
        Self::Process(ProcessAudit::with_event(event))
    }

    /// 创建带输出的处理事件审计。
    ///
    /// 此方法创建一个包含事件和输出的 ProcessAudit。
    ///
    /// # 类型参数
    ///
    /// - `E`: 可转换为 `Event` 的类型
    /// - `O`: 可转换为 `Output` 的类型
    ///
    /// # 参数
    ///
    /// - `event`: 要审计的事件
    /// - `output`: 事件处理产生的输出
    ///
    /// # 返回值
    ///
    /// 返回包含事件和输出的 `EngineAudit::Process`。
    pub fn process_with_output<E, O>(event: E, output: O) -> Self
    where
        E: Into<Event>,
        O: Into<Output>,
    {
        Self::Process(ProcessAudit::with_output(event, output))
    }

    /// 创建带输出和错误的处理事件审计。
    ///
    /// 此方法创建一个包含事件、输出和不可恢复错误的 ProcessAudit。
    ///
    /// # 类型参数
    ///
    /// - `E`: 可转换为 `Event` 的类型
    /// - `ErrIter`: 不可恢复错误迭代器
    /// - `O`: 可转换为 `Output` 的类型
    ///
    /// # 参数
    ///
    /// - `event`: 要审计的事件
    /// - `unrecoverable`: 不可恢复错误迭代器
    /// - `output`: 事件处理产生的输出
    ///
    /// # 返回值
    ///
    /// 返回包含事件、输出和错误的 `EngineAudit::Process`。
    pub fn process_with_output_and_errs<E, ErrIter, O>(
        event: E,
        unrecoverable: ErrIter,
        output: O,
    ) -> Self
    where
        E: Into<Event>,
        ErrIter: IntoIterator<Item = UnrecoverableEngineError>,
        O: Into<Output>,
    {
        Self::Process(ProcessAudit {
            event: event.into(),
            outputs: NoneOneOrMany::One(output.into()),
            errors: NoneOneOrMany::from_iter(unrecoverable),
        })
    }

    /// 从 ProcessAudit 和错误创建 EngineAudit。
    ///
    /// 此方法将错误添加到现有的 ProcessAudit 中。
    ///
    /// # 类型参数
    ///
    /// - `ErrIter`: 不可恢复错误迭代器
    ///
    /// # 参数
    ///
    /// - `process`: 现有的 ProcessAudit
    /// - `unrecoverable`: 要添加的不可恢复错误迭代器
    ///
    /// # 返回值
    ///
    /// 返回包含添加了错误的 ProcessAudit 的 `EngineAudit::Process`。
    pub fn with_process_and_err<ErrIter>(
        process: ProcessAudit<Event, Output>,
        unrecoverable: ErrIter,
    ) -> Self
    where
        ErrIter: IntoIterator<Item = UnrecoverableEngineError>,
    {
        let process = process.add_errors(unrecoverable);
        Self::Process(process)
    }
}

/// 表示当 `Engine` 处理 `Event` 时生成的 [`AuditTick`] 类型。
///
/// ProcessAudit 包含事件处理的完整信息，包括处理的事件、产生的输出和发生的错误。
/// 它用于追踪 Engine 的事件处理流程和结果。
///
/// ## 字段
///
/// - **event**: 被处理的事件
/// - **outputs**: 事件处理产生的输出（可能为 None、单个或多个）
/// - **errors**: 处理过程中发生的不可恢复错误（可能为 None、单个或多个）
///
/// ## 类型参数
///
/// - `Event`: Engine 事件类型
/// - `Output`: Engine 输出类型
///
/// ## 终端事件
///
/// ProcessAudit 实现了 `Terminal` trait。如果事件本身是终端事件，或者存在
/// 不可恢复错误，则视为终端事件。
///
/// # 使用示例
///
/// ```rust,ignore
/// // 创建仅包含事件的审计
/// let audit = ProcessAudit::with_event(some_event);
///
/// // 创建包含输出的审计
/// let audit = ProcessAudit::with_output(event, output);
///
/// // 添加错误
/// let audit = audit.add_errors(errors);
///
/// // 检查是否为终端事件
/// if audit.is_terminal() {
///     // 处理终端事件
/// }
/// ```
#[derive(
    Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, Constructor,
)]
pub struct ProcessAudit<Event, Output> {
    /// 被处理的事件。
    pub event: Event,
    /// 事件处理产生的输出（None、单个或多个）。
    pub outputs: NoneOneOrMany<Output>,
    /// 处理过程中发生的不可恢复错误（None、单个或多个）。
    pub errors: NoneOneOrMany<UnrecoverableEngineError>,
}

impl<Event, Output> Terminal for ProcessAudit<Event, Output>
where
    Event: Terminal,
{
    /// 检查是否为终端事件。
    ///
    /// 如果事件本身是终端事件，或者存在不可恢复错误，则返回 `true`。
    fn is_terminal(&self) -> bool {
        self.event.is_terminal() || !self.errors.is_empty()
    }
}

impl<Event, Output> ProcessAudit<Event, Output> {
    /// 创建一个只包含事件的 ProcessAudit（无输出和错误）。
    ///
    /// # 类型参数
    ///
    /// - `E`: 事件类型，必须能转换为 `Event`
    ///
    /// # 参数
    ///
    /// - `event`: 要记录的事件
    ///
    /// # 返回值
    ///
    /// 返回只包含事件的 ProcessAudit。
    pub fn with_event<E>(event: E) -> Self
    where
        E: Into<Event>,
    {
        Self {
            event: event.into(),
            outputs: NoneOneOrMany::None,
            errors: NoneOneOrMany::None,
        }
    }

    /// 创建一个包含事件和输出的 ProcessAudit（无错误）。
    ///
    /// # 类型参数
    ///
    /// - `E`: 事件类型，必须能转换为 `Event`
    /// - `O`: 输出类型，必须能转换为 `Output`
    ///
    /// # 参数
    ///
    /// - `event`: 要记录的事件
    /// - `output`: 处理事件的输出
    ///
    /// # 返回值
    ///
    /// 返回包含事件和输出的 ProcessAudit。
    pub fn with_output<E, O>(event: E, output: O) -> Self
    where
        E: Into<Event>,
        O: Into<Output>,
    {
        Self {
            event: event.into(),
            outputs: NoneOneOrMany::One(output.into()),
            errors: NoneOneOrMany::None,
        }
    }
}

impl<Event, OnTradingDisabled, OnDisconnect>
    ProcessAudit<Event, EngineOutput<OnTradingDisabled, OnDisconnect>>
{
    /// 创建一个包含交易状态更新的 ProcessAudit。
    ///
    /// 如果提供了交易禁用信息，则包含在输出中；否则只包含事件。
    ///
    /// # 类型参数
    ///
    /// - `E`: 事件类型，必须能转换为 `Event`
    ///
    /// # 参数
    ///
    /// - `event`: 要记录的事件
    /// - `disabled`: 可选的交易禁用信息
    ///
    /// # 返回值
    ///
    /// 返回包含事件和（如果有）交易状态更新的 ProcessAudit。
    pub fn with_trading_state_update<E>(event: E, disabled: Option<OnTradingDisabled>) -> Self
    where
        E: Into<Event>,
    {
        if let Some(disabled) = disabled {
            Self {
                event: event.into(),
                outputs: NoneOneOrMany::One(EngineOutput::OnTradingDisabled(disabled)),
                errors: NoneOneOrMany::None,
            }
        } else {
            Self::with_event(event)
        }
    }

    /// 创建一个包含账户更新的 ProcessAudit。
    ///
    /// 根据账户更新类型（无更新、断开连接、仓位退出）创建相应的 ProcessAudit。
    ///
    /// # 类型参数
    ///
    /// - `E`: 事件类型，必须能转换为 `Event`
    ///
    /// # 参数
    ///
    /// - `event`: 要记录的事件
    /// - `account`: 账户更新输出
    ///
    /// # 返回值
    ///
    /// 返回包含事件和（如果有）账户更新的 ProcessAudit。
    pub fn with_account_update<E>(event: E, account: UpdateFromAccountOutput<OnDisconnect>) -> Self
    where
        E: Into<Event>,
    {
        match account {
            UpdateFromAccountOutput::None => Self::with_event(event),
            UpdateFromAccountOutput::OnDisconnect(disconnect) => {
                Self::with_output(event, EngineOutput::AccountDisconnect(disconnect))
            }
            UpdateFromAccountOutput::PositionExit(position) => Self::with_output(event, position),
        }
    }

    /// 创建一个包含市场更新的 ProcessAudit。
    ///
    /// 根据市场更新类型（无更新、断开连接）创建相应的 ProcessAudit。
    ///
    /// # 类型参数
    ///
    /// - `E`: 事件类型，必须能转换为 `Event`
    ///
    /// # 参数
    ///
    /// - `event`: 要记录的事件
    /// - `account`: 市场更新输出
    ///
    /// # 返回值
    ///
    /// 返回包含事件和（如果有）市场更新的 ProcessAudit。
    pub fn with_market_update<E>(event: E, account: UpdateFromMarketOutput<OnDisconnect>) -> Self
    where
        E: Into<Event>,
    {
        match account {
            UpdateFromMarketOutput::None => Self::with_event(event),
            UpdateFromMarketOutput::OnDisconnect(disconnect) => {
                Self::with_output(event, EngineOutput::MarketDisconnect(disconnect))
            }
        }
    }
}

impl<Event, Output> ProcessAudit<Event, Output> {
    /// 向 ProcessAudit 添加输出。
    ///
    /// 此方法将新的输出添加到现有的输出列表中。
    ///
    /// # 类型参数
    ///
    /// - `O`: 输出类型，必须能转换为 `Output`
    ///
    /// # 参数
    ///
    /// - `output`: 要添加的输出
    ///
    /// # 返回值
    ///
    /// 返回包含添加了输出的 ProcessAudit。
    pub fn add_output<O>(self, output: O) -> Self
    where
        O: Into<Output>,
    {
        let Self {
            event,
            outputs,
            errors,
        } = self;

        Self {
            event,
            outputs: outputs.extend(NoneOneOrMany::One(output.into())),
            errors,
        }
    }

    /// 向 ProcessAudit 添加错误。
    ///
    /// 此方法将新的错误添加到现有的错误列表中。
    ///
    /// # 类型参数
    ///
    /// - `ErrIter`: 错误迭代器类型
    ///
    /// # 参数
    ///
    /// - `errs`: 要添加的错误迭代器
    ///
    /// # 返回值
    ///
    /// 返回包含添加了错误的 ProcessAudit。
    pub fn add_errors<ErrIter>(self, errs: ErrIter) -> Self
    where
        ErrIter: IntoIterator<Item = UnrecoverableEngineError>,
    {
        let Self {
            event,
            outputs,
            errors,
        } = self;

        Self {
            event,
            outputs,
            errors: errors.extend(errs),
        }
    }
}

impl<Event, Output> From<ProcessAudit<Event, Output>> for EngineAudit<Event, Output> {
    /// 从 `ProcessAudit` 转换为 `EngineAudit::Process`。
    fn from(value: ProcessAudit<Event, Output>) -> Self {
        Self::Process(value)
    }
}
