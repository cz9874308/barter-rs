//! Engine（引擎）模块
//!
//! 本模块定义了 Barter 的核心算法交易引擎 Engine。Engine 是系统的核心组件，负责处理所有交易事件、
//! 维护交易状态、生成算法订单，并与 Strategy 和 RiskManager 协作执行交易策略。
//!
//! # 核心概念
//!
//! - **Engine**: 算法交易引擎，处理事件并维护状态
//! - **EngineState**: Engine 的完整状态，包括持仓、订单、资产等
//! - **EngineEvent**: Engine 处理的事件类型（市场事件、账户事件、命令等）
//! - **Processor**: 事件处理 Trait，定义如何处理事件并生成审计信息
//! - **EngineClock**: 时间接口，用于确定 Engine 的当前时间（支持回测）
//!
//! # 工作原理
//!
//! Engine 采用事件驱动架构：
//!
//! 1. **事件接收**: Engine 接收各种事件（市场数据、账户更新、命令等）
//! 2. **状态更新**: 根据事件更新内部 EngineState
//! 3. **策略执行**: 如果交易状态为 Enabled，调用 Strategy 生成算法订单
//! 4. **风险检查**: 通过 RiskManager 检查生成的订单
//! 5. **订单发送**: 将通过的订单发送到 ExecutionManager
//! 6. **审计记录**: 生成审计信息，记录所有操作
//!
//! # 使用方式
//!
//! 1. 创建 Engine 实例，配置 Clock、State、Strategy、RiskManager
//! 2. 使用 `run` 函数处理事件流
//! 3. Engine 会自动处理事件、更新状态、生成订单
//!
//! # 注意事项
//!
//! - Engine 是线程安全的，可以在多线程环境中使用
//! - Engine 支持同步和异步两种运行模式
//! - 可以通过 Command 从外部控制 Engine（平仓、取消订单等）

use crate::{
    EngineEvent, Sequence,
    engine::{
        action::{
            ActionOutput,
            cancel_orders::CancelOrders,
            close_positions::ClosePositions,
            generate_algo_orders::{GenerateAlgoOrders, GenerateAlgoOrdersOutput},
            send_requests::SendRequests,
        },
        audit::{AuditTick, Auditor, EngineAudit, ProcessAudit, context::EngineContext},
        clock::EngineClock,
        command::Command,
        execution_tx::ExecutionTxMap,
        state::{
            EngineState, instrument::data::InstrumentDataState,
            order::in_flight_recorder::InFlightRequestRecorder, position::PositionExited,
            trading::TradingState,
        },
    },
    execution::{AccountStreamEvent, request::ExecutionRequest},
    risk::RiskManager,
    shutdown::SyncShutdown,
    statistic::summary::TradingSummaryGenerator,
    strategy::{
        algo::AlgoStrategy, close_positions::ClosePositionsStrategy,
        on_disconnect::OnDisconnectStrategy, on_trading_disabled::OnTradingDisabled,
    },
};
use barter_data::{event::MarketEvent, streams::consumer::MarketStreamEvent};
use barter_execution::AccountEvent;
use barter_instrument::{asset::QuoteAsset, exchange::ExchangeIndex, instrument::InstrumentIndex};
use barter_integration::channel::Tx;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use tracing::info;

/// 定义 Engine 如何处理 Command（命令）以及相关的输出。
pub mod action;

/// 定义 Engine 的审计类型以及处理 Engine AuditStream 的工具。
///
/// 例如：`StateReplicaManager` 组件可用于维护 EngineState 的副本。
pub mod audit;

/// 定义用于确定当前 Engine 时间的 [`EngineClock`] 接口。
///
/// 这种灵活性使得回测运行可以使用近似正确的历史时间戳。
pub mod clock;

/// 定义 Engine 的 [`Command`] - 用于从外部进程向 Engine 提供交易指令（例如 ClosePositions）。
pub mod command;

/// 定义 Engine 中可能发生的所有错误。
pub mod error;

/// 定义 [`ExecutionTxMap`] 接口，该接口建模用于将 ExecutionRequest 路由到相应 ExecutionManager 的发送器集合。
pub mod execution_tx;

/// 定义 Engine 用于算法交易的所有状态。
///
/// 例如：`ConnectivityStates`、`AssetStates`、`InstrumentStates`、`Position` 等。
pub mod state;

/// Engine 运行器，用于处理输入事件。
///
/// 例如：`fn sync_run`、`fn sync_run_with_audit`、`fn async_run`、`fn async_run_with_audit`
pub mod run;

/// 定义组件如何处理输入事件并生成相应的审计信息。
///
/// Processor 是一个通用 Trait，用于处理事件并生成审计信息。Engine 实现了此 Trait，
/// 可以处理 EngineEvent 并生成 EngineAudit。
///
/// # 类型参数
///
/// - `Event`: 要处理的事件类型
///
/// # 关联类型
///
/// - `Audit`: 处理事件后生成的审计信息类型
///
/// # 实现要求
///
/// 实现者必须：
/// - 定义 `Audit` 关联类型
/// - 实现 `process` 方法，处理事件并返回审计信息
///
/// # 使用示例
///
/// ```rust,ignore
/// // Engine 实现了 Processor<EngineEvent>
/// let mut engine = Engine::new(...);
/// let audit = engine.process(market_event);
/// ```
pub trait Processor<Event> {
    /// 处理事件后生成的审计信息类型
    type Audit;
    /// 处理输入事件并生成审计信息。
    ///
    /// # 参数
    ///
    /// - `event`: 要处理的事件
    ///
    /// # 返回值
    ///
    /// 返回处理事件后生成的审计信息。
    fn process(&mut self, event: Event) -> Self::Audit;
}

/// 使用 Engine 处理事件并生成 [`AuditTick`] 记录完成的工作。
///
/// 这是一个便捷函数，将事件处理分为两个步骤：
/// 1. 处理事件并生成审计输出
/// 2. 将审计输出转换为 AuditTick
///
/// # 类型参数
///
/// - `Event`: 要处理的事件类型
/// - `Engine`: 实现了 Processor 和 Auditor 的 Engine 类型
///
/// # 参数
///
/// - `engine`: 要使用的 Engine 实例
/// - `event`: 要处理的事件
///
/// # 返回值
///
/// 返回包含处理结果和上下文的 AuditTick。
///
/// # 使用示例
///
/// ```rust,ignore
/// let mut engine = Engine::new(...);
/// let audit_tick = process_with_audit(&mut engine, market_event);
/// ```
pub fn process_with_audit<Event, Engine>(
    engine: &mut Engine,
    event: Event,
) -> AuditTick<Engine::Audit, EngineContext>
where
    Engine: Processor<Event> + Auditor<Engine::Audit, Context = EngineContext>,
{
    let output = engine.process(event);
    engine.audit(output)
}

/// 算法交易 Engine（引擎）。
///
/// Engine 是 Barter 的核心组件，负责处理所有交易相关的逻辑。它就像一个"交易大脑"，
/// 接收各种事件，维护交易状态，并根据策略生成交易订单。
///
/// ## Engine 的主要功能
///
/// - **事件处理**: 处理输入的 [`EngineEvent`]（或自定义事件，如果已实现）
/// - **状态管理**: 维护内部的 [`EngineState`]（工具数据状态、未完成订单、持仓等）
/// - **订单生成**: 如果 `TradingState::Enabled`，根据 Strategy 生成算法订单
/// - **风险检查**: 通过 RiskManager 检查生成的订单
/// - **订单执行**: 将订单发送到 ExecutionManager 执行
///
/// ## 工作原理
///
/// Engine 采用事件驱动架构：
///
/// 1. **接收事件**: 从外部接收 EngineEvent（市场数据、账户更新、命令等）
/// 2. **更新状态**: 根据事件更新内部 EngineState
/// 3. **执行策略**: 如果交易启用，调用 Strategy 生成订单
/// 4. **风险检查**: 通过 RiskManager 过滤订单
/// 5. **发送订单**: 将订单发送到交易所执行
/// 6. **生成审计**: 记录所有操作，生成审计信息
///
/// ## 类型参数
///
/// - `Clock`: [`EngineClock`] 实现，用于确定当前时间（支持回测）
/// - `State`: Engine 状态实现（例如 [`EngineState`]）
/// - `ExecutionTxs`: [`ExecutionTxMap`] 实现，用于发送执行请求
/// - `Strategy`: 交易策略实现（参见 [`super::strategy`]）
/// - `Risk`: [`RiskManager`] 实现，用于风险管理
///
/// ## 字段
///
/// - `clock`: 时间接口，用于获取当前时间
/// - `meta`: Engine 元数据（启动时间、事件序列号等）
/// - `state`: Engine 的完整状态
/// - `execution_txs`: 执行请求发送器映射
/// - `strategy`: 交易策略
/// - `risk`: 风险管理器
///
/// ## 使用示例
///
/// ```rust,ignore
/// // 创建 Engine
/// let engine = Engine::new(
///     clock,
///     engine_state,
///     execution_txs,
///     strategy,
///     risk_manager,
/// );
///
/// // 处理事件
/// let audit = engine.process(market_event);
/// ```
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Engine<Clock, State, ExecutionTxs, Strategy, Risk> {
    /// 时间接口，用于获取当前时间
    pub clock: Clock,
    /// Engine 元数据（启动时间、事件序列号等）
    pub meta: EngineMeta,
    /// Engine 的完整状态
    pub state: State,
    /// 执行请求发送器映射
    pub execution_txs: ExecutionTxs,
    /// 交易策略
    pub strategy: Strategy,
    /// 风险管理器
    pub risk: Risk,
}

/// 运行中的 [`Engine`] 元数据。
///
/// EngineMeta 存储 Engine 运行时的元信息，包括启动时间和事件序列号。
/// 这些信息用于性能统计、审计追踪和交易摘要生成。
///
/// # 字段
///
/// - `time_start`: Engine 当前运行周期的启动时间戳（UTC）
/// - `sequence`: 单调递增的事件序列号，用于跟踪已处理的事件数量
///
/// # 使用场景
///
/// - 计算交易会话的持续时间
/// - 生成交易摘要（TradingSummary）
/// - 事件排序和去重
/// - 性能分析和调试
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub struct EngineMeta {
    /// Engine 当前运行周期的启动时间戳（UTC）
    pub time_start: DateTime<Utc>,
    /// 单调递增的事件序列号，关联已处理的事件数量
    pub sequence: Sequence,
}

impl<Clock, GlobalData, InstrumentData, ExecutionTxs, Strategy, Risk>
    Processor<EngineEvent<InstrumentData::MarketEventKind>>
    for Engine<Clock, EngineState<GlobalData, InstrumentData>, ExecutionTxs, Strategy, Risk>
where
    Clock: EngineClock + for<'a> Processor<&'a EngineEvent<InstrumentData::MarketEventKind>>,
    InstrumentData: InstrumentDataState,
    GlobalData: for<'a> Processor<&'a AccountEvent>
        + for<'a> Processor<&'a MarketEvent<InstrumentIndex, InstrumentData::MarketEventKind>>,
    ExecutionTxs: ExecutionTxMap<ExchangeIndex, InstrumentIndex>,
    Strategy: OnTradingDisabled<Clock, EngineState<GlobalData, InstrumentData>, ExecutionTxs, Risk>
        + OnDisconnectStrategy<Clock, EngineState<GlobalData, InstrumentData>, ExecutionTxs, Risk>
        + AlgoStrategy<State = EngineState<GlobalData, InstrumentData>>
        + ClosePositionsStrategy<State = EngineState<GlobalData, InstrumentData>>,
    Risk: RiskManager<State = EngineState<GlobalData, InstrumentData>>,
{
    type Audit = EngineAudit<
        EngineEvent<InstrumentData::MarketEventKind>,
        EngineOutput<Strategy::OnTradingDisabled, Strategy::OnDisconnect>,
    >;

    fn process(&mut self, event: EngineEvent<InstrumentData::MarketEventKind>) -> Self::Audit {
        // 更新时钟时间（某些事件可能影响时间，如回测中的历史事件）
        self.clock.process(&event);

        // 根据事件类型处理事件并生成审计信息
        let process_audit = match &event {
            // 关闭事件：直接返回，不进行后续处理
            EngineEvent::Shutdown(_) => return EngineAudit::process(event),
            // 命令事件：执行命令（平仓、取消订单等）
            EngineEvent::Command(command) => {
                let output = self.action(command);

                // 如果命令执行产生不可恢复的错误，立即返回错误审计
                if let Some(unrecoverable) = output.unrecoverable_errors() {
                    return EngineAudit::process_with_output_and_errs(event, unrecoverable, output);
                } else {
                    ProcessAudit::with_output(event, output)
                }
            }
            // 交易状态更新：更新交易状态（开启/关闭算法交易）
            EngineEvent::TradingStateUpdate(trading_state) => {
                let trading_disabled = self.update_from_trading_state_update(*trading_state);
                ProcessAudit::with_trading_state_update(event, trading_disabled)
            }
            // 账户事件：更新账户状态（余额、订单、持仓等）
            EngineEvent::Account(account) => {
                let output = self.update_from_account_stream(account);
                ProcessAudit::with_account_update(event, output)
            }
            // 市场事件：更新市场数据（价格、订单簿等）
            EngineEvent::Market(market) => {
                let output = self.update_from_market_stream(market);
                ProcessAudit::with_market_update(event, output)
            }
        };

        // 如果交易状态为 Enabled，生成算法订单
        if let TradingState::Enabled = self.state.trading {
            let output = self.generate_algo_orders();

            // 根据订单生成结果构造最终审计
            if output.is_empty() {
                // 没有生成订单，直接返回处理审计
                EngineAudit::from(process_audit)
            } else if let Some(unrecoverable) = output.unrecoverable_errors() {
                // 有不可恢复的错误，添加错误到审计
                EngineAudit::Process(process_audit.add_errors(unrecoverable))
            } else {
                // 正常生成订单，添加输出到审计
                EngineAudit::from(process_audit.add_output(output))
            }
        } else {
            // 交易状态为 Disabled，不生成订单，直接返回处理审计
            EngineAudit::from(process_audit)
        }
    }
}

impl<Clock, GlobalData, InstrumentData, ExecutionTxs, Strategy, Risk> SyncShutdown
    for Engine<Clock, EngineState<GlobalData, InstrumentData>, ExecutionTxs, Strategy, Risk>
where
    ExecutionTxs: ExecutionTxMap,
{
    type Result = ();

    /// 优雅地关闭 Engine。
    ///
    /// 向所有 ExecutionManager 发送关闭请求，确保所有执行任务都能正常关闭。
    ///
    /// # 工作原理
    ///
    /// 遍历所有执行发送器，向每个 ExecutionManager 发送 Shutdown 请求。
    /// 这确保了所有执行任务都能收到关闭信号并正常退出。
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// engine.shutdown();
    /// ```
    fn shutdown(&mut self) -> Self::Result {
        // 向所有 ExecutionManager 发送关闭请求
        self.execution_txs.iter().for_each(|execution_tx| {
            let _send_result = execution_tx.send(ExecutionRequest::Shutdown);
        });
    }
}

impl<Clock, GlobalData, InstrumentData, ExecutionTxs, Strategy, Risk>
    Engine<Clock, EngineState<GlobalData, InstrumentData>, ExecutionTxs, Strategy, Risk>
{
    /// 执行 Engine 的 [`Command`]，生成 [`ActionOutput`] 记录完成的工作。
    ///
    /// 此方法处理从外部进程发送的命令，如平仓、取消订单等。命令执行后会更新 EngineState
    /// 并记录在途请求。
    ///
    /// # 参数
    ///
    /// - `command`: 要执行的命令
    ///
    /// # 返回值
    ///
    /// 返回 `ActionOutput`，包含命令执行的结果和相关信息。
    ///
    /// # 支持的命令
    ///
    /// - `SendCancelRequests`: 发送取消订单请求
    /// - `SendOpenRequests`: 发送开仓订单请求
    /// - `ClosePositions`: 平仓命令
    /// - `CancelOrders`: 取消订单命令
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let command = Command::ClosePositions(InstrumentFilter::None);
    /// let output = engine.action(&command);
    /// ```
    pub fn action(&mut self, command: &Command) -> ActionOutput
    where
        InstrumentData: InFlightRequestRecorder,
        ExecutionTxs: ExecutionTxMap,
        Strategy: ClosePositionsStrategy<State = EngineState<GlobalData, InstrumentData>>,
        Risk: RiskManager,
    {
        match &command {
            Command::SendCancelRequests(requests) => {
                info!(
                    ?requests,
                    "Engine actioning user Command::SendCancelRequests"
                );
                let output = self.send_requests(requests.clone());
                self.state.record_in_flight_cancels(&output.sent);
                ActionOutput::CancelOrders(output)
            }
            Command::SendOpenRequests(requests) => {
                info!(?requests, "Engine actioning user Command::SendOpenRequests");
                let output = self.send_requests(requests.clone());
                self.state.record_in_flight_opens(&output.sent);
                ActionOutput::OpenOrders(output)
            }
            Command::ClosePositions(filter) => {
                info!(?filter, "Engine actioning user Command::ClosePositions");
                ActionOutput::ClosePositions(self.close_positions(filter))
            }
            Command::CancelOrders(filter) => {
                info!(?filter, "Engine actioning user Command::CancelOrders");
                ActionOutput::CancelOrders(self.cancel_orders(filter))
            }
        }
    }

    /// 更新 Engine 的 [`TradingState`]（交易状态）。
    ///
    /// 当交易状态更新时，Engine 会更新内部状态。如果状态转换到 `TradingState::Disabled`，
    /// Engine 会调用配置的 [`OnTradingDisabled`] 策略逻辑。
    ///
    /// # 参数
    ///
    /// - `update`: 新的交易状态
    ///
    /// # 返回值
    ///
    /// 如果状态转换到 Disabled，返回 `OnTradingDisabled` 策略的输出；否则返回 `None`。
    ///
    /// # 使用场景
    ///
    /// - 从外部进程开启/关闭算法交易
    /// - 在风险控制时自动禁用交易
    /// - 在系统维护时暂停交易
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let output = engine.update_from_trading_state_update(TradingState::Disabled);
    /// if let Some(on_disabled_output) = output {
    ///     // 处理交易禁用时的策略输出
    /// }
    /// ```
    pub fn update_from_trading_state_update(
        &mut self,
        update: TradingState,
    ) -> Option<Strategy::OnTradingDisabled>
    where
        Strategy:
            OnTradingDisabled<Clock, EngineState<GlobalData, InstrumentData>, ExecutionTxs, Risk>,
    {
        self.state
            .trading
            .update(update)
            .transitioned_to_disabled()
            .then(|| Strategy::on_trading_disabled(self))
    }

    /// 从 [`AccountStreamEvent`] 更新 Engine。
    ///
    /// 当接收到账户流事件时，Engine 会更新内部状态（余额、订单状态、持仓等）。
    /// 如果事件指示交易所执行连接已断开，Engine 会调用配置的 [`OnDisconnectStrategy`] 策略逻辑。
    ///
    /// # 参数
    ///
    /// - `event`: 账户流事件
    ///
    /// # 返回值
    ///
    /// 返回 `UpdateFromAccountOutput`，可能包含：
    /// - `OnDisconnect`: 如果连接断开，包含断开策略的输出
    /// - `PositionExit`: 如果持仓已平仓，包含平仓信息
    /// - `None`: 正常更新，无特殊输出
    ///
    /// # 使用场景
    ///
    /// - 处理账户余额更新
    /// - 处理订单状态更新
    /// - 处理交易执行确认
    /// - 处理连接断开事件
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let output = engine.update_from_account_stream(&account_event);
    /// match output {
    ///     UpdateFromAccountOutput::OnDisconnect(output) => {
    ///         // 处理断开连接
    ///     }
    ///     UpdateFromAccountOutput::PositionExit(exit) => {
    ///         // 处理持仓平仓
    ///     }
    ///     UpdateFromAccountOutput::None => {
    ///         // 正常更新
    ///     }
    /// }
    /// ```
    pub fn update_from_account_stream(
        &mut self,
        event: &AccountStreamEvent,
    ) -> UpdateFromAccountOutput<Strategy::OnDisconnect>
    where
        InstrumentData: for<'a> Processor<&'a AccountEvent>,
        GlobalData: for<'a> Processor<&'a AccountEvent>,
        Strategy: OnDisconnectStrategy<Clock, EngineState<GlobalData, InstrumentData>, ExecutionTxs, Risk>,
    {
        match event {
            AccountStreamEvent::Reconnecting(exchange) => {
                self.state
                    .connectivity
                    .update_from_account_reconnecting(exchange);

                UpdateFromAccountOutput::OnDisconnect(Strategy::on_disconnect(self, *exchange))
            }
            AccountStreamEvent::Item(event) => self
                .state
                .update_from_account(event)
                .map(UpdateFromAccountOutput::PositionExit)
                .unwrap_or(UpdateFromAccountOutput::None),
        }
    }

    /// 从 [`MarketStreamEvent`] 更新 Engine。
    ///
    /// 当接收到市场流事件时，Engine 会更新内部市场数据状态（价格、订单簿等）。
    /// 如果事件指示交易所市场数据连接已断开，Engine 会调用配置的 [`OnDisconnectStrategy`] 策略逻辑。
    ///
    /// # 参数
    ///
    /// - `event`: 市场流事件
    ///
    /// # 返回值
    ///
    /// 返回 `UpdateFromMarketOutput`，可能包含：
    /// - `OnDisconnect`: 如果连接断开，包含断开策略的输出
    /// - `None`: 正常更新，无特殊输出
    ///
    /// # 使用场景
    ///
    /// - 处理实时价格更新
    /// - 处理订单簿更新
    /// - 处理逐笔交易数据
    /// - 处理连接断开事件
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let output = engine.update_from_market_stream(&market_event);
    /// match output {
    ///     UpdateFromMarketOutput::OnDisconnect(output) => {
    ///         // 处理断开连接
    ///     }
    ///     UpdateFromMarketOutput::None => {
    ///         // 正常更新市场数据
    ///     }
    /// }
    /// ```
    pub fn update_from_market_stream(
        &mut self,
        event: &MarketStreamEvent<InstrumentIndex, InstrumentData::MarketEventKind>,
    ) -> UpdateFromMarketOutput<Strategy::OnDisconnect>
    where
        InstrumentData: InstrumentDataState,
        GlobalData:
            for<'a> Processor<&'a MarketEvent<InstrumentIndex, InstrumentData::MarketEventKind>>,
        Strategy: OnDisconnectStrategy<Clock, EngineState<GlobalData, InstrumentData>, ExecutionTxs, Risk>,
    {
        match event {
            MarketStreamEvent::Reconnecting(exchange) => {
                self.state
                    .connectivity
                    .update_from_market_reconnecting(exchange);

                UpdateFromMarketOutput::OnDisconnect(Strategy::on_disconnect(self, *exchange))
            }
            MarketStreamEvent::Item(event) => {
                self.state.update_from_market(event);
                UpdateFromMarketOutput::None
            }
        }
    }

    /// 返回当前交易会话的 [`TradingSummaryGenerator`]（交易摘要生成器）。
    ///
    /// TradingSummaryGenerator 用于生成交易性能摘要，包括盈亏、夏普比率、回撤等指标。
    ///
    /// # 参数
    ///
    /// - `risk_free_return`: 无风险收益率（用于计算风险调整后收益指标，如夏普比率）
    ///
    /// # 返回值
    ///
    /// 返回一个初始化的 TradingSummaryGenerator，可用于生成交易摘要。
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let generator = engine.trading_summary_generator(dec!(0.05)); // 5% 无风险收益率
    /// let summary = generator.generate(Daily);
    /// summary.print_summary();
    /// ```
    pub fn trading_summary_generator(&self, risk_free_return: Decimal) -> TradingSummaryGenerator
    where
        Clock: EngineClock,
    {
        TradingSummaryGenerator::init(
            risk_free_return,
            self.meta.time_start,
            self.time(),
            &self.state.instruments,
            &self.state.assets,
        )
    }
}

impl<Clock, State, ExecutionTxs, Strategy, Risk> Engine<Clock, State, ExecutionTxs, Strategy, Risk>
where
    Clock: EngineClock,
{
    /// 构造一个新的 Engine。
    ///
    /// 使用提供的组件创建 Engine 实例。初始的 [`EngineMeta`] 从提供的 `clock` 和 `Sequence(0)` 构造。
    ///
    /// # 参数
    ///
    /// - `clock`: EngineClock 实现，用于获取当前时间
    /// - `state`: Engine 状态实现
    /// - `execution_txs`: 执行请求发送器映射
    /// - `strategy`: 交易策略实现
    /// - `risk`: 风险管理器实现
    ///
    /// # 返回值
    ///
    /// 返回新创建的 Engine 实例。
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let engine = Engine::new(
    ///     LiveClock,
    ///     engine_state,
    ///     execution_txs,
    ///     my_strategy,
    ///     my_risk_manager,
    /// );
    /// ```
    pub fn new(
        clock: Clock,
        state: State,
        execution_txs: ExecutionTxs,
        strategy: Strategy,
        risk: Risk,
    ) -> Self {
        Self {
            meta: EngineMeta {
                time_start: clock.time(),
                sequence: Sequence(0),
            },
            clock,
            state,
            execution_txs,
            strategy,
            risk,
        }
    }

    /// 返回 Engine 的时钟时间。
    ///
    /// # 返回值
    ///
    /// 返回 Engine 的当前时间（UTC）。
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let current_time = engine.time();
    /// ```
    pub fn time(&self) -> DateTime<Utc> {
        self.clock.time()
    }

    /// 重置内部 `EngineMeta` 为时钟时间和 `Sequence(0)`。
    ///
    /// 用于开始新的交易会话或重置统计信息。
    ///
    /// # 使用场景
    ///
    /// - 开始新的交易会话
    /// - 重置性能统计
    /// - 回测时重置状态
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// engine.reset_metadata();
    /// ```
    pub fn reset_metadata(&mut self) {
        self.meta.time_start = self.clock.time();
        self.meta.sequence = Sequence(0);
    }
}

/// Engine 操作产生的输出，用于构造 Engine 的 [`EngineAudit`]。
///
/// EngineOutput 枚举了 Engine 可能产生的所有输出类型，这些输出会被转换为 EngineAudit
/// 用于审计和监控。
///
/// # 类型参数
///
/// - `OnTradingDisabled`: 交易禁用策略的输出类型
/// - `OnDisconnect`: 断开连接策略的输出类型
/// - `ExchangeKey`: 交易所标识类型（默认为 `ExchangeIndex`）
/// - `InstrumentKey`: 交易工具标识类型（默认为 `InstrumentIndex`）
///
/// # 变体
///
/// - `Commanded`: 命令执行输出（平仓、取消订单等）
/// - `OnTradingDisabled`: 交易禁用时的策略输出
/// - `AccountDisconnect`: 账户连接断开时的策略输出
/// - `PositionExit`: 持仓平仓输出
/// - `MarketDisconnect`: 市场数据连接断开时的策略输出
/// - `AlgoOrders`: 算法订单生成输出
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub enum EngineOutput<
    OnTradingDisabled,
    OnDisconnect,
    ExchangeKey = ExchangeIndex,
    InstrumentKey = InstrumentIndex,
> {
    /// 命令执行输出（平仓、取消订单等）
    Commanded(ActionOutput<ExchangeKey, InstrumentKey>),
    /// 交易禁用时的策略输出
    OnTradingDisabled(OnTradingDisabled),
    /// 账户连接断开时的策略输出
    AccountDisconnect(OnDisconnect),
    /// 持仓平仓输出
    PositionExit(PositionExited<QuoteAsset, InstrumentKey>),
    /// 市场数据连接断开时的策略输出
    MarketDisconnect(OnDisconnect),
    /// 算法订单生成输出
    AlgoOrders(GenerateAlgoOrdersOutput<ExchangeKey, InstrumentKey>),
}

/// Engine 从 [`TradingState`] 更新时产生的输出，用于构造 Engine 的 [`EngineAudit`]。
///
/// # 类型参数
///
/// - `OnTradingDisabled`: 交易禁用策略的输出类型
///
/// # 变体
///
/// - `None`: 无特殊输出（状态未转换到 Disabled）
/// - `OnTradingDisabled`: 交易禁用时的策略输出
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub enum UpdateTradingStateOutput<OnTradingDisabled> {
    /// 无特殊输出
    None,
    /// 交易禁用时的策略输出
    OnTradingDisabled(OnTradingDisabled),
}

/// Engine 从 [`AccountStreamEvent`] 更新时产生的输出，用于构造 Engine 的 [`EngineAudit`]。
///
/// # 类型参数
///
/// - `OnDisconnect`: 断开连接策略的输出类型
/// - `InstrumentKey`: 交易工具标识类型（默认为 `InstrumentIndex`）
///
/// # 变体
///
/// - `None`: 无特殊输出（正常更新）
/// - `OnDisconnect`: 账户连接断开时的策略输出
/// - `PositionExit`: 持仓平仓输出
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub enum UpdateFromAccountOutput<OnDisconnect, InstrumentKey = InstrumentIndex> {
    /// 无特殊输出
    None,
    /// 账户连接断开时的策略输出
    OnDisconnect(OnDisconnect),
    /// 持仓平仓输出
    PositionExit(PositionExited<QuoteAsset, InstrumentKey>),
}

/// Engine 从 [`MarketStreamEvent`] 更新时产生的输出，用于构造 Engine 的 [`EngineAudit`]。
///
/// # 类型参数
///
/// - `OnDisconnect`: 断开连接策略的输出类型
///
/// # 变体
///
/// - `None`: 无特殊输出（正常更新）
/// - `OnDisconnect`: 市场数据连接断开时的策略输出
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub enum UpdateFromMarketOutput<OnDisconnect> {
    /// 无特殊输出
    None,
    /// 市场数据连接断开时的策略输出
    OnDisconnect(OnDisconnect),
}

impl<OnTradingDisabled, OnDisconnect, ExchangeKey, InstrumentKey>
    From<ActionOutput<ExchangeKey, InstrumentKey>>
    for EngineOutput<OnTradingDisabled, OnDisconnect, ExchangeKey, InstrumentKey>
{
    fn from(value: ActionOutput<ExchangeKey, InstrumentKey>) -> Self {
        Self::Commanded(value)
    }
}

impl<OnTradingDisabled, OnDisconnect, ExchangeKey, InstrumentKey>
    From<PositionExited<QuoteAsset, InstrumentKey>>
    for EngineOutput<OnTradingDisabled, OnDisconnect, ExchangeKey, InstrumentKey>
{
    fn from(value: PositionExited<QuoteAsset, InstrumentKey>) -> Self {
        Self::PositionExit(value)
    }
}

impl<OnTradingDisabled, OnDisconnect, ExchangeKey, InstrumentKey>
    From<GenerateAlgoOrdersOutput<ExchangeKey, InstrumentKey>>
    for EngineOutput<OnTradingDisabled, OnDisconnect, ExchangeKey, InstrumentKey>
{
    fn from(value: GenerateAlgoOrdersOutput<ExchangeKey, InstrumentKey>) -> Self {
        Self::AlgoOrders(value)
    }
}
