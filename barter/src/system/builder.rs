//! SystemBuilder 系统构建器模块
//!
//! 本模块提供了用于构建完整 Barter 交易系统的构建器。
//! SystemBuilder 允许用户配置 Engine、执行组件、市场数据流等，然后构建和初始化系统。
//!
//! # 核心概念
//!
//! - **SystemBuilder**: 系统构建器，用于配置和构建系统
//! - **SystemArgs**: 构建系统所需的参数
//! - **SystemBuild**: 已构建但未初始化的系统
//! - **EngineFeedMode**: Engine 事件处理模式（Iterator 或 Stream）
//! - **AuditMode**: 审计模式（启用或禁用）

use crate::{
    engine::{
        Engine, Processor,
        audit::{Auditor, context::EngineContext},
        clock::EngineClock,
        execution_tx::MultiExchangeTxMap,
        run::{async_run, async_run_with_audit, sync_run, sync_run_with_audit},
        state::{EngineState, builder::EngineStateBuilder, trading::TradingState},
    },
    error::BarterError,
    execution::{
        AccountStreamEvent,
        builder::{ExecutionBuildFutures, ExecutionBuilder},
    },
    shutdown::SyncShutdown,
    system::{System, SystemAuxillaryHandles, config::ExecutionConfig},
};
use barter_data::streams::reconnect::stream::ReconnectingStream;
use barter_execution::balance::Balance;
use barter_instrument::{
    Keyed,
    asset::{AssetIndex, ExchangeAsset, name::AssetNameInternal},
    exchange::{ExchangeId, ExchangeIndex},
    index::IndexedInstruments,
    instrument::{Instrument, InstrumentIndex},
};
use barter_integration::{
    FeedEnded, Terminal,
    channel::{Channel, ChannelTxDroppable, mpsc_unbounded},
    snapshot::SnapUpdates,
};
use derive_more::Constructor;
use fnv::FnvHashMap;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, marker::PhantomData};

/// 定义 `Engine` 如何处理输入事件。
///
/// 使用此枚举控制 `Engine` 是在阻塞线程中使用 `Iterator` 同步运行，
/// 还是使用 `Stream` 和 tokio 任务异步运行。
///
/// ## 模式说明
///
/// - **Iterator**: 同步模式，在阻塞线程中处理事件（默认）
/// - **Stream**: 异步模式，使用 tokio 任务处理事件
///
/// ## 使用场景
///
/// - **Iterator**: 单线程回测、简单场景
/// - **Stream**: 大规模并发回测、需要异步处理的场景
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, Default)]
pub enum EngineFeedMode {
    /// 在阻塞线程中使用 `Iterator` 同步处理事件（默认）。
    #[default]
    Iterator,

    /// 使用 `Stream` 和 tokio 任务异步处理事件。
    ///
    /// 在运行大规模并发回测时很有用。
    Stream,
}

/// 定义 `Engine` 是否在审计通道上发送其生成的审计事件。
///
/// AuditMode 控制是否启用审计事件流。启用后，可以通过 `System::take_audit()` 获取审计流。
///
/// ## 模式说明
///
/// - **Enabled**: 启用审计事件发送
/// - **Disabled**: 禁用审计事件发送（默认）
///
/// ## 使用场景
///
/// - **Enabled**: 需要审计流用于 UI、监控、调试等
/// - **Disabled**: 不需要审计流，节省资源（默认）
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, Default)]
pub enum AuditMode {
    /// 启用审计事件发送。
    Enabled,

    /// 禁用审计事件发送（默认）。
    #[default]
    Disabled,
}

/// 构建完整 Barter 交易系统所需的参数。
///
/// SystemArgs 包含构建和初始化完整 Barter 交易系统所需的所有组件，
/// 包括 `Engine` 和所有支持基础设施。
///
/// ## 类型参数
///
/// - `Clock`: Engine 时钟类型
/// - `Strategy`: 策略类型
/// - `Risk`: 风险管理器类型
/// - `MarketStream`: 市场数据流类型
/// - `GlobalData`: 全局数据类型
/// - `FnInstrumentData`: 交易对数据初始化函数类型
///
/// ## 字段说明
///
/// - **instruments**: 系统将跟踪的索引化交易对集合
/// - **executions**: 交易所执行链接的配置
/// - **clock**: 时间保持的 `EngineClock` 实现（例如，回测用 `HistoricalClock`，实盘/模拟用 `LiveClock`）
/// - **strategy**: Engine `Strategy` 实现
/// - **risk**: Engine `RiskManager` 实现
/// - **market_stream**: `MarketStreamEvent` 流
/// - **global_data**: `EngineState` 的 `GlobalData`
/// - **instrument_data_init**: 构建 `EngineState` 时用于初始化每个交易对的 `InstrumentDataState` 的闭包
#[derive(Debug, Clone, PartialEq, PartialOrd, Constructor)]
pub struct SystemArgs<'a, Clock, Strategy, Risk, MarketStream, GlobalData, FnInstrumentData> {
    /// 系统将跟踪的索引化交易对集合。
    pub instruments: &'a IndexedInstruments,

    /// 交易所执行链接的配置。
    pub executions: Vec<ExecutionConfig>,

    /// 用于时间保持的 `EngineClock` 实现。
    ///
    /// 例如，回测使用 `HistoricalClock`，实盘/模拟交易使用 `LiveClock`。
    pub clock: Clock,

    /// Engine `Strategy` 实现。
    pub strategy: Strategy,

    /// Engine `RiskManager` 实现。
    pub risk: Risk,

    /// `MarketStreamEvent` 流。
    pub market_stream: MarketStream,

    /// `EngineState` 的 `GlobalData`。
    pub global_data: GlobalData,

    /// 构建 `EngineState` 时用于初始化每个交易对的 `InstrumentDataState` 的闭包。
    pub instrument_data_init: FnInstrumentData,
}

/// 用于构建完整 Barter 交易系统的构建器。
///
/// SystemBuilder 提供了流畅的 API 来配置和构建交易系统。它支持链式调用，
/// 允许逐步配置系统的各个组件。
///
/// ## 类型参数
///
/// - `Clock`: Engine 时钟类型
/// - `Strategy`: 策略类型
/// - `Risk`: 风险管理器类型
/// - `MarketStream`: 市场数据流类型
/// - `GlobalData`: 全局数据类型
/// - `FnInstrumentData`: 交易对数据初始化函数类型
///
/// ## 使用流程
///
/// 1. 创建 SystemBuilder（使用 SystemArgs）
/// 2. 可选配置（engine_feed_mode、audit_mode、trading_state、balances）
/// 3. 调用 `build()` 构建系统
/// 4. 调用 `init()` 初始化系统
///
/// # 使用示例
///
/// ```rust,ignore
/// let builder = SystemBuilder::new(SystemArgs {
///     instruments: &instruments,
///     executions: vec![...],
///     clock: HistoricalClock::new(...),
///     strategy: MyStrategy::new(),
///     risk: MyRiskManager::new(),
///     market_stream: market_stream,
///     global_data: DefaultGlobalData,
///     instrument_data_init: |_| DefaultInstrumentData,
/// })
/// .engine_feed_mode(EngineFeedMode::Iterator)
/// .audit_mode(AuditMode::Enabled)
/// .trading_state(TradingState::Enabled);
///
/// let system = builder.build()?.init().await?;
/// ```
#[derive(Debug)]
pub struct SystemBuilder<'a, Clock, Strategy, Risk, MarketStream, GlobalData, FnInstrumentData> {
    /// 系统构建参数。
    args: SystemArgs<'a, Clock, Strategy, Risk, MarketStream, GlobalData, FnInstrumentData>,
    /// 可选的 Engine 事件处理模式。
    engine_feed_mode: Option<EngineFeedMode>,
    /// 可选的审计模式。
    audit_mode: Option<AuditMode>,
    /// 可选的初始交易状态。
    trading_state: Option<TradingState>,
    /// 初始交易所资产余额。
    balances: FnvHashMap<ExchangeAsset<AssetNameInternal>, Balance>,
}

impl<'a, Clock, Strategy, Risk, MarketStream, GlobalData, FnInstrumentData>
    SystemBuilder<'a, Clock, Strategy, Risk, MarketStream, GlobalData, FnInstrumentData>
{
    /// 使用提供的 `SystemArguments` 创建新的 `SystemBuilder`。
    ///
    /// 使用可选配置的默认值初始化构建器。
    ///
    /// # 参数
    ///
    /// - `config`: 系统构建参数
    ///
    /// # 返回值
    ///
    /// 返回新创建的 SystemBuilder 实例。
    pub fn new(
        config: SystemArgs<'a, Clock, Strategy, Risk, MarketStream, GlobalData, FnInstrumentData>,
    ) -> Self {
        Self {
            args: config,
            engine_feed_mode: None,
            audit_mode: None,
            trading_state: None,
            balances: FnvHashMap::default(),
        }
    }

    /// 可选配置 [`EngineFeedMode`]（`Iterator` 或 `Stream`）。
    ///
    /// 控制 Engine 是同步还是异步处理事件。
    ///
    /// # 参数
    ///
    /// - `value`: Engine 事件处理模式
    ///
    /// # 返回值
    ///
    /// 返回更新后的 SystemBuilder。
    pub fn engine_feed_mode(self, value: EngineFeedMode) -> Self {
        Self {
            engine_feed_mode: Some(value),
            ..self
        }
    }

    /// 可选配置 [`AuditMode`]（启用或禁用）。
    ///
    /// 控制 Engine 是否发送其产生的审计事件。
    ///
    /// # 参数
    ///
    /// - `value`: 审计模式
    ///
    /// # 返回值
    ///
    /// 返回更新后的 SystemBuilder。
    pub fn audit_mode(self, value: AuditMode) -> Self {
        Self {
            audit_mode: Some(value),
            ..self
        }
    }

    /// 可选配置初始 [`TradingState`]（启用或禁用）。
    ///
    /// 设置系统启动时算法交易是否初始启用。
    ///
    /// # 参数
    ///
    /// - `value`: 交易状态
    ///
    /// # 返回值
    ///
    /// 返回更新后的 SystemBuilder。
    pub fn trading_state(self, value: TradingState) -> Self {
        Self {
            trading_state: Some(value),
            ..self
        }
    }

    /// 可选提供初始交易所资产 `Balance`。
    ///
    /// 对于需要在 EngineState 中初始化初始 `Balance` 的回测场景很有用。
    ///
    /// 注意：内部实现使用 `HashMap`，因此重复的 `ExchangeAsset<AssetNameInternal>` 键会被覆盖。
    ///
    /// # 类型参数
    ///
    /// - `BalanceIter`: 余额迭代器类型
    /// - `KeyedBalance`: 键值化余额类型
    ///
    /// # 参数
    ///
    /// - `balances`: 初始余额迭代器
    ///
    /// # 返回值
    ///
    /// 返回更新后的 SystemBuilder。
    pub fn balances<BalanceIter, KeyedBalance>(mut self, balances: BalanceIter) -> Self
    where
        BalanceIter: IntoIterator<Item = KeyedBalance>,
        KeyedBalance: Into<Keyed<ExchangeAsset<AssetNameInternal>, Balance>>,
    {
        self.balances.extend(balances.into_iter().map(|keyed| {
            let Keyed { key, value } = keyed.into();

            (key, value)
        }));
        self
    }

    /// 使用配置的构建器设置构建 [`SystemBuild`]。
    ///
    /// 此方法构建所有系统组件，但不启动任何任务或流。
    ///
    /// 初始化 `SystemBuild` 实例以启动系统。
    ///
    /// ## 构建流程
    ///
    /// 1. 构建执行基础设施
    /// 2. 构建 EngineState
    /// 3. 构造 Engine
    /// 4. 返回 SystemBuild
    ///
    /// # 类型参数
    ///
    /// - `Event`: Engine 事件类型
    /// - `InstrumentData`: 交易对数据类型
    ///
    /// # 返回值
    ///
    /// 返回已构建但未初始化的 SystemBuild，如果构建失败则返回错误。
    pub fn build<Event, InstrumentData>(
        self,
    ) -> Result<
        SystemBuild<
            Engine<
                Clock,
                EngineState<GlobalData, InstrumentData>,
                MultiExchangeTxMap,
                Strategy,
                Risk,
            >,
            Event,
            MarketStream,
        >,
        BarterError,
    >
    where
        Clock: EngineClock + Clone + Send + Sync + 'static,
        FnInstrumentData: Fn(
            &'a Keyed<InstrumentIndex, Instrument<Keyed<ExchangeIndex, ExchangeId>, AssetIndex>>,
        ) -> InstrumentData,
    {
        let Self {
            args:
                SystemArgs {
                    instruments,
                    executions,
                    clock,
                    strategy,
                    risk,
                    market_stream,
                    global_data,
                    instrument_data_init,
                },
            engine_feed_mode,
            audit_mode,
            trading_state,
            balances,
        } = self;

        // Default if not provided
        let engine_feed_mode = engine_feed_mode.unwrap_or_default();
        let audit_mode = audit_mode.unwrap_or_default();
        let trading_state = trading_state.unwrap_or_default();

        // Build Execution infrastructure
        let execution = executions
            .into_iter()
            .try_fold(
                ExecutionBuilder::new(instruments),
                |builder, config| match config {
                    ExecutionConfig::Mock(mock_config) => {
                        builder.add_mock(mock_config, clock.clone())
                    }
                },
            )?
            .build();

        // Build EngineState
        let state = EngineStateBuilder::new(instruments, global_data, instrument_data_init)
            .time_engine_start(clock.time())
            .trading_state(trading_state)
            .balances(
                balances
                    .into_iter()
                    .map(|(key, value)| Keyed::new(key, value)),
            )
            .build();

        // Construct Engine
        let engine = Engine::new(clock, state, execution.execution_tx_map, strategy, risk);

        Ok(SystemBuild {
            engine,
            engine_feed_mode,
            audit_mode,
            market_stream,
            account_channel: execution.account_channel,
            execution_build_futures: execution.futures,
            phantom_event: PhantomData,
        })
    }
}

/// 已完全构建但准备初始化的 `SystemBuild`。
///
/// 这是在生成任务和运行系统之前的中间步骤。
///
/// ## 类型参数
///
/// - `Engine`: Engine 类型
/// - `Event`: Engine 事件类型
/// - `MarketStream`: 市场数据流类型
///
/// ## 字段说明
///
/// - **engine**: 已构建的 `Engine` 实例
/// - **engine_feed_mode**: 选择的 [`EngineFeedMode`]
/// - **audit_mode**: 选择的 [`AuditMode`]
/// - **market_stream**: `MarketStreamEvent` 流
/// - **account_channel**: `AccountStreamEvent` 通道
/// - **execution_build_futures**: 用于初始化 `ExecutionBuild` 组件的 Future
#[allow(missing_debug_implementations)]
pub struct SystemBuild<Engine, Event, MarketStream> {
    /// 已构建的 `Engine` 实例。
    pub engine: Engine,

    /// 选择的 [`EngineFeedMode`]。
    pub engine_feed_mode: EngineFeedMode,

    /// 选择的 [`AuditMode`]。
    pub audit_mode: AuditMode,

    /// `MarketStreamEvent` 流。
    pub market_stream: MarketStream,

    /// `AccountStreamEvent` 通道。
    pub account_channel: Channel<AccountStreamEvent>,

    /// 用于初始化 `ExecutionBuild` 组件的 Future。
    pub execution_build_futures: ExecutionBuildFutures,

    /// 事件类型标记。
    phantom_event: PhantomData<Event>,
}

impl<Engine, Event, MarketStream> SystemBuild<Engine, Event, MarketStream>
where
    Engine: Processor<Event>
        + Auditor<Engine::Audit, Context = EngineContext>
        + SyncShutdown
        + Send
        + 'static,
    Engine::Audit: From<FeedEnded> + Terminal + Debug + Clone + Send + 'static,
    Event: From<MarketStream::Item> + From<AccountStreamEvent> + Debug + Clone + Send + 'static,
    MarketStream: Stream + Send + 'static,
{
    /// 从提供的组件构造新的 `SystemBuild`。
    ///
    /// # 参数
    ///
    /// - `engine`: 已构建的 Engine 实例
    /// - `engine_feed_mode`: Engine 事件处理模式
    /// - `audit_mode`: 审计模式
    /// - `market_stream`: 市场数据流
    /// - `account_channel`: 账户事件通道
    /// - `execution_build_futures`: 执行构建 Future
    ///
    /// # 返回值
    ///
    /// 返回新创建的 SystemBuild 实例。
    pub fn new(
        engine: Engine,
        engine_feed_mode: EngineFeedMode,
        audit_mode: AuditMode,
        market_stream: MarketStream,
        account_channel: Channel<AccountStreamEvent>,
        execution_build_futures: ExecutionBuildFutures,
    ) -> Self {
        Self {
            engine,
            engine_feed_mode,
            audit_mode,
            market_stream,
            account_channel,
            execution_build_futures,
            phantom_event: Default::default(),
        }
    }

    /// 使用当前 tokio 运行时初始化系统。
    ///
    /// 生成所有必要的任务并返回运行中的 `System` 实例。
    ///
    /// ## 初始化流程
    ///
    /// 1. 初始化所有执行组件
    /// 2. 创建 Engine 事件通道
    /// 3. 启动市场事件转发任务
    /// 4. 启动账户事件转发任务
    /// 5. 根据配置的模式运行 Engine
    ///
    /// # 返回值
    ///
    /// 返回初始化的 System 实例，如果初始化失败则返回错误。
    pub async fn init(self) -> Result<System<Engine, Event>, BarterError> {
        self.init_internal(tokio::runtime::Handle::current()).await
    }

    /// 使用提供的 tokio 运行时初始化系统。
    ///
    /// 允许指定自定义运行时来生成任务。
    ///
    /// # 参数
    ///
    /// - `runtime`: tokio 运行时句柄
    ///
    /// # 返回值
    ///
    /// 返回初始化的 System 实例，如果初始化失败则返回错误。
    pub async fn init_with_runtime(
        self,
        runtime: tokio::runtime::Handle,
    ) -> Result<System<Engine, Event>, BarterError> {
        self.init_internal(runtime).await
    }

    async fn init_internal(
        self,
        runtime: tokio::runtime::Handle,
    ) -> Result<System<Engine, Event>, BarterError> {
        let Self {
            mut engine,
            engine_feed_mode,
            audit_mode,
            market_stream,
            account_channel,
            execution_build_futures,
            phantom_event: _,
        } = self;

        // Initialise all execution components
        let execution = execution_build_futures
            .init_with_runtime(runtime.clone())
            .await?;

        // Initialise central Engine channel
        let (feed_tx, mut feed_rx) = mpsc_unbounded();

        // Forward MarketStreamEvents to Engine feed
        let market_to_engine = runtime
            .clone()
            .spawn(market_stream.forward_to(feed_tx.clone()));

        // Forward AccountStreamEvents to Engine feed
        let account_stream = account_channel.rx.into_stream();
        let account_to_engine = runtime.spawn(account_stream.forward_to(feed_tx.clone()));

        // Run Engine in configured mode
        let (engine, audit) = match (engine_feed_mode, audit_mode) {
            (EngineFeedMode::Iterator, AuditMode::Enabled) => {
                // Initialise Audit channel
                let (audit_tx, audit_rx) = mpsc_unbounded();
                let mut audit_tx = ChannelTxDroppable::new(audit_tx);

                let audit = SnapUpdates {
                    snapshot: engine.audit_snapshot(),
                    updates: audit_rx,
                };

                let handle = runtime.spawn_blocking(move || {
                    let shutdown_audit =
                        sync_run_with_audit(&mut feed_rx, &mut engine, &mut audit_tx);

                    (engine, shutdown_audit)
                });

                (handle, Some(audit))
            }
            (EngineFeedMode::Iterator, AuditMode::Disabled) => {
                let handle = runtime.spawn_blocking(move || {
                    let shutdown_audit = sync_run(&mut feed_rx, &mut engine);
                    (engine, shutdown_audit)
                });

                (handle, None)
            }
            (EngineFeedMode::Stream, AuditMode::Enabled) => {
                // Initialise Audit channel
                let (audit_tx, audit_rx) = mpsc_unbounded();
                let mut audit_tx = ChannelTxDroppable::new(audit_tx);

                let audit = SnapUpdates {
                    snapshot: engine.audit_snapshot(),
                    updates: audit_rx,
                };

                let handle = runtime.spawn(async move {
                    let shutdown_audit =
                        async_run_with_audit(&mut feed_rx, &mut engine, &mut audit_tx).await;
                    (engine, shutdown_audit)
                });

                (handle, Some(audit))
            }
            (EngineFeedMode::Stream, AuditMode::Disabled) => {
                let handle = runtime.spawn(async move {
                    let shutdown_audit = async_run(&mut feed_rx, &mut engine).await;
                    (engine, shutdown_audit)
                });

                (handle, None)
            }
        };

        Ok(System {
            engine,
            handles: SystemAuxillaryHandles {
                execution,
                market_to_engine,
                account_to_engine,
            },
            feed_tx,
            audit,
        })
    }
}
