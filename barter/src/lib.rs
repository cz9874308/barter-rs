#![forbid(unsafe_code)]
#![warn(
    unused,
    clippy::cognitive_complexity,
    unused_crate_dependencies,
    unused_extern_crates,
    clippy::unused_self,
    clippy::useless_let_if_seq,
    missing_debug_implementations,
    rust_2018_idioms,
    rust_2024_compatibility
)]
#![allow(clippy::type_complexity, clippy::too_many_arguments, type_alias_bounds)]

//! # Barter
//! Barter 核心是一个用于构建高性能实盘交易、模拟交易和回测系统的 Rust 框架。
//! * **快速**：使用原生 Rust 编写。最小化分配。具有直接索引查找的数据导向状态管理系统。
//! * **健壮**：强类型。线程安全。广泛的测试覆盖。
//! * **可定制**：即插即用的 Strategy 和 RiskManager 组件，支持大多数交易策略（做市、统计套利、高频交易等）。
//! * **可扩展**：采用模块化设计的多线程架构。利用 Tokio 进行 I/O。内存高效的数据结构。
//!
//! ## 概述
//! Barter 核心是一个用于构建专业级实盘交易、模拟交易和回测系统的 Rust 框架。核心 Engine（引擎）
//! 支持同时在多个交易所执行交易，并提供运行大多数类型交易策略的灵活性。它允许开启/关闭算法订单生成，
//! 并可以执行从外部进程发出的命令（例如 CloseAllPositions、OpenOrders、CancelOrders 等）。
//!
//! 从高层次来看，它提供了几个主要组件：
//! * 具有即插即用 Strategy 和 RiskManager 组件的 Engine。
//! * 使用索引数据结构进行 O(1) 常量查找的集中式缓存友好 EngineState 管理。
//! * 用于自定义 Engine 行为的 Strategy 接口（AlgoStrategy、ClosePositionsStrategy、OnDisconnectStrategy 等）。
//! * 用于定义检查生成的算法订单的自定义风险逻辑的 RiskManager 接口。
//! * 允许从外部进程发出命令（例如 CloseAllPositions、OpenOrders、CancelOrders 等）以及开启/关闭算法交易的事件驱动系统。
//! * 提供关键性能指标摘要的综合统计包（PnL、Sharpe、Sortino、Drawdown 等）。
//!
//! ## 通过 Engine 示例快速开始
//! [查看 Engine 示例](https://github.com/barter-rs/barter-rs/tree/feat/docs_tests_readmes_examples/barter/examples)

use crate::{
    engine::{command::Command, state::trading::TradingState},
    execution::AccountStreamEvent,
};
use barter_data::{
    event::{DataKind, MarketEvent},
    streams::consumer::MarketStreamEvent,
};
use barter_execution::AccountEvent;
use barter_instrument::{asset::AssetIndex, exchange::ExchangeIndex, instrument::InstrumentIndex};
use barter_integration::Terminal;
use chrono::{DateTime, Utc};
use derive_more::{Constructor, From};
use serde::{Deserialize, Serialize};
use shutdown::Shutdown;

/// 算法交易 Engine（引擎），以及处理输入事件的入口点。
///
/// 例如：`Engine`、`run`、`process_with_audit` 等。
pub mod engine;

/// 定义 Barter 核心中所有可能的错误。
pub mod error;

/// 用于初始化多交易所执行、路由 ExecutionRequest 和其他执行逻辑的组件。
pub mod execution;

/// 提供 Barter 核心的默认 Tracing 日志初始化器。
pub mod logging;

/// RiskManager 接口，用于审查并可选地过滤算法取消和开仓订单请求。
pub mod risk;

/// 用于分析数据集、金融指标和金融摘要的统计算法。
///
/// 例如：`TradingSummary`、`TearSheet`、`SharpeRatio` 等。
pub mod statistic;

/// Strategy 接口，用于生成算法订单、平仓，以及在断开连接/交易禁用时执行 Engine 操作。
pub mod strategy;

/// 用于初始化和与完整交易系统交互的工具。
pub mod system;

/// 回测工具。
pub mod backtest;

/// 与组件关闭相关的 Trait 和类型。
pub mod shutdown;

/// 带时间戳的值。
///
/// 用于将任意值与 UTC 时间戳关联，常用于记录事件发生时间或数据更新时间。
///
/// # 类型参数
///
/// - `T`: 值的类型
///
/// # 字段
///
/// - `value`: 存储的值
/// - `time`: UTC 时间戳
///
/// # 使用示例
///
/// ```rust,ignore
/// let timed_price = Timed::new(100.0, Utc::now());
/// println!("价格: {}, 时间: {}", timed_price.value, timed_price.time);
/// ```
#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    Deserialize,
    Serialize,
    Constructor,
)]
pub struct Timed<T> {
    /// 存储的值
    pub value: T,
    /// UTC 时间戳
    pub time: DateTime<Utc>,
}

/// 默认的 [`Engine`](engine::Engine) 事件，包含市场事件、账户/执行事件和 Engine 命令。
///
/// EngineEvent 是 Engine 处理的所有事件类型的统一枚举。它允许 Engine 处理来自不同来源的事件，
/// 包括实时市场数据、账户更新、外部命令等。
///
/// # 类型参数
///
/// - `MarketKind`: 市场数据类型（默认为 `DataKind`）
/// - `ExchangeKey`: 交易所标识类型（默认为 `ExchangeIndex`）
/// - `AssetKey`: 资产标识类型（默认为 `AssetIndex`）
/// - `InstrumentKey`: 交易工具标识类型（默认为 `InstrumentIndex`）
///
/// # 变体
///
/// - `Shutdown`: 关闭事件，用于优雅地关闭 Engine
/// - `Command`: Engine 命令，用于从外部进程控制 Engine（如平仓、取消订单等）
/// - `TradingStateUpdate`: 交易状态更新，用于开启/关闭算法交易
/// - `Account`: 账户事件，包含账户余额、订单状态、交易执行等更新
/// - `Market`: 市场事件，包含市场数据更新（如价格、订单簿等）
///
/// # 注意事项
///
/// Engine 可以配置为处理自定义事件类型，不一定需要使用此默认事件类型。
///
/// # 使用示例
///
/// ```rust,ignore
/// // 创建市场事件
/// let market_event = EngineEvent::Market(market_stream_event);
///
/// // 创建关闭事件
/// let shutdown_event = EngineEvent::shutdown();
///
/// // 处理事件
/// engine.process(market_event)?;
/// ```
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, From)]
pub enum EngineEvent<
    MarketKind = DataKind,
    ExchangeKey = ExchangeIndex,
    AssetKey = AssetIndex,
    InstrumentKey = InstrumentIndex,
> {
    /// 关闭事件，用于优雅地关闭 Engine
    Shutdown(Shutdown),
    /// Engine 命令，用于从外部进程控制 Engine
    Command(Command<ExchangeKey, AssetKey, InstrumentKey>),
    /// 交易状态更新，用于开启/关闭算法交易
    TradingStateUpdate(TradingState),
    /// 账户事件，包含账户余额、订单状态、交易执行等更新
    Account(AccountStreamEvent<ExchangeKey, AssetKey, InstrumentKey>),
    /// 市场事件，包含市场数据更新（如价格、订单簿等）
    Market(MarketStreamEvent<InstrumentKey, MarketKind>),
}

impl<MarketKind, ExchangeKey, AssetKey, InstrumentKey> Terminal
    for EngineEvent<MarketKind, ExchangeKey, AssetKey, InstrumentKey>
{
    fn is_terminal(&self) -> bool {
        matches!(self, Self::Shutdown(_))
    }
}

impl<MarketKind, ExchangeKey, AssetKey, InstrumentKey>
    EngineEvent<MarketKind, ExchangeKey, AssetKey, InstrumentKey>
{
    /// 创建一个关闭事件。
    ///
    /// 用于优雅地关闭 Engine。当 Engine 处理此事件时，会停止处理新事件并执行清理操作。
    ///
    /// # 返回值
    ///
    /// 返回一个 `Shutdown` 类型的 `EngineEvent`。
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let shutdown_event = EngineEvent::shutdown();
    /// engine.process(shutdown_event)?;
    /// ```
    pub fn shutdown() -> Self {
        Self::Shutdown(Shutdown)
    }
}

impl<MarketKind, ExchangeKey, AssetKey, InstrumentKey>
    From<AccountEvent<ExchangeKey, AssetKey, InstrumentKey>>
    for EngineEvent<MarketKind, ExchangeKey, AssetKey, InstrumentKey>
{
    fn from(value: AccountEvent<ExchangeKey, AssetKey, InstrumentKey>) -> Self {
        Self::Account(AccountStreamEvent::Item(value))
    }
}

impl<MarketKind, ExchangeKey, AssetKey, InstrumentKey> From<MarketEvent<InstrumentKey, MarketKind>>
    for EngineEvent<MarketKind, ExchangeKey, AssetKey, InstrumentKey>
{
    fn from(value: MarketEvent<InstrumentKey, MarketKind>) -> Self {
        Self::Market(MarketStreamEvent::Item(value))
    }
}

/// 单调递增的事件序列号。
///
/// 用于跟踪 Engine 事件处理的顺序。每个事件都会被分配一个唯一的序列号，
/// 确保事件处理的顺序性和可追溯性。
///
/// # 工作原理
///
/// Sequence 内部维护一个 u64 类型的计数器，每次调用 `fetch_add` 时会返回当前值并递增。
/// 这确保了每个事件都有唯一的、单调递增的序列号。
///
/// # 使用场景
///
/// - 事件排序和去重
/// - 事件处理顺序验证
/// - 审计和日志记录
/// - 调试和问题追踪
///
/// # 使用示例
///
/// ```rust,ignore
/// let mut sequence = Sequence::new(0);
///
/// // 获取下一个序列号
/// let seq1 = sequence.fetch_add(); // 返回 Sequence(0)，内部值变为 1
/// let seq2 = sequence.fetch_add(); // 返回 Sequence(1)，内部值变为 2
///
/// // 获取当前值（不递增）
/// let current = sequence.value(); // 返回 2
/// ```
#[derive(
    Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, Constructor,
)]
pub struct Sequence(pub u64);

impl Sequence {
    /// 获取序列号的当前值。
    ///
    /// # 返回值
    ///
    /// 返回序列号的当前值（u64 类型）。
    pub fn value(&self) -> u64 {
        self.0
    }

    /// 获取当前序列号并递增。
    ///
    /// 这是一个原子操作：先返回当前值，然后将内部计数器加 1。
    /// 类似于 `fetch_add` 原子操作的行为。
    ///
    /// # 返回值
    ///
    /// 返回递增前的序列号值。
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let mut seq = Sequence::new(0);
    /// let first = seq.fetch_add();  // 返回 Sequence(0)，seq 内部变为 1
    /// let second = seq.fetch_add(); // 返回 Sequence(1)，seq 内部变为 2
    /// ```
    pub fn fetch_add(&mut self) -> Sequence {
        let sequence = *self;
        self.0 += 1;
        sequence
    }
}

/// Barter 核心测试工具。
///
/// 提供用于测试的辅助函数和工具，包括时间操作、浮点数比较、测试数据生成等。
pub mod test_utils {
    use crate::{
        Timed, engine::state::asset::AssetState, statistic::summary::asset::TearSheetAssetGenerator,
    };
    use barter_execution::{
        balance::Balance,
        order::id::{OrderId, StrategyId},
        trade::{AssetFees, Trade, TradeId},
    };
    use barter_instrument::{
        Side, asset::QuoteAsset, instrument::name::InstrumentNameInternal, test_utils::asset,
    };
    use chrono::{DateTime, Days, TimeDelta, Utc};
    use rust_decimal::Decimal;

    /// 比较两个 f64 浮点数是否相等（考虑 NaN 和无穷大）。
    ///
    /// 由于浮点数的精度问题，直接使用 `==` 比较可能不准确。此函数使用 epsilon 容差进行比较，
    /// 并正确处理 NaN 和无穷大的特殊情况。
    ///
    /// # 参数
    ///
    /// - `actual`: 实际值
    /// - `expected`: 期望值
    /// - `epsilon`: 允许的最大误差
    ///
    /// # 返回值
    ///
    /// 如果两个值在容差范围内相等，返回 `true`；否则返回 `false`。
    ///
    /// # 特殊情况处理
    ///
    /// - 两个 NaN 值被视为相等
    /// - 两个同符号的无穷大值被视为相等
    /// - 其他包含 NaN 或无穷大的情况返回 `false`
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// assert!(f64_is_eq(0.1 + 0.2, 0.3, 1e-10));
    /// assert!(f64_is_eq(f64::NAN, f64::NAN, 1e-10)); // true
    /// ```
    pub fn f64_is_eq(actual: f64, expected: f64, epsilon: f64) -> bool {
        if actual.is_nan() && expected.is_nan() {
            true
        } else if actual.is_infinite() && expected.is_infinite() {
            actual.is_sign_positive() == expected.is_sign_positive()
        } else if actual.is_nan()
            || expected.is_nan()
            || actual.is_infinite()
            || expected.is_infinite()
        {
            false
        } else {
            (actual - expected).abs() < epsilon
        }
    }

    /// 在基础时间上增加指定的天数。
    ///
    /// # 参数
    ///
    /// - `base`: 基础时间
    /// - `plus`: 要增加的天数
    ///
    /// # 返回值
    ///
    /// 返回增加天数后的新时间。
    ///
    /// # Panics
    ///
    /// 如果时间溢出，此函数会 panic。
    pub fn time_plus_days(base: DateTime<Utc>, plus: u64) -> DateTime<Utc> {
        base.checked_add_days(Days::new(plus)).unwrap()
    }

    /// 在基础时间上增加指定的秒数。
    ///
    /// # 参数
    ///
    /// - `base`: 基础时间
    /// - `plus`: 要增加的秒数（可以为负数）
    ///
    /// # 返回值
    ///
    /// 返回增加秒数后的新时间。
    ///
    /// # Panics
    ///
    /// 如果时间溢出，此函数会 panic。
    pub fn time_plus_secs(base: DateTime<Utc>, plus: i64) -> DateTime<Utc> {
        base.checked_add_signed(TimeDelta::seconds(plus)).unwrap()
    }

    /// 在基础时间上增加指定的毫秒数。
    ///
    /// # 参数
    ///
    /// - `base`: 基础时间
    /// - `plus`: 要增加的毫秒数（可以为负数）
    ///
    /// # 返回值
    ///
    /// 返回增加毫秒数后的新时间。
    ///
    /// # Panics
    ///
    /// 如果时间溢出，此函数会 panic。
    pub fn time_plus_millis(base: DateTime<Utc>, plus: i64) -> DateTime<Utc> {
        base.checked_add_signed(TimeDelta::milliseconds(plus))
            .unwrap()
    }

    /// 在基础时间上增加指定的微秒数。
    ///
    /// # 参数
    ///
    /// - `base`: 基础时间
    /// - `plus`: 要增加的微秒数（可以为负数）
    ///
    /// # 返回值
    ///
    /// 返回增加微秒数后的新时间。
    ///
    /// # Panics
    ///
    /// 如果时间溢出，此函数会 panic。
    pub fn time_plus_micros(base: DateTime<Utc>, plus: i64) -> DateTime<Utc> {
        base.checked_add_signed(TimeDelta::microseconds(plus))
            .unwrap()
    }

    /// 创建一个测试用的 Trade（交易）对象。
    ///
    /// 用于测试场景中快速创建 Trade 实例，所有字段都使用默认或提供的值。
    ///
    /// # 参数
    ///
    /// - `time_exchange`: 交易所时间戳
    /// - `side`: 交易方向（买入/卖出）
    /// - `price`: 交易价格
    /// - `quantity`: 交易数量
    /// - `fees`: 手续费
    ///
    /// # 返回值
    ///
    /// 返回一个 `Trade` 实例，包含提供的交易信息。
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let trade = trade(
    ///     Utc::now(),
    ///     Side::Buy,
    ///     100.0,
    ///     1.0,
    ///     0.001
    /// );
    /// ```
    pub fn trade(
        time_exchange: DateTime<Utc>,
        side: Side,
        price: f64,
        quantity: f64,
        fees: f64,
    ) -> Trade<QuoteAsset, InstrumentNameInternal> {
        Trade {
            id: TradeId::new("trade_id"),
            order_id: OrderId::new("order_id"),
            instrument: InstrumentNameInternal::new("instrument"),
            strategy: StrategyId::new("strategy"),
            time_exchange,
            side,
            price: price.try_into().unwrap(),
            quantity: quantity.try_into().unwrap(),
            fees: AssetFees {
                asset: QuoteAsset,
                fees: fees.try_into().unwrap(),
            },
        }
    }

    /// 创建一个测试用的 AssetState（资产状态）对象。
    ///
    /// 用于测试场景中快速创建 AssetState 实例，包含资产余额和统计信息。
    ///
    /// # 参数
    ///
    /// - `symbol`: 资产符号（如 "BTC", "USDT"）
    /// - `balance_total`: 总余额
    /// - `balance_free`: 可用余额
    /// - `time_exchange`: 交易所时间戳
    ///
    /// # 返回值
    ///
    /// 返回一个 `AssetState` 实例，包含资产信息和初始化的统计信息。
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let asset_state = asset_state(
    ///     "BTC",
    ///     1.0,
    ///     0.5,
    ///     Utc::now()
    /// );
    /// ```
    pub fn asset_state(
        symbol: &str,
        balance_total: f64,
        balance_free: f64,
        time_exchange: DateTime<Utc>,
    ) -> AssetState {
        let balance = Timed::new(
            Balance::new(
                Decimal::try_from(balance_total).unwrap(),
                Decimal::try_from(balance_free).unwrap(),
            ),
            time_exchange,
        );

        AssetState {
            asset: asset(symbol),
            balance: Some(balance),
            statistics: TearSheetAssetGenerator::init(&balance),
        }
    }
}
