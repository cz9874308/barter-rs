//! ExecutionManager 执行管理器模块
//!
//! 本模块定义了每个交易所的执行管理器，负责处理来自 Engine 的订单请求并转发响应。
//! ExecutionManager 是 Engine 与交易所之间的桥梁，处理订单执行、账户事件等。
//!
//! # 核心概念
//!
//! - **ExecutionManager**: 每个交易所的执行管理器
//! - **工作流程**: 接收请求 → 转换标识符 → 发送到交易所 → 处理响应 → 转发回 Engine
//! - **超时处理**: 跟踪请求并在超时时返回错误

use crate::execution::{
    AccountStreamEvent,
    error::ExecutionError,
    request::{ExecutionRequest, RequestFuture},
};
use barter_data::streams::{
    consumer::StreamKey,
    reconnect::stream::{ReconnectingStream, ReconnectionBackoffPolicy, init_reconnecting_stream},
};
use barter_execution::{
    AccountEvent, AccountEventKind,
    client::ExecutionClient,
    error::{ConnectivityError, OrderError, UnindexedOrderError},
    indexer::{AccountEventIndexer, IndexedAccountStream},
    map::ExecutionInstrumentMap,
    order::{
        Order,
        request::{
            OrderRequestCancel, OrderRequestOpen, OrderResponseCancel, UnindexedOrderResponseCancel,
        },
        state::{Open, OrderState},
    },
};
use barter_instrument::{
    asset::{AssetIndex, name::AssetNameExchange},
    exchange::{ExchangeId, ExchangeIndex},
    index::error::IndexError,
    instrument::{InstrumentIndex, name::InstrumentNameExchange},
};
use barter_integration::{
    channel::{Tx, UnboundedTx, mpsc_unbounded},
    snapshot::Snapshot,
    stream::merge::merge,
};
use derive_more::Constructor;
use futures::{Stream, StreamExt, future::Either, stream::FuturesUnordered};
use std::sync::Arc;
use tracing::{error, info, warn};

/// 每个交易所的执行管理器，处理来自 Engine 的订单请求并转发响应。
///
/// ExecutionManager 处理索引化的 Engine [`ExecutionRequest`]，具体流程：
/// - 将请求转换为使用关联交易所的资产和交易对名称
/// - 通过关联交易所的 [`ExecutionClient`] 发出请求
/// - 跟踪请求并在必要时向 Engine 返回超时
///
/// ## 类型参数
///
/// - `RequestStream`: 请求流类型
/// - `Client`: 执行客户端类型
///
/// ## 工作流程
///
/// 1. 接收来自 Engine 的执行请求
/// 2. 将索引标识符转换为交易所特定标识符
/// 3. 通过 ExecutionClient 发送请求到交易所
/// 4. 跟踪请求并等待响应
/// 5. 处理响应（成功、错误或超时）
/// 6. 将响应转换回索引格式并转发回 Engine
#[derive(Debug, Constructor)]
pub struct ExecutionManager<RequestStream, Client> {
    /// 来自 Engine 的 [`ExecutionRequest`] 流。
    pub request_stream: RequestStream,

    /// 等待 [`ExecutionClient`] 执行请求响应的最大 `Duration`。
    pub request_timeout: std::time::Duration,

    /// 用于将执行请求响应发送回 Engine 的发送器。
    pub response_tx: UnboundedTx<AccountStreamEvent<ExchangeIndex, AssetIndex, InstrumentIndex>>,

    /// 用于执行订单的交易所特定 [`ExecutionClient`]。
    pub client: Arc<Client>,

    /// 用于在交易所特定标识符和索引标识符之间转换的映射器。
    ///
    /// 例如，`InstrumentNameExchange` -> `InstrumentIndex`。
    pub indexer: AccountEventIndexer,
}

impl<RequestStream, Client> ExecutionManager<RequestStream, Client>
where
    RequestStream: Stream<Item = ExecutionRequest<ExchangeIndex, InstrumentIndex>> + Unpin,
    Client: ExecutionClient + Send + Sync,
    Client::AccountStream: Send,
{
    /// 初始化新的 `ExecutionManager` 及其关联的 AccountStream。
    ///
    /// 此方法初始化执行管理器并设置账户事件流。AccountStream 的第一项将是完整的账户快照。
    ///
    /// ## 初始化流程
    ///
    /// 1. 确定账户流键和交易所 ID（用于日志记录）
    /// 2. 初始化带自动重连的 IndexedAccountStream（快照 + 更新）
    /// 3. 构建用于与 Engine 通信的通道
    /// 4. 合并执行响应和账户通知流
    ///
    /// # 参数
    ///
    /// - `request_stream`: 请求流
    /// - `request_timeout`: 请求超时时间
    /// - `client`: 执行客户端
    /// - `indexer`: 账户事件索引器
    /// - `reconnect_policy`: 重连退避策略
    ///
    /// # 返回值
    ///
    /// 返回一个元组，包含 ExecutionManager 实例和合并的账户事件流。
    pub async fn init(
        request_stream: RequestStream,
        request_timeout: std::time::Duration,
        client: Arc<Client>,
        indexer: AccountEventIndexer,
        reconnect_policy: ReconnectionBackoffPolicy,
    ) -> Result<(Self, impl Stream<Item = AccountStreamEvent> + Send), ExecutionError> {
        // 确定 StreamKey 和 ExchangeId（用于日志记录）
        let stream_key = Self::determine_account_stream_key(&indexer.map)?;

        info!(
            exchange_index = %indexer.map.exchange.key,
            exchange_id = %indexer.map.exchange.value,
            policy = ?reconnect_policy,
            ?stream_key,
            "AccountStream with auto reconnect initialising"
        );

        // 初始化带重连的 IndexedAccountStream（快照 + 更新）
        let client_clone = Arc::clone(&client);
        let indexer_clone = indexer.clone();
        let account_stream = init_reconnecting_stream(move || {
            let client = client_clone.clone();
            let indexer = indexer_clone.clone();
            async move {
                // 分配 AssetNameExchanges 和 InstrumentNameExchanges 以避免生命周期问题
                let assets = indexer.map.exchange_assets().cloned().collect::<Vec<_>>();
                let instruments = indexer
                    .map
                    .exchange_instruments()
                    .cloned()
                    .collect::<Vec<_>>();

                // 初始化 AccountStream 并应用索引
                let updates = Self::init_indexed_account_stream(
                    &client,
                    indexer.clone(),
                    &assets,
                    &instruments,
                )
                .await?;

                // 获取 AccountSnapshot 并索引
                let snapshot =
                    Self::fetch_indexed_account_snapshot(&client, &indexer, &assets, &instruments)
                        .await?;

                // 预期下游消费者（例如 EngineState）会同步更新
                Ok(futures::stream::once(std::future::ready(snapshot)).chain(updates))
            }
        })
        .await?;

        // 构建用于与 Engine 通信 ExecutionRequest 响应（即 AccountEvents）的通道
        let (response_tx, response_rx) = mpsc_unbounded();

        // 构建合并的 IndexedAccountStream（执行响应 + 账户通知）
        let merged_account_stream = merge(
            response_rx.into_stream(),
            account_stream
                .with_reconnect_backoff::<_, ExecutionError>(reconnect_policy, stream_key)
                .with_reconnection_events(indexer.map.exchange.value),
        );

        Ok((
            Self::new(
                request_stream,
                request_timeout,
                response_tx,
                client,
                indexer,
            ),
            merged_account_stream,
        ))
    }

    fn determine_account_stream_key(
        instrument_map: &Arc<ExecutionInstrumentMap>,
    ) -> Result<StreamKey, ExecutionError> {
        match (Client::EXCHANGE, instrument_map.exchange.value) {
            (ExchangeId::Mock, instrument_exchange) => Ok(StreamKey::new_general(
                "account_stream_mock",
                instrument_exchange,
            )),
            (ExchangeId::Simulated, instrument_exchange) => Ok(StreamKey::new_general(
                "account_stream_simulated",
                instrument_exchange,
            )),
            (client, instrument_exchange) if client == instrument_exchange => {
                Ok(StreamKey::new_general("account_stream", client))
            }
            (client, instrument_exchange) => Err(ExecutionError::Config(format!(
                "ExecutionManager Client ExchangeId: {client} does not match \
                    ExecutionInstrumentMap ExchangeId: {instrument_exchange}"
            ))),
        }
    }

    async fn fetch_indexed_account_snapshot(
        client: &Arc<Client>,
        indexer: &AccountEventIndexer,
        assets: &[AssetNameExchange],
        instruments: &[InstrumentNameExchange],
    ) -> Result<AccountEvent, ExecutionError> {
        match client.account_snapshot(assets, instruments).await {
            Ok(snapshot) => {
                let indexed_snapshot = indexer.snapshot(snapshot)?;
                Ok(AccountEvent {
                    exchange: indexer.map.exchange.key,
                    kind: AccountEventKind::Snapshot(indexed_snapshot),
                })
            }
            Err(error) => Err(ExecutionError::Client(indexer.client_error(error)?)),
        }
    }

    async fn init_indexed_account_stream(
        client: &Arc<Client>,
        indexer: AccountEventIndexer,
        assets: &[AssetNameExchange],
        instruments: &[InstrumentNameExchange],
    ) -> Result<impl Stream<Item = AccountEvent> + use<RequestStream, Client>, ExecutionError> {
        let stream = match client.account_stream(assets, instruments).await {
            Ok(stream) => stream,
            Err(error) => return Err(ExecutionError::Client(indexer.client_error(error)?)),
        };

        Ok(
            IndexedAccountStream::new(stream, indexer).filter_map(|result| {
                std::future::ready(match result {
                    Ok(indexed_event) => Some(indexed_event),
                    Err(error) => {
                        error!(
                            ?error,
                            "filtered IndexError produced by IndexedAccountStream"
                        );
                        None
                    }
                })
            }),
        )
    }

    /// 运行 `ExecutionManager`，处理执行请求并通过 AccountStream 转发响应。
    ///
    /// 此方法运行执行管理器的主循环，处理来自 Engine 的执行请求并转发响应。
    /// 它同时处理取消和开仓请求，跟踪在途请求，并在超时时返回错误。
    ///
    /// ## 工作流程
    ///
    /// 1. 接收来自 Engine 的执行请求
    /// 2. 将请求转换为交易所格式并发送
    /// 3. 跟踪在途请求
    /// 4. 处理响应（成功、错误或超时）
    /// 5. 将响应转换回索引格式并转发回 Engine
    ///
    /// ## 超时处理
    ///
    /// 如果请求在超时时间内未收到响应，会生成超时错误并转发回 Engine。
    pub async fn run(mut self) {
        let mut in_flight_cancels = FuturesUnordered::new();
        let mut in_flight_opens = FuturesUnordered::new();

        loop {
            let next_cancel_response = if in_flight_cancels.is_empty() {
                Either::Left(std::future::pending())
            } else {
                Either::Right(in_flight_cancels.select_next_some())
            };

            let next_open_response = if in_flight_opens.is_empty() {
                Either::Left(std::future::pending())
            } else {
                Either::Right(in_flight_opens.select_next_some())
            };

            tokio::select! {
                // Process Engine ExecutionRequests
                request = self.request_stream.next() => match request {
                    Some(ExecutionRequest::Shutdown) | None => {
                        break;
                    }
                    Some(ExecutionRequest::Cancel(request)) => {
                        // Panic since the system is set up incorrectly, so it's foolish to continue
                        let client_request = self
                            .indexer
                            .order_request(&request)
                            .unwrap_or_else(|error| panic!(
                                "ExecutionManager received cancel request for non-configured key: {error}"
                            ));

                        in_flight_cancels.push(RequestFuture::new(
                            self.client.cancel_order(client_request),
                            self.request_timeout,
                            request,
                        ))
                    },
                    Some(ExecutionRequest::Open(request)) => {
                        // Panic since the system is set up incorrectly, so it's foolish to continue
                        let client_request = self
                            .indexer
                            .order_request(&request)
                            .unwrap_or_else(|error| panic!(
                                "ExecutionManager received open request for non-configured key: {error}"
                            ));

                        in_flight_opens.push(RequestFuture::new(
                            self.client.open_order(client_request),
                            self.request_timeout,
                            request,
                        ))
                    }
                },

                // Process next ExecutionRequest::Cancel response
                response_cancel = next_cancel_response => {
                    match response_cancel {
                        Ok(Some(response)) => {
                            let event = match self.process_cancel_response(response) {
                                Ok(indexed_event) => indexed_event,
                                Err(error) => {
                                    warn!(
                                        exchange = %self.indexer.map.exchange.value,
                                        ?error,
                                        "ExecutionManager filtering cancel response due to unrecognised index"
                                    );
                                    continue
                                }
                            };

                            if self.response_tx.send(event).is_err() {
                                break;
                            }
                        }
                        Err(request) => {
                            let event = Self::process_cancel_timeout(request);

                            if self.response_tx.send(event).is_err() {
                                break;
                            }
                        }
                        Ok(None) => {
                            // Do nothing
                        }
                    };
                },

                // Process next ExecutionRequest::Open response
                response_open = next_open_response => {
                    match response_open {
                        Ok(Some(response)) => {
                            let event = match self.process_open_response(response) {
                                Ok(indexed_event) => indexed_event,
                                Err(error) => {
                                    warn!(
                                        exchange = %self.indexer.map.exchange.value,
                                        ?error,
                                        "ExecutionManager filtering open response due to unrecognised index"
                                    );
                                    continue
                                }
                            };

                            if self.response_tx.send(event).is_err() {
                                break;
                            }
                        }
                        Err(request) => {
                            let event = Self::process_open_timeout(request);

                            if self.response_tx.send(event).is_err() {
                                break;
                            }
                        }
                        Ok(None) => {
                            // Do nothing
                        }
                    }
                }
            }
        }

        info!(
            exchange = %self.indexer.map.exchange.value,
            "ExecutionManager shutting down"
        )
    }

    fn process_cancel_response(
        &self,
        order: UnindexedOrderResponseCancel,
    ) -> Result<AccountStreamEvent, IndexError> {
        let order = self.indexer.order_response_cancel(order)?;

        Ok(AccountStreamEvent::Item(AccountEvent {
            exchange: order.key.exchange,
            kind: AccountEventKind::OrderCancelled(order),
        }))
    }

    fn process_cancel_timeout(
        order: OrderRequestCancel<ExchangeIndex, InstrumentIndex>,
    ) -> AccountStreamEvent {
        let OrderRequestCancel { key, state: _ } = order;

        AccountStreamEvent::Item(AccountEvent {
            exchange: key.exchange,
            kind: AccountEventKind::OrderCancelled(OrderResponseCancel {
                key,
                state: Err(OrderError::Connectivity(ConnectivityError::Timeout)),
            }),
        })
    }

    fn process_open_response(
        &self,
        order: Order<ExchangeId, InstrumentNameExchange, Result<Open, UnindexedOrderError>>,
    ) -> Result<AccountStreamEvent, IndexError> {
        let Order {
            key,
            side,
            price,
            quantity,
            kind,
            time_in_force,
            state,
        } = order;

        let key = self.indexer.order_key(key)?;

        let state = match state {
            Ok(open) if open.quantity_remaining(quantity).is_zero() => OrderState::fully_filled(),
            Ok(open) => OrderState::active(open),
            Err(error) => OrderState::inactive(self.indexer.order_error(error)?),
        };

        Ok(AccountStreamEvent::Item(AccountEvent {
            exchange: key.exchange,
            kind: AccountEventKind::OrderSnapshot(Snapshot(Order {
                key,
                side,
                price,
                quantity,
                kind,
                time_in_force,
                state,
            })),
        }))
    }

    fn process_open_timeout(
        order: OrderRequestOpen<ExchangeIndex, InstrumentIndex>,
    ) -> AccountStreamEvent {
        let OrderRequestOpen { key, state } = order;

        AccountStreamEvent::Item(AccountEvent {
            exchange: key.exchange,
            kind: AccountEventKind::OrderSnapshot(Snapshot(Order {
                key,
                side: state.side,
                price: state.price,
                quantity: state.quantity,
                kind: state.kind,
                time_in_force: state.time_in_force,
                state: OrderState::inactive(OrderError::Connectivity(ConnectivityError::Timeout)),
            })),
        })
    }
}
