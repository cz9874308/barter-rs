//! ExecutionBuilder 执行构建器模块
//!
//! 本模块提供了执行基础设施的构建器，用于方便地初始化多个执行链接到模拟和真实交易所。
//! ExecutionBuilder 支持添加模拟和真实交易所配置，并自动设置所需的基础设施。
//!
//! # 核心概念
//!
//! - **ExecutionBuilder**: 执行基础设施构建器
//! - **ExecutionBuild**: 已构建的执行基础设施容器
//! - **ExecutionHandles**: 执行组件任务句柄集合

use crate::{
    engine::{clock::EngineClock, execution_tx::MultiExchangeTxMap},
    error::BarterError,
    execution::{
        AccountStreamEvent, Execution, error::ExecutionError, manager::ExecutionManager,
        request::ExecutionRequest,
    },
    shutdown::AsyncShutdown,
};
use barter_data::streams::{
    consumer::STREAM_RECONNECTION_POLICY, reconnect::stream::ReconnectingStream,
};
use barter_execution::{
    UnindexedAccountEvent,
    client::{
        ExecutionClient,
        mock::{MockExecution, MockExecutionClientConfig, MockExecutionConfig},
    },
    exchange::mock::{MockExchange, request::MockExchangeRequest},
    indexer::AccountEventIndexer,
    map::generate_execution_instrument_map,
};
use barter_instrument::{
    Keyed, Underlying,
    asset::{AssetIndex, name::AssetNameExchange},
    exchange::{ExchangeId, ExchangeIndex},
    index::IndexedInstruments,
    instrument::{
        Instrument, InstrumentIndex,
        kind::InstrumentKind,
        name::InstrumentNameExchange,
        spec::{InstrumentSpec, InstrumentSpecQuantity, OrderQuantityUnits},
    },
};
use barter_integration::channel::{Channel, UnboundedTx, mpsc_unbounded};
use fnv::FnvHashMap;
use futures::{FutureExt, future::try_join_all};
use std::{pin::Pin, sync::Arc, time::Duration};
use tokio::{
    sync::{broadcast, mpsc},
    task::{JoinError, JoinHandle},
};

type ExecutionInitFuture =
    Pin<Box<dyn Future<Output = Result<(RunFuture, RunFuture), ExecutionError>> + Send>>;
type RunFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// 完整的执行基础设施构建器。
///
/// 添加模拟和真实 [`ExecutionClient`] 配置，让构建器设置所需的基础设施。
///
/// 添加所有配置后，调用 [`ExecutionBuilder::build`] 返回完整的 [`ExecutionBuild`]。
/// 然后调用 [`ExecutionBuild::init`] 将初始化已构建的基础设施。
///
/// ## 处理的功能
///
/// - 构建模拟执行管理器（通过 [`MockExchange`] 在内部模拟特定交易所）
/// - 构建真实执行管理器，设置到每个交易所的外部连接
/// - 构建 [`MultiExchangeTxMap`]，为每个模拟/真实执行管理器添加条目
/// - 将所有交易所账户流合并为统一的 [`AccountStreamEvent`] `Stream`
///
/// ## 使用流程
///
/// 1. 创建 ExecutionBuilder
/// 2. 添加模拟或真实交易所配置
/// 3. 调用 `build()` 构建基础设施
/// 4. 调用 `init()` 初始化所有组件
///
/// # 使用示例
///
/// ```rust,ignore
/// let builder = ExecutionBuilder::new(&instruments)
///     .add_mock(mock_config, clock)?
///     .add_live::<BinanceClient>(binance_config, timeout)?;
///
/// let execution = builder.build().init().await?;
/// ```
#[allow(missing_debug_implementations)]
pub struct ExecutionBuilder<'a> {
    /// 索引化交易对集合。
    instruments: &'a IndexedInstruments,
    /// 执行请求通道映射（交易所 ID -> (交易所索引, 请求发送器)）。
    execution_txs: FnvHashMap<ExchangeId, (ExchangeIndex, UnboundedTx<ExecutionRequest>)>,
    /// 合并的账户事件通道。
    merged_channel: Channel<AccountStreamEvent<ExchangeIndex, AssetIndex, InstrumentIndex>>,
    /// MockExchange 运行 Future 集合。
    mock_exchange_futures: Vec<RunFuture>,
    /// 执行管理器初始化 Future 集合。
    execution_init_futures: Vec<ExecutionInitFuture>,
}

impl<'a> ExecutionBuilder<'a> {
    /// 使用提供的 `IndexedInstruments` 构造新的 `ExecutionBuilder`。
    ///
    /// # 参数
    ///
    /// - `instruments`: 索引化交易对集合
    ///
    /// # 返回值
    ///
    /// 返回新创建的 ExecutionBuilder 实例。
    pub fn new(instruments: &'a IndexedInstruments) -> Self {
        Self {
            instruments,
            execution_txs: FnvHashMap::default(),
            merged_channel: Channel::default(),
            mock_exchange_futures: Vec::default(),
            execution_init_futures: Vec::default(),
        }
    }

    /// 为模拟交易所添加 [`ExecutionManager`，在内部设置 [`MockExchange`]。
    ///
    /// 此方法添加一个模拟交易所的执行管理器。提供的 [`MockExecutionConfig`] 用于配置
    /// [`MockExchange`] 并提供初始账户状态。
    ///
    /// ## 类型参数
    ///
    /// - `Clock`: Engine 时钟类型
    ///
    /// # 参数
    ///
    /// - `config`: 模拟执行配置
    /// - `clock`: Engine 时钟
    ///
    /// # 返回值
    ///
    /// 返回更新后的 ExecutionBuilder，如果配置无效则返回错误。
    pub fn add_mock<Clock>(
        mut self,
        config: MockExecutionConfig,
        clock: Clock,
    ) -> Result<Self, BarterError>
    where
        Clock: EngineClock + Clone + Send + Sync + 'static,
    {
        const ACCOUNT_STREAM_CAPACITY: usize = 256;
        const DUMMY_EXECUTION_REQUEST_TIMEOUT: Duration = Duration::from_secs(1);

        let (request_tx, request_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = broadcast::channel(ACCOUNT_STREAM_CAPACITY);

        let mock_execution_client_config = MockExecutionClientConfig {
            mocked_exchange: config.mocked_exchange,
            clock: move || clock.time(),
            request_tx,
            event_rx,
        };

        // Register MockExchange init Future
        let mock_exchange_future = self.init_mock_exchange(config, request_rx, event_tx);
        self.mock_exchange_futures.push(mock_exchange_future);

        self.add_execution::<MockExecution<_>>(
            mock_execution_client_config.mocked_exchange,
            mock_execution_client_config,
            DUMMY_EXECUTION_REQUEST_TIMEOUT,
        )
    }

    fn init_mock_exchange(
        &self,
        config: MockExecutionConfig,
        request_rx: mpsc::UnboundedReceiver<MockExchangeRequest>,
        event_tx: broadcast::Sender<UnindexedAccountEvent>,
    ) -> RunFuture {
        let instruments =
            generate_mock_exchange_instruments(self.instruments, config.mocked_exchange);
        Box::pin(MockExchange::new(config, request_rx, event_tx, instruments).run())
    }

    /// 为真实交易所添加 [`ExecutionManager`]。
    ///
    /// 此方法添加一个真实交易所的执行管理器，设置到交易所的外部连接。
    ///
    /// ## 类型参数
    ///
    /// - `Client`: 执行客户端类型
    ///
    /// # 参数
    ///
    /// - `config`: 客户端配置
    /// - `request_timeout`: 请求超时时间
    ///
    /// # 返回值
    ///
    /// 返回更新后的 ExecutionBuilder，如果配置无效则返回错误。
    pub fn add_live<Client>(
        self,
        config: Client::Config,
        request_timeout: Duration,
    ) -> Result<Self, BarterError>
    where
        Client: ExecutionClient + Send + Sync + 'static,
        Client::AccountStream: Send,
        Client::Config: Send,
    {
        self.add_execution::<Client>(Client::EXCHANGE, config, request_timeout)
    }

    fn add_execution<Client>(
        mut self,
        exchange: ExchangeId,
        config: Client::Config,
        request_timeout: Duration,
    ) -> Result<Self, BarterError>
    where
        Client: ExecutionClient + Send + Sync + 'static,
        Client::AccountStream: Send,
        Client::Config: Send,
    {
        let instrument_map = generate_execution_instrument_map(self.instruments, exchange)?;

        let (execution_tx, execution_rx) = mpsc_unbounded();

        if self
            .execution_txs
            .insert(exchange, (instrument_map.exchange.key, execution_tx))
            .is_some()
        {
            return Err(BarterError::ExecutionBuilder(format!(
                "ExecutionBuilder does not support duplicate mocked ExecutionManagers: {exchange}"
            )));
        }

        let merged_tx = self.merged_channel.tx.clone();

        // Init ExecutionManager Future
        let future_result = ExecutionManager::init(
            execution_rx.into_stream(),
            request_timeout,
            Arc::new(Client::new(config)),
            AccountEventIndexer::new(Arc::new(instrument_map)),
            STREAM_RECONNECTION_POLICY,
        );

        let future_result = future_result.map(|result| {
            result.map(|(manager, account_stream)| {
                let manager_future: RunFuture = Box::pin(manager.run());
                let stream_future: RunFuture = Box::pin(account_stream.forward_to(merged_tx));

                (manager_future, stream_future)
            })
        });

        self.execution_init_futures.push(Box::pin(future_result));

        Ok(self)
    }

    /// 消费此 `ExecutionBuilder` 并构建包含所有 [`ExecutionManager`]（模拟和真实）
    /// 和 [`MockExchange`] Future 的完整 [`ExecutionBuild`]。
    ///
    /// **对于大多数用户，在此之后调用 [`ExecutionBuild::init`] 就足够了。**
    ///
    /// 如果你想要更多控制哪个运行时驱动 Future 完成，可以调用 [`ExecutionBuild::init_with_runtime`]。
    ///
    /// ## 构建流程
    ///
    /// 1. 构建索引化的 ExecutionTx 映射
    /// 2. 收集所有执行管理器初始化 Future
    /// 3. 返回 ExecutionBuild 容器
    ///
    /// # 返回值
    ///
    /// 返回包含所有执行基础设施组件的 ExecutionBuild。
    pub fn build(mut self) -> ExecutionBuild {
        // Construct indexed ExecutionTx map
        let execution_tx_map = self
            .instruments
            .exchanges()
            .iter()
            .map(|exchange| {
                // If IndexedInstruments execution not used for execution, add None to map
                let Some((added_execution_exchange_index, added_execution_exchange_tx)) =
                    self.execution_txs.remove(&exchange.value)
                else {
                    return (exchange.value, None);
                };

                assert_eq!(
                    exchange.key, added_execution_exchange_index,
                    "execution ExchangeIndex != IndexedInstruments Keyed<ExchangeIndex, ExchangeId>"
                );

                // If execution has been added, add Some(ExecutionTx) to map
                (exchange.value, Some(added_execution_exchange_tx))
            })
            .collect();

        ExecutionBuild {
            execution_tx_map,
            account_channel: self.merged_channel,
            futures: ExecutionBuildFutures {
                mock_exchange_run_futures: self.mock_exchange_futures,
                execution_init_futures: self.execution_init_futures,
            },
        }
    }
}

/// 包含准备初始化的执行基础设施组件的容器。
///
/// 调用 [`ExecutionBuild::init`] 在 tokio 任务上运行所有必需的执行组件 Future -
/// 返回 [`MultiExchangeTxMap`] 和多交易所 [`AccountStreamEvent`] 流。
///
/// ## 字段说明
///
/// - **execution_tx_map**: 多交易所执行请求通道映射
/// - **account_channel**: 合并的账户事件通道
/// - **futures**: 执行组件 Future 集合
#[allow(missing_debug_implementations)]
pub struct ExecutionBuild {
    /// 多交易所执行请求通道映射。
    pub execution_tx_map: MultiExchangeTxMap,
    /// 合并的账户事件通道。
    pub account_channel: Channel<AccountStreamEvent>,
    /// 执行组件 Future 集合。
    pub futures: ExecutionBuildFutures,
}

impl ExecutionBuild {
    /// 在当前 tokio 运行时上初始化所有执行组件。
    ///
    /// 此方法：
    /// - 生成 [`MockExchange`] 运行器 tokio 任务
    /// - 初始化所有 [`ExecutionManager`] 及其 AccountStream
    /// - 返回 `MultiExchangeTxMap` 和多交易所 AccountStream
    ///
    /// # 返回值
    ///
    /// 返回初始化的 Execution 实例，如果初始化失败则返回错误。
    pub async fn init(self) -> Result<Execution, BarterError> {
        self.init_internal(tokio::runtime::Handle::current()).await
    }

    /// 在提供的 tokio 运行时上初始化所有执行组件。
    ///
    /// 如果你想要更多控制哪个 tokio 运行时处理运行执行组件，请使用此方法。
    ///
    /// 此方法：
    /// - 生成 [`MockExchange`] 运行器 tokio 任务
    /// - 初始化所有 [`ExecutionManager`] 及其 AccountStream
    /// - 返回 `MultiExchangeTxMap` 和多交易所 AccountStream
    ///
    /// # 参数
    ///
    /// - `runtime`: tokio 运行时句柄
    ///
    /// # 返回值
    ///
    /// 返回初始化的 Execution 实例，如果初始化失败则返回错误。
    pub async fn init_with_runtime(
        self,
        runtime: tokio::runtime::Handle,
    ) -> Result<Execution, BarterError> {
        self.init_internal(runtime).await
    }

    async fn init_internal(
        self,
        runtime: tokio::runtime::Handle,
    ) -> Result<Execution, BarterError> {
        let handles = self.futures.init_with_runtime(runtime).await?;

        Ok(Execution {
            execution_txs: self.execution_tx_map,
            account_channel: self.account_channel,
            handles,
        })
    }
}

#[allow(missing_debug_implementations)]
pub struct ExecutionBuildFutures {
    pub mock_exchange_run_futures: Vec<RunFuture>,
    pub execution_init_futures: Vec<ExecutionInitFuture>,
}

impl ExecutionBuildFutures {
    /// Initialises all execution components on the current tokio runtime.
    ///
    /// This method:
    /// - Spawns [`MockExchange`] runner tokio tasks.
    /// - Initialises all [`ExecutionManager`]s and their AccountStreams.
    /// - Spawns tokio tasks to forward AccountStreams to multi-exchange AccountStream
    pub async fn init(self) -> Result<ExecutionHandles, BarterError> {
        self.init_internal(tokio::runtime::Handle::current()).await
    }

    /// Initialises all execution components on the provided tokio runtime.
    ///
    /// Use this method if you want more control over which tokio runtime handles running
    /// execution components.
    ///
    /// This method:
    /// - Spawns [`MockExchange`] runner tokio tasks.
    /// - Initialises all [`ExecutionManager`]s and their AccountStreams.
    /// - Spawns tokio tasks to forward AccountStreams to multi-exchange AccountStream
    pub async fn init_with_runtime(
        self,
        runtime: tokio::runtime::Handle,
    ) -> Result<ExecutionHandles, BarterError> {
        self.init_internal(runtime).await
    }

    async fn init_internal(
        self,
        runtime: tokio::runtime::Handle,
    ) -> Result<ExecutionHandles, BarterError> {
        let mock_exchanges = self
            .mock_exchange_run_futures
            .into_iter()
            .map(|mock_exchange_run_future| runtime.spawn(mock_exchange_run_future))
            .collect();

        // Await ExecutionManager build futures and ensure success
        let (managers, account_to_engines) =
            futures::future::try_join_all(self.execution_init_futures)
                .await?
                .into_iter()
                .map(|(manager_run_future, account_event_forward_future)| {
                    (
                        runtime.spawn(manager_run_future),
                        runtime.spawn(account_event_forward_future),
                    )
                })
                .unzip();

        Ok(ExecutionHandles {
            mock_exchanges,
            managers,
            account_to_engines,
        })
    }
}

#[allow(missing_debug_implementations)]
pub struct ExecutionHandles {
    pub mock_exchanges: Vec<JoinHandle<()>>,
    pub managers: Vec<JoinHandle<()>>,
    pub account_to_engines: Vec<JoinHandle<()>>,
}

impl AsyncShutdown for ExecutionHandles {
    type Result = Result<(), JoinError>;

    async fn shutdown(&mut self) -> Self::Result {
        let handles = self
            .mock_exchanges
            .drain(..)
            .chain(self.managers.drain(..))
            .chain(self.account_to_engines.drain(..));

        try_join_all(handles).await?;
        Ok(())
    }
}

impl IntoIterator for ExecutionHandles {
    type Item = JoinHandle<()>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.mock_exchanges
            .into_iter()
            .chain(self.managers)
            .chain(self.account_to_engines)
            .collect::<Vec<_>>()
            .into_iter()
    }
}

fn generate_mock_exchange_instruments(
    instruments: &IndexedInstruments,
    exchange: ExchangeId,
) -> FnvHashMap<InstrumentNameExchange, Instrument<ExchangeId, AssetNameExchange>> {
    instruments
        .instruments()
        .iter()
        .filter_map(
            |Keyed {
                 key: _,
                 value: instrument,
             }| {
                if instrument.exchange.value != exchange {
                    return None;
                }

                let Instrument {
                    exchange,
                    name_internal,
                    name_exchange,
                    underlying,
                    quote,
                    kind,
                    spec,
                } = instrument;

                let kind = match kind {
                    InstrumentKind::Spot => InstrumentKind::Spot,
                    unsupported => {
                        panic!("MockExchange does not support: {unsupported:?}")
                    }
                };

                let spec = match spec {
                    Some(spec) => {
                        let InstrumentSpec {
                            price,
                            quantity:
                                InstrumentSpecQuantity {
                                    unit,
                                    min,
                                    increment,
                                },
                            notional,
                        } = spec;

                        let unit = match unit {
                            OrderQuantityUnits::Asset(asset) => {
                                let quantity_asset = instruments
                                    .find_asset(*asset)
                                    .unwrap()
                                    .asset
                                    .name_exchange
                                    .clone();
                                OrderQuantityUnits::Asset(quantity_asset)
                            }
                            OrderQuantityUnits::Contract => OrderQuantityUnits::Contract,
                            OrderQuantityUnits::Quote => OrderQuantityUnits::Quote,
                        };

                        Some(InstrumentSpec {
                            price: *price,
                            quantity: InstrumentSpecQuantity {
                                unit,
                                min: *min,
                                increment: *increment,
                            },
                            notional: *notional,
                        })
                    }
                    None => None,
                };

                let underlying_base = instruments
                    .find_asset(underlying.base)
                    .unwrap()
                    .asset
                    .name_exchange
                    .clone();

                let underlying_quote = instruments
                    .find_asset(underlying.quote)
                    .unwrap()
                    .asset
                    .name_exchange
                    .clone();

                let instrument = Instrument {
                    exchange: exchange.value,
                    name_internal: name_internal.clone(),
                    name_exchange: name_exchange.clone(),
                    underlying: Underlying {
                        base: underlying_base,
                        quote: underlying_quote,
                    },
                    quote: *quote,
                    kind,
                    spec,
                };

                Some((instrument.name_exchange.clone(), instrument))
            },
        )
        .collect()
}
