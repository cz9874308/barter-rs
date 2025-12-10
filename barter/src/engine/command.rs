//! Engine 命令模块
//!
//! 本模块定义了 Engine 可以执行的各种交易命令。这些命令通常由外部进程发送给 Engine，
//! 用于控制交易行为，如发送订单、取消订单、平仓等。
//!
//! # 核心概念
//!
//! - **Command**: 命令枚举，定义了所有 Engine 支持的命令类型
//! - **外部控制**: 命令由外部进程发送，允许动态控制 Engine 的交易行为
//! - **批量操作**: 支持单个或批量操作（通过 `OneOrMany` 和 `InstrumentFilter`）
//!
//! # 使用场景
//!
//! - **策略控制**: 策略可以通过命令控制 Engine 的交易行为
//! - **风险管理**: 风险管理系统可以通过命令强制平仓或取消订单
//! - **手动干预**: 交易员可以通过命令手动干预交易
//! - **系统集成**: 外部系统可以通过命令与 Engine 交互
//!
//! # 工作原理
//!
//! 1. 外部进程创建 Command 并发送给 Engine
//! 2. Engine 接收命令并解析
//! 3. Engine 根据命令类型执行相应的操作
//! 4. 操作结果通过 EngineEvent 返回

use crate::engine::state::instrument::filter::InstrumentFilter;
use barter_execution::order::request::{OrderRequestCancel, OrderRequestOpen};
use barter_instrument::{asset::AssetIndex, exchange::ExchangeIndex, instrument::InstrumentIndex};
use barter_integration::collection::one_or_many::OneOrMany;
use serde::{Deserialize, Serialize};

/// 交易相关命令，由外部进程发送给 [`Engine`](super::Engine) 执行。
///
/// Command 枚举定义了所有 Engine 支持的命令类型。这些命令允许外部进程动态控制
/// Engine 的交易行为，如发送订单、取消订单、平仓等。
///
/// ## 为什么需要命令？
///
/// Engine 需要支持外部控制，以便：
///
/// - **策略控制**: 策略可以根据市场条件动态发送命令
/// - **风险管理**: 风险管理系统可以强制平仓或取消订单
///
/// ## 类型参数
///
/// - `ExchangeKey`: 交易所索引类型，默认为 `ExchangeIndex`
/// - `AssetKey`: 资产索引类型，默认为 `AssetIndex`
/// - `InstrumentKey`: 交易对索引类型，默认为 `InstrumentIndex`
///
/// ## 命令类型
///
/// 1. **SendCancelRequests**: 发送取消订单请求
/// 2. **SendOpenRequests**: 发送开仓订单请求
/// 3. **ClosePositions**: 平仓（根据过滤器筛选）
/// 4. **CancelOrders**: 取消订单（根据过滤器筛选）
///
/// ## 使用场景
///
/// - 策略根据市场条件发送订单
/// - 风险管理系统强制平仓
/// - 交易员手动干预交易
/// - 外部系统集成
///
/// # 使用示例
///
/// ```rust,ignore
/// // 发送单个取消订单请求
/// let command = Command::SendCancelRequests(
///     OneOrMany::One(OrderRequestCancel { ... })
/// );
///
/// // 发送多个开仓订单请求
/// let command = Command::SendOpenRequests(
///     OneOrMany::Many(vec![
///         OrderRequestOpen { ... },
///         OrderRequestOpen { ... },
///     ])
/// );
///
/// // 平仓所有 BTC/USD 仓位
/// let command = Command::ClosePositions(
///     InstrumentFilter::new()
///         .with_instrument(instrument_index)
/// );
///
/// // 取消所有未成交订单
/// let command = Command::CancelOrders(
///     InstrumentFilter::new()
///         .with_exchange(exchange_index)
/// );
/// ```
#[derive(Debug, Clone, PartialEq, PartialOrd, Deserialize, Serialize)]
pub enum Command<
    ExchangeKey = ExchangeIndex,
    AssetKey = AssetIndex,
    InstrumentKey = InstrumentIndex,
> {
    /// 发送取消订单请求。
    ///
    /// 此命令用于向交易所发送取消订单请求。可以发送单个或多个取消请求。
    ///
    /// # 参数
    ///
    /// - `OneOrMany<OrderRequestCancel>`: 单个或多个取消订单请求
    ///
    /// # 使用场景
    ///
    /// - 策略决定取消某个订单
    /// - 风险管理系统强制取消订单
    /// - 订单超时自动取消
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// // 取消单个订单
    /// let command = Command::SendCancelRequests(
    ///     OneOrMany::One(OrderRequestCancel {
    ///         exchange: exchange_index,
    ///         instrument: instrument_index,
    ///         order_id: "order_123".to_string(),
    ///     })
    /// );
    ///
    /// // 批量取消订单
    /// let command = Command::SendCancelRequests(
    ///     OneOrMany::Many(vec![
    ///         OrderRequestCancel { ... },
    ///         OrderRequestCancel { ... },
    ///     ])
    /// );
    /// ```
    SendCancelRequests(OneOrMany<OrderRequestCancel<ExchangeKey, InstrumentKey>>),

    /// 发送开仓订单请求。
    ///
    /// 此命令用于向交易所发送开仓订单请求。可以发送单个或多个开仓请求。
    ///
    /// # 参数
    ///
    /// - `OneOrMany<OrderRequestOpen>`: 单个或多个开仓订单请求
    ///
    /// # 使用场景
    ///
    /// - 策略根据信号发送买入/卖出订单
    /// - 手动下单
    /// - 批量下单
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// // 发送单个开仓订单
    /// let command = Command::SendOpenRequests(
    ///     OneOrMany::One(OrderRequestOpen {
    ///         exchange: exchange_index,
    ///         instrument: instrument_index,
    ///         side: Side::Buy,
    ///         quantity: 1.0,
    ///         price: Some(50000.0),
    ///         order_type: OrderType::Limit,
    ///     })
    /// );
    ///
    /// // 批量发送开仓订单
    /// let command = Command::SendOpenRequests(
    ///     OneOrMany::Many(vec![
    ///         OrderRequestOpen { ... },
    ///         OrderRequestOpen { ... },
    ///     ])
    /// );
    /// ```
    SendOpenRequests(OneOrMany<OrderRequestOpen<ExchangeKey, InstrumentKey>>),

    /// 平仓命令，根据过滤器筛选要平仓的仓位。
    ///
    /// 此命令用于平仓，可以根据交易所、资产、交易对等条件筛选要平仓的仓位。
    ///
    /// # 参数
    ///
    /// - `InstrumentFilter`: 过滤器，用于筛选要平仓的仓位
    ///
    /// # 使用场景
    ///
    /// - 风险管理系统强制平仓
    /// - 策略决定平仓
    /// - 手动平仓
    /// - 止损/止盈触发平仓
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// // 平仓所有 BTC/USD 仓位
    /// let command = Command::ClosePositions(
    ///     InstrumentFilter::new()
    ///         .with_instrument(btc_usd_index)
    /// );
    ///
    /// // 平仓某个交易所的所有仓位
    /// let command = Command::ClosePositions(
    ///     InstrumentFilter::new()
    ///         .with_exchange(binance_index)
    /// );
    ///
    /// // 平仓所有仓位
    /// let command = Command::ClosePositions(
    ///     InstrumentFilter::new()
    /// );
    /// ```
    ClosePositions(InstrumentFilter<ExchangeKey, AssetKey, InstrumentKey>),

    /// 取消订单命令，根据过滤器筛选要取消的订单。
    ///
    /// 此命令用于取消订单，可以根据交易所、资产、交易对等条件筛选要取消的订单。
    ///
    /// # 参数
    ///
    /// - `InstrumentFilter`: 过滤器，用于筛选要取消的订单
    ///
    /// # 使用场景
    ///
    /// - 风险管理系统强制取消订单
    /// - 策略决定取消所有未成交订单
    /// - 手动取消订单
    /// - 市场条件变化导致取消订单
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// // 取消某个交易对的所有未成交订单
    /// let command = Command::CancelOrders(
    ///     InstrumentFilter::new()
    ///         .with_instrument(btc_usd_index)
    /// );
    ///
    /// // 取消某个交易所的所有未成交订单
    /// let command = Command::CancelOrders(
    ///     InstrumentFilter::new()
    ///         .with_exchange(binance_index)
    /// );
    ///
    /// // 取消所有未成交订单
    /// let command = Command::CancelOrders(
    ///     InstrumentFilter::new()
    /// );
    /// ```
    CancelOrders(InstrumentFilter<ExchangeKey, AssetKey, InstrumentKey>),
}
