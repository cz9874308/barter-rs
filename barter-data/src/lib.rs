// 允许 dev-dependencies 中的未使用 extern crate 警告
// 这些依赖仅在示例/测试/基准测试中使用，不在库代码中使用
#![allow(unused_extern_crates)]
#![forbid(unsafe_code)]
#![warn(
    unused,
    clippy::cognitive_complexity,
    unused_crate_dependencies,
    clippy::unused_self,
    clippy::useless_let_if_seq,
    missing_debug_implementations,
    rust_2018_idioms,
    rust_2024_compatibility
)]
#![allow(clippy::type_complexity, clippy::too_many_arguments, type_alias_bounds)]

//! # Barter-Data
//! A high-performance WebSocket integration library for streaming public market data from leading cryptocurrency
//! exchanges - batteries included. It is:
//! * **Easy**: Barter-Data's simple [`StreamBuilder`](streams::builder::StreamBuilder) and [`DynamicStreams`](streams::builder::dynamic::DynamicStreams) interface allows for easy & quick setup (see example below and /examples!).
//! * **Normalised**: Barter-Data's unified interface for consuming public WebSocket data means every Exchange returns a normalised data model.
//! * **Real-Time**: Barter-Data utilises real-time WebSocket integrations enabling the consumption of normalised tick-by-tick data.

//! * **Extensible**: Barter-Data is highly extensible, and therefore easy to contribute to with coding new integrations!
//!
//! ## User API
//! - [`StreamBuilder`](streams::builder::StreamBuilder) for initialising [`MarketStream`]s of specific data kinds.
//! - [`DynamicStreams`](streams::builder::dynamic::DynamicStreams) for initialising [`MarketStream`]s of every supported data kind at once.
//! - Define what exchange market data you want to stream using the [`Subscription`] type.
//! - Pass [`Subscription`]s to the [`StreamBuilder::subscribe`](streams::builder::StreamBuilder::subscribe) or [`DynamicStreams::init`](streams::builder::dynamic::DynamicStreams::init) methods.
//! - Each call to the [`StreamBuilder::subscribe`](streams::builder::StreamBuilder::subscribe) (or each batch passed to the [`DynamicStreams::init`](streams::builder::dynamic::DynamicStreams::init))
//!   method opens a new WebSocket connection to the exchange - giving you full control.
//!
//! ## Examples
//! For a comprehensive collection of examples, see the /examples directory.
//!
//! ### Multi Exchange Public Trades
//! ```rust,no_run
//! use barter_data::{
//!     exchange::{
//!         gateio::spot::GateioSpot,
//!         binance::{futures::BinanceFuturesUsd, spot::BinanceSpot},
//!         coinbase::Coinbase,
//!         okx::Okx,
//!     },
//!     streams::{Streams, reconnect::stream::ReconnectingStream},
//!     subscription::trade::PublicTrades,
//! };
//! use barter_instrument::instrument::market_data::kind::MarketDataInstrumentKind;
//! use futures::StreamExt;
//! use tracing::warn;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Initialise PublicTrades Streams for various exchanges
//!     // '--> each call to StreamBuilder::subscribe() initialises a separate WebSocket connection
//!
//!     let streams = Streams::<PublicTrades>::builder()
//!         .subscribe([
//!             (BinanceSpot::default(), "btc", "usdt", MarketDataInstrumentKind::Spot, PublicTrades),
//!             (BinanceSpot::default(), "eth", "usdt", MarketDataInstrumentKind::Spot, PublicTrades),
//!         ])
//!         .subscribe([
//!             (BinanceFuturesUsd::default(), "btc", "usdt", MarketDataInstrumentKind::Perpetual, PublicTrades),
//!             (BinanceFuturesUsd::default(), "eth", "usdt", MarketDataInstrumentKind::Perpetual, PublicTrades),
//!         ])
//!         .subscribe([
//!             (Coinbase, "btc", "usd", MarketDataInstrumentKind::Spot, PublicTrades),
//!             (Coinbase, "eth", "usd", MarketDataInstrumentKind::Spot, PublicTrades),
//!         ])
//!         .subscribe([
//!             (GateioSpot::default(), "btc", "usdt", MarketDataInstrumentKind::Spot, PublicTrades),
//!             (GateioSpot::default(), "eth", "usdt", MarketDataInstrumentKind::Spot, PublicTrades),
//!         ])
//!         .subscribe([
//!             (Okx, "btc", "usdt", MarketDataInstrumentKind::Spot, PublicTrades),
//!             (Okx, "eth", "usdt", MarketDataInstrumentKind::Spot, PublicTrades),
//!             (Okx, "btc", "usdt", MarketDataInstrumentKind::Perpetual, PublicTrades),
//!             (Okx, "eth", "usdt", MarketDataInstrumentKind::Perpetual, PublicTrades),
//!        ])
//!         .init()
//!         .await
//!         .unwrap();
//!
//!     // Select and merge every exchange Stream using futures_util::stream::select_all
//!     // Note: use `Streams.select(ExchangeId)` to interact with individual exchange streams!
//!     let mut joined_stream = streams
//!         .select_all()
//!         .with_error_handler(|error| warn!(?error, "MarketStream generated error"));
//!
//!     while let Some(event) = joined_stream.next().await {
//!         println!("{event:?}");
//!     }
//! }
//! ```
#[allow(unused_extern_crates)]
use prost as _;

use crate::{
    error::DataError,
    event::MarketEvent,
    exchange::{Connector, PingInterval},
    instrument::InstrumentData,
    subscriber::{Subscribed, Subscriber},
    subscription::{Subscription, SubscriptionKind},
    transformer::ExchangeTransformer,
};
use async_trait::async_trait;
use barter_instrument::exchange::ExchangeId;
use barter_integration::{
    Transformer,
    error::SocketError,
    protocol::{
        StreamParser,
        websocket::{WsError, WsMessage, WsSink, WsStream},
    },
    stream::ExchangeStream,
};
use futures::{SinkExt, Stream, StreamExt};

use std::{collections::VecDeque, future::Future};
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

/// Barter-Data 中生成的所有 [`Error`](std::error::Error)。
pub mod error;

/// 定义在每个 [`MarketStream`] 中使用的通用 [`MarketEvent<T>`](MarketEvent)。
pub mod event;

/// 每个交易所的 [`Connector`] 实现。
pub mod exchange;

/// 用于从 Barter [`Subscription`] 集合构建 [`MarketStream`] 的高级 API 类型。
pub mod streams;

/// [`Subscriber`]、[`SubscriptionMapper`](subscriber::mapper::SubscriptionMapper) 和
/// [`SubscriptionValidator`](subscriber::validator::SubscriptionValidator) Trait，定义
/// [`Connector`] 如何订阅交易所 [`MarketStream`]。
///
/// 包含用于订阅 WebSocket [`MarketStream`] 的标准实现。
pub mod subscriber;

/// 用于传达要初始化的每个 [`MarketStream`] 类型以及交易所将被转换为什么标准化
/// Barter 输出类型的类型。
pub mod subscription;

/// 用于描述交易对数据的 [`InstrumentData`] Trait。
pub mod instrument;

/// [`OrderBook`](books::OrderBook) 相关类型，以及用于初始化和维护
/// 排序的本地交易对 [`OrderBook`](books::OrderBook) 集合的工具。
pub mod books;

/// [`MarketStream`] 使用的通用 [`ExchangeTransformer`] 实现，用于将交易所特定类型
/// 转换为标准化 Barter 类型。
///
/// 包含适用于大多数 `Exchange`-`SubscriptionKind` 组合的标准
/// [`StatelessTransformer`](transformer::stateless::StatelessTransformer) 实现。
///
/// 需要自定义逻辑的情况，例如在启动时获取初始 [`OrderBooksL2`](subscription::book::OrderBooksL2)
/// 和 [`OrderBooksL3`](subscription::book::OrderBooksL3) 快照，可能需要自定义
/// [`ExchangeTransformer`] 实现。
/// 有关示例，请参见 [`Binance`](exchange::binance::Binance) [`OrderBooksL2`](subscription::book::OrderBooksL2)
/// [`ExchangeTransformer`] 实现：
/// [`spot`](exchange::binance::spot::l2::BinanceSpotOrderBooksL2Transformer) 和
/// [`futures_usd`](exchange::binance::futures::l2::BinanceFuturesUsdOrderBooksL2Transformer)。
pub mod transformer;

/// 使用 tungstenite [`WebSocket`](barter_integration::protocol::websocket::WebSocket)
/// 的 [`ExchangeStream`] 的便捷类型别名。
pub type ExchangeWsStream<Parser, Transformer> = ExchangeStream<Parser, WsStream, Transformer>;

/// 为实现者定义通用标识类型。
///
/// Identifier Trait 用于从对象中提取标识符。
pub trait Identifier<T> {
    /// 获取标识符。
    ///
    /// # 返回值
    ///
    /// 返回标识符。
    fn id(&self) -> T;
}

/// 产生 [`Market<Kind>`](MarketEvent) 事件的 [`Stream`]。
///
/// [`Market<Kind>`](MarketEvent) 的类型取决于传递的 [`Subscription`] 的 [`SubscriptionKind`]。
///
/// MarketStream Trait 定义了市场数据流的标准接口。
///
/// ## 类型参数
///
/// - `Exchange`: 交易所类型，必须实现 `Connector`
/// - `Instrument`: 交易对类型，必须实现 `InstrumentData`
/// - `Kind`: 订阅类型，必须实现 `SubscriptionKind`
#[async_trait]
pub trait MarketStream<Exchange, Instrument, Kind>
where
    Self: Stream<Item = Result<MarketEvent<Instrument::Key, Kind::Event>, DataError>>
        + Send
        + Sized
        + Unpin,
    Exchange: Connector,
    Instrument: InstrumentData,
    Kind: SubscriptionKind,
{
    /// 从订阅列表初始化市场流。
    ///
    /// ## 类型参数
    ///
    /// - `SnapFetcher`: 快照获取器类型，用于获取初始快照
    ///
    /// # 参数
    ///
    /// - `subscriptions`: 订阅列表
    ///
    /// # 返回值
    ///
    /// 返回初始化的市场流，如果出错则返回错误。
    async fn init<SnapFetcher>(
        subscriptions: &[Subscription<Exchange, Instrument, Kind>],
    ) -> Result<Self, DataError>
    where
        SnapFetcher: SnapshotFetcher<Exchange, Kind>,
        Subscription<Exchange, Instrument, Kind>:
            Identifier<Exchange::Channel> + Identifier<Exchange::Market>;
}

/// 定义如何为 [`Subscription`] 集合获取市场数据快照。
///
/// 当 [`MarketStream`] 在启动时需要初始快照时很有用。
///
/// ## 使用场景
///
/// - OrderBooksL2 和 OrderBooksL3 需要初始订单簿快照
/// - 某些交易所要求在订阅前获取当前状态
///
/// ## 示例
///
/// 参见 Binance OrderBooksL2 示例：<br>
/// - [`BinanceSpotOrderBooksL2SnapshotFetcher`](exchange::binance::spot::l2::BinanceSpotOrderBooksL2SnapshotFetcher)
/// - [`BinanceFuturesUsdOrderBooksL2SnapshotFetcher`](exchange::binance::futures::l2::BinanceFuturesUsdOrderBooksL2SnapshotFetcher)
///
/// ## 类型参数
///
/// - `Exchange`: 交易所类型
/// - `Kind`: 订阅类型
pub trait SnapshotFetcher<Exchange, Kind> {
    /// 获取市场数据快照。
    ///
    /// ## 类型参数
    ///
    /// - `Instrument`: 交易对类型
    ///
    /// # 参数
    ///
    /// - `subscriptions`: 订阅列表
    ///
    /// # 返回值
    ///
    /// 返回市场事件列表，如果出错则返回错误。
    fn fetch_snapshots<Instrument>(
        subscriptions: &[Subscription<Exchange, Instrument, Kind>],
    ) -> impl Future<Output = Result<Vec<MarketEvent<Instrument::Key, Kind::Event>>, SocketError>> + Send
    where
        Exchange: Connector,
        Instrument: InstrumentData,
        Kind: SubscriptionKind,
        Kind::Event: Send,
        Subscription<Exchange, Instrument, Kind>: Identifier<Exchange::Market>;
}

#[async_trait]
impl<Exchange, Instrument, Kind, Transformer, Parser> MarketStream<Exchange, Instrument, Kind>
    for ExchangeWsStream<Parser, Transformer>
where
    Exchange: Connector + Send + Sync,
    Instrument: InstrumentData,
    Kind: SubscriptionKind + Send + Sync,
    Transformer: ExchangeTransformer<Exchange, Instrument::Key, Kind> + Send,
    Kind::Event: Send,
    Parser: StreamParser<Transformer::Input, Message = WsMessage, Error = WsError> + Send,
{
    async fn init<SnapFetcher>(
        subscriptions: &[Subscription<Exchange, Instrument, Kind>],
    ) -> Result<Self, DataError>
    where
        SnapFetcher: SnapshotFetcher<Exchange, Kind>,
        Subscription<Exchange, Instrument, Kind>:
            Identifier<Exchange::Channel> + Identifier<Exchange::Market>,
    {
        // 连接并订阅
        let Subscribed {
            websocket,
            map: instrument_map,
            buffered_websocket_events,
        } = Exchange::Subscriber::subscribe(subscriptions).await?;

        // 获取任何所需的初始 MarketEvent 快照
        let initial_snapshots = SnapFetcher::fetch_snapshots(subscriptions).await?;

        // 将 WebSocket 拆分为 WsStream 和 WsSink 组件
        let (ws_sink, ws_stream) = websocket.split();

        // 生成任务以将 Transformer 消息（例如自定义 pong）分发到交易所
        let (ws_sink_tx, ws_sink_rx) = mpsc::unbounded_channel();
        tokio::spawn(distribute_messages_to_exchange(
            Exchange::ID,
            ws_sink,
            ws_sink_rx,
        ));

        // 生成可选任务以将自定义应用级 ping 分发到交易所
        if let Some(ping_interval) = Exchange::ping_interval() {
            tokio::spawn(schedule_pings_to_exchange(
                Exchange::ID,
                ws_sink_tx.clone(),
                ping_interval,
            ));
        }

        // 初始化与此交易所和 SubscriptionKind 关联的 Transformer
        let mut transformer =
            Transformer::init(instrument_map, &initial_snapshots, ws_sink_tx).await?;

        // 处理在订阅验证期间接收到的任何缓冲的活动订阅事件
        let mut processed = process_buffered_events::<Parser, Transformer>(
            &mut transformer,
            buffered_websocket_events,
        );

        // 使用任何初始快照事件扩展缓冲事件
        processed.extend(initial_snapshots.into_iter().map(Ok));

        Ok(ExchangeWsStream::new(ws_stream, transformer, processed))
    }
}

/// [`SnapshotFetcher`] 的实现，不获取任何初始市场数据快照。
///
/// 通常用于无状态的 [`MarketStream`]，例如公共交易。
///
/// NoInitialSnapshots 是一个空实现，适用于不需要初始快照的订阅类型。
#[derive(Debug)]
pub struct NoInitialSnapshots;

impl<Exchange, Kind> SnapshotFetcher<Exchange, Kind> for NoInitialSnapshots {
    /// 不获取任何快照，返回空列表。
    fn fetch_snapshots<Instrument>(
        _: &[Subscription<Exchange, Instrument, Kind>],
    ) -> impl Future<Output = Result<Vec<MarketEvent<Instrument::Key, Kind::Event>>, SocketError>> + Send
    where
        Exchange: Connector,
        Instrument: InstrumentData,
        Kind: SubscriptionKind,
        Kind::Event: Send,
        Subscription<Exchange, Instrument, Kind>: Identifier<Exchange::Market>,
    {
        std::future::ready(Ok(vec![]))
    }
}

pub fn process_buffered_events<Parser, StreamTransformer>(
    transformer: &mut StreamTransformer,
    events: Vec<Parser::Message>,
) -> VecDeque<Result<StreamTransformer::Output, StreamTransformer::Error>>
where
    Parser: StreamParser<StreamTransformer::Input>,
    StreamTransformer: Transformer,
{
    events
        .into_iter()
        .filter_map(|event| {
            Parser::parse(Ok(event))?
                .inspect_err(|error| {
                    warn!(
                        ?error,
                        "failed to parse message buffered during Subscription validation"
                    )
                })
                .ok()
        })
        .flat_map(|parsed| transformer.transform(parsed))
        .collect()
}

/// 将通过 [`ExchangeTransformer`] 发送的 [`WsMessage`] 通过 [`WsSink`] 传输到交易所。
///
/// **注意：**
/// ExchangeTransformer 在同步 Trait 上下文中运行，因此我们使用此单独任务
/// 来避免向 transformer 添加 `#[async_trait]` - 这避免了分配。
///
/// # 参数
///
/// - `exchange`: 交易所标识
/// - `ws_sink`: WebSocket 发送端
/// - `ws_sink_rx`: 消息接收通道
pub async fn distribute_messages_to_exchange(
    exchange: ExchangeId,
    mut ws_sink: WsSink,
    mut ws_sink_rx: mpsc::UnboundedReceiver<WsMessage>,
) {
    while let Some(message) = ws_sink_rx.recv().await {
        if let Err(error) = ws_sink.send(message).await {
            if barter_integration::protocol::websocket::is_websocket_disconnected(&error) {
                break;
            }

            // 仅在通过已连接的 WebSocket 发送 WsMessage 失败时记录错误
            error!(
                %exchange,
                %error,
                "failed to send output message to the exchange via WsSink"
            );
        }
    }
}

/// 使用提供的 [`PingInterval`] 安排向交易所发送自定义应用级 ping [`WsMessage`]。
///
/// **注意事项：**
///  - 这仅用于需要自定义应用级 ping 的交易所。
///  - 这是对 `tokio_tungstenite` 已处理的协议级 ping 的补充。
///
/// # 参数
///
/// - `exchange`: 交易所标识
/// - `ws_sink_tx`: 消息发送通道
/// - `PingInterval`: Ping 间隔配置
pub async fn schedule_pings_to_exchange(
    exchange: ExchangeId,
    ws_sink_tx: mpsc::UnboundedSender<WsMessage>,
    PingInterval { mut interval, ping }: PingInterval,
) {
    loop {
        // 等待下一个计划的 ping
        interval.tick().await;

        // 构建交易所自定义应用级 ping 负载
        let payload = ping();
        debug!(%exchange, %payload, "sending custom application-level ping to exchange");

        if ws_sink_tx.send(payload).is_err() {
            break;
        }
    }
}

pub mod test_utils {
    use crate::{
        event::{DataKind, MarketEvent},
        subscription::trade::PublicTrade,
    };
    use barter_instrument::{Side, exchange::ExchangeId};
    use chrono::{DateTime, Utc};

    pub fn market_event_trade_buy<InstrumentKey>(
        time_exchange: DateTime<Utc>,
        time_received: DateTime<Utc>,
        instrument: InstrumentKey,
        price: f64,
        quantity: f64,
    ) -> MarketEvent<InstrumentKey, DataKind> {
        MarketEvent {
            time_exchange,
            time_received,
            exchange: ExchangeId::BinanceSpot,
            instrument,
            kind: DataKind::Trade(PublicTrade {
                id: "trade_id".to_string(),
                price,
                amount: quantity,
                side: Side::Buy,
            }),
        }
    }
}
