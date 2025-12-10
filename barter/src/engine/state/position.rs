//! Engine 仓位管理模块
//!
//! 本模块定义了仓位（Position）的数据结构和相关管理逻辑。仓位表示在特定交易对上的
//! 持仓状态，包括持仓方向、数量、平均入场价格、盈亏等信息。
//!
//! # 核心概念
//!
//! - **Position**: 当前持仓，表示在特定交易对上的开仓状态
//! - **PositionManager**: 仓位管理器，管理当前仓位
//! - **PositionExited**: 已平仓的仓位，包含完整的交易历史
//! - **PnL**: 盈亏计算（已实现盈亏和未实现盈亏）
//!
//! # 仓位操作
//!
//! - **开仓**: 从交易创建新仓位
//! - **加仓**: 同方向交易增加仓位
//! - **减仓**: 反向交易减少仓位
//! - **平仓**: 完全关闭仓位
//! - **翻仓**: 关闭当前仓位并开立反向仓位
//!
//! # 盈亏计算
//!
//! - **已实现盈亏（PnL Realised）**: 已平仓部分的盈亏
//! - **未实现盈亏（PnL Unrealised）**: 当前持仓的估算盈亏
//! - **手续费**: 入场和出场手续费分别计算

use barter_execution::trade::{AssetFees, Trade, TradeId};
use barter_instrument::{
    Side,
    asset::{AssetIndex, QuoteAsset},
    instrument::InstrumentIndex,
};
use chrono::{DateTime, Utc};
use derive_more::Constructor;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use tracing::error;

/// 仓位管理器，管理当前仓位状态。
///
/// PositionManager 负责跟踪和管理当前仓位。它维护一个可选的当前仓位，
/// 当仓位被完全平仓时，返回 `PositionExited`。
///
/// ## 类型参数
///
/// - `InstrumentKey`: 交易对键类型，默认为 `InstrumentIndex`
///
/// ## 使用场景
///
/// - 跟踪当前持仓状态
/// - 处理交易更新仓位
/// - 管理仓位生命周期
///
/// # 使用示例
///
/// ```rust,ignore
/// let mut position_manager = PositionManager::default();
///
/// // 从交易创建仓位
/// if let Some(position_exited) = position_manager.update_from_trade(&trade) {
///     // 处理已平仓的仓位
/// }
/// ```
#[derive(Debug, Clone, PartialEq, PartialOrd, Deserialize, Serialize, Constructor)]
pub struct PositionManager<InstrumentKey = InstrumentIndex> {
    /// 当前仓位（如果存在）
    pub current: Option<Position<QuoteAsset, InstrumentKey>>,
}

impl<InstrumentKey> Default for PositionManager<InstrumentKey> {
    fn default() -> Self {
        Self { current: None }
    }
}

impl<InstrumentKey> PositionManager<InstrumentKey> {
    /// 基于新交易更新当前仓位状态。
    ///
    /// 此方法处理各种仓位操作场景：
    ///
    /// - **开仓**: 如果当前没有仓位，从交易创建新仓位
    /// - **加仓**: 如果交易方向与当前仓位相同，增加仓位数量
    /// - **减仓**: 如果交易方向与当前仓位相反，减少仓位数量（部分平仓）
    /// - **平仓**: 如果交易完全抵消当前仓位，关闭仓位
    /// - **翻仓**: 如果交易数量超过当前仓位，关闭当前仓位并开立反向仓位
    ///
    /// # 参数
    ///
    /// - `trade`: 新交易
    ///
    /// # 返回值
    ///
    /// - `Some(PositionExited)`: 如果仓位被完全平仓
    /// - `None`: 如果仓位仍然存在或新开仓
    ///
    /// # 类型约束
    ///
    /// - `InstrumentKey`: 必须实现 `Debug + Clone + PartialEq`
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// // 处理交易更新仓位
    /// if let Some(position_exited) = position_manager.update_from_trade(&trade) {
    ///     // 处理已平仓的仓位
    ///     println!("Position closed: {:?}", position_exited);
    /// }
    /// ```
    pub fn update_from_trade(
        &mut self,
        trade: &Trade<QuoteAsset, InstrumentKey>,
    ) -> Option<PositionExited<QuoteAsset, InstrumentKey>>
    where
        InstrumentKey: Debug + Clone + PartialEq,
    {
        let (current, closed) = match self.current.take() {
            Some(position) => {
                // 更新当前仓位，可能会关闭它，并可能用剩余的交易数量开立新仓位
                position.update_from_trade(trade)
            }
            None => {
                // 当前没有仓位，所以从交易创建新仓位
                (Some(Position::from(trade)), None)
            }
        };

        self.current = current;

        closed
    }
}

/// 表示特定交易对的开放交易仓位。
///
/// Position 表示在特定交易对上的当前持仓状态，包括持仓方向、数量、平均入场价格、
/// 盈亏等信息。仓位可以通过交易进行更新（加仓、减仓、平仓、翻仓）。
///
/// ## 类型参数
///
/// - `AssetKey`: 用于表示手续费资产的类型（例如 `AssetIndex`, `QuoteAsset` 等）
/// - `InstrumentKey`: 用于标识交易对的类型（例如 `InstrumentIndex` 等）
///
/// ## 仓位状态
///
/// - **持仓方向**: `Side::Buy` 表示做多（LONG），`Side::Sell` 表示做空（SHORT）
/// - **持仓数量**: `quantity_abs` 表示当前绝对持仓数量
/// - **最大持仓**: `quantity_abs_max` 表示历史最大持仓数量
/// - **平均入场价**: `price_entry_average` 表示所有加仓交易的数量加权平均价格
///
/// ## 盈亏计算
///
/// - **已实现盈亏**: `pnl_realised` 表示已平仓部分的累计盈亏（包含手续费）
/// - **未实现盈亏**: `pnl_unrealised` 表示当前持仓的估算盈亏（包含估算的出场手续费）
///
/// ## 手续费
///
/// - **入场手续费**: `fees_enter` 表示开仓和加仓时累计支付的手续费
/// - **出场手续费**: `fees_exit` 表示减仓和平仓时累计支付的手续费
///
/// # 使用示例
/// ## Partially Reduce LONG Position
/// ```rust
/// use barter::engine::state::position::Position;
/// use barter_execution::order::id::{OrderId, StrategyId};
/// use barter_execution::trade::{AssetFees, Trade, TradeId};
/// use barter_instrument::asset::QuoteAsset;
/// use barter_instrument::instrument::name::InstrumentNameInternal;
/// use barter_instrument::Side;
/// use chrono::{DateTime, Utc};
/// use std::str::FromStr;
/// use rust_decimal_macros::dec;
///
/// // Create a new LONG Position from an initial Buy trade
/// let position = Position::from(&Trade {
///     id: TradeId::new("trade_1"),
///     order_id: OrderId::new("order_1"),
///     instrument: InstrumentNameInternal::new("BTC-USD"),
///     strategy: StrategyId::new("strategy_1"),
///     time_exchange: DateTime::from_str("2024-01-01T00:00:00Z").unwrap(),
///     side: Side::Buy,
///     price: dec!(50_000.0),
///     quantity: dec!(0.1),
///     fees: AssetFees::quote_fees(dec!(5.0))
/// });
/// assert_eq!(position.side, Side::Buy);
/// assert_eq!(position.quantity_abs, dec!(0.1));
///
/// // Partially reduce LONG Position from a new Sell Trade
/// let (updated_position, closed_position) = position.update_from_trade(&Trade {
///     id: TradeId::new("trade_2"),
///     order_id: OrderId::new("order_2"),
///     instrument: InstrumentNameInternal::new("BTC-USD"),
///     strategy: StrategyId::new("strategy_1"),
///     time_exchange: DateTime::from_str("2024-01-01T01:00:00Z").unwrap(),
///     side: Side::Sell,
///     price: dec!(60_000.0),
///     quantity: dec!(0.05),
///     fees: AssetFees::quote_fees(dec!(2.5))
/// });
///
/// // LONG Position is still open, but with reduced size
/// let updated_position = updated_position.unwrap();
/// assert_eq!(updated_position.quantity_abs, dec!(0.05));
/// assert_eq!(updated_position.quantity_abs_max, dec!(0.1));
/// assert_eq!(updated_position.pnl_realised, dec!(492.5));
/// assert!(closed_position.is_none());
/// ```
///
/// ## Flip Position - Close SHORT and Open LONG
/// ```rust
/// use barter::engine::state::position::Position;
/// use barter_execution::order::id::{OrderId, StrategyId};
/// use barter_execution::trade::{AssetFees, Trade, TradeId};
/// use barter_instrument::asset::QuoteAsset;
/// use barter_instrument::instrument::name::InstrumentNameInternal;
/// use barter_instrument::Side;
/// use chrono::{DateTime, Utc};
/// use std::str::FromStr;
/// use rust_decimal_macros::dec;
///
/// // Create a new SHORT Position from an initial Sell trade
/// let position = Position::from(&Trade {
///     id: TradeId::new("trade_1"),
///     order_id: OrderId::new("order_1"),
///     instrument: InstrumentNameInternal::new("BTC-USD"),
///     strategy: StrategyId::new("strategy_1"),
///     time_exchange: DateTime::from_str("2024-01-01T00:00:00Z").unwrap(),
///     side: Side::Sell,
///     price: dec!(50_000.0),
///     quantity: dec!(0.1),
///     fees: AssetFees::quote_fees(dec!(5.0))
/// });
/// assert_eq!(position.side, Side::Sell);
/// assert_eq!(position.quantity_abs, dec!(0.1));
///
/// // Close SHORT from a new Buy trade with larger quantity, flipping into a new LONG Position
/// let (new_position, closed_position) = position.update_from_trade(&Trade {
///     id: TradeId::new("trade_2"),
///     order_id: OrderId::new("order_2"),
///     instrument: InstrumentNameInternal::new("BTC-USD"),
///     strategy: StrategyId::new("strategy_1"),
///     time_exchange: DateTime::from_str("2024-01-01T01:00:00Z").unwrap(),
///     side: Side::Buy,
///     price: dec!(40_000.0),
///     quantity: dec!(0.2),
///     fees: AssetFees::quote_fees(dec!(10.0))
/// });
///
/// // Original SHORT Position closed with profit
/// let closed = closed_position.unwrap();
/// assert_eq!(closed.side, Side::Sell);
/// assert_eq!(closed.quantity_abs_max, dec!(0.1));
/// assert_eq!(closed.pnl_realised, dec!(990.0));
///
/// // New LONG Position opened with remaining quantity & proportional fees
/// let new_position = new_position.unwrap();
/// assert_eq!(new_position.side, Side::Buy);
/// assert_eq!(new_position.quantity_abs, dec!(0.1));
/// assert_eq!(new_position.price_entry_average, dec!(40_000.0));
/// assert_eq!(new_position.pnl_realised, dec!(-5.0));
/// ```
#[derive(Debug, Clone, PartialEq, PartialOrd, Deserialize, Serialize, Constructor)]
pub struct Position<AssetKey = AssetIndex, InstrumentKey = InstrumentIndex> {
    /// 仓位对应的交易对标识符（例如 `InstrumentIndex`, `InstrumentNameInternal` 等）。
    pub instrument: InstrumentKey,

    /// 仓位方向（`Side::Buy` => 做多 LONG，`Side::Sell` => 做空 SHORT）。
    pub side: Side,

    /// 所有加仓交易的数量加权平均入场价格。
    ///
    /// 当加仓时，使用公式计算新的平均价格：
    /// `(当前平均价格 * 当前数量 + 新交易价格 * 新交易数量) / (当前数量 + 新交易数量)`
    pub price_entry_average: Decimal,

    /// 当前绝对仓位数量。
    ///
    /// 这是当前持仓的绝对数量，无论方向如何都是正数。
    pub quantity_abs: Decimal,

    /// 所有开仓/加仓交易达到的最大绝对仓位数量。
    ///
    /// 此值记录仓位的历史峰值，用于计算手续费比例等。
    pub quantity_abs_max: Decimal,

    /// 估算的未实现盈亏，通过以当前价格平仓剩余 `quantity_abs` 计算得出。
    ///
    /// 注意：此值包含估算的出场手续费。
    ///
    /// 未实现盈亏的计算公式：
    /// - 做多：`(当前价格 - 平均入场价格) * 数量 - 估算出场手续费`
    /// - 做空：`(平均入场价格 - 当前价格) * 数量 - 估算出场手续费`
    pub pnl_unrealised: Decimal,

    /// 从任何部分平仓的累计已实现盈亏。
    ///
    /// 注意：此值包含手续费。
    ///
    /// 已实现盈亏是实际平仓时产生的盈亏，与未实现盈亏不同，这是已经确认的盈亏。
    pub pnl_realised: Decimal,

    /// 开仓/加仓时累计支付的手续费。
    pub fees_enter: AssetFees<AssetKey>,

    /// 减仓/平仓时累计支付的手续费。
    pub fees_exit: AssetFees<AssetKey>,

    /// 触发初始仓位开仓的交易时间戳。
    pub time_enter: DateTime<Utc>,

    /// 最近一次仓位更新的时间戳。
    ///
    /// 注意：这可能是由交易触发的更新，也可能是由新市场价格触发的 `pnl_unrealised` 更新。
    pub time_exchange_update: DateTime<Utc>,

    /// 与此仓位相关的所有交易的 [`TradeId`] 列表。
    ///
    /// 包括开仓、加仓、减仓、平仓等所有相关交易。
    pub trades: Vec<TradeId>,
}

impl<InstrumentKey> Position<QuoteAsset, InstrumentKey> {
    /// 基于新交易更新仓位状态。
    ///
    /// 此方法处理各种仓位操作场景：
    ///
    /// - **加仓**: 如果交易方向与当前仓位相同，增加仓位数量并更新平均入场价格
    /// - **减仓**: 如果交易方向与当前仓位相反且数量小于当前仓位，减少仓位数量（部分平仓）
    /// - **平仓**: 如果交易方向与当前仓位相反且数量等于当前仓位，完全关闭仓位
    /// - **翻仓**: 如果交易方向与当前仓位相反且数量大于当前仓位，关闭当前仓位并开立反向仓位
    ///
    /// ## 处理逻辑
    ///
    /// 1. **验证交易对**: 确保交易属于同一交易对
    /// 2. **记录交易ID**: 将交易ID添加到仓位交易列表
    /// 3. **根据方向处理**: 根据交易方向与仓位方向的关系执行相应操作
    /// 4. **更新状态**: 更新数量、价格、盈亏、手续费等状态
    ///
    /// # 参数
    ///
    /// - `trade`: 要处理的新交易
    ///
    /// # 返回值
    ///
    /// 返回一个元组，包含：
    /// - `Option<Position>`: 更新后的仓位，如果仓位被完全平仓则为 `None`
    /// - `Option<PositionExited>`: 如果仓位被关闭，返回已平仓的仓位信息
    ///
    /// # 类型约束
    ///
    /// - `InstrumentKey`: 必须实现 `Debug + Clone + PartialEq`
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// // 加仓
    /// let (updated, closed) = position.update_from_trade(&buy_trade);
    /// // updated 包含更新后的仓位，closed 为 None
    ///
    /// // 平仓
    /// let (updated, closed) = position.update_from_trade(&sell_trade);
    /// // updated 为 None，closed 包含已平仓的仓位信息
    /// ```
    pub fn update_from_trade(
        mut self,
        trade: &Trade<QuoteAsset, InstrumentKey>,
    ) -> (
        Option<Self>,
        Option<PositionExited<QuoteAsset, InstrumentKey>>,
    )
    where
        InstrumentKey: Debug + Clone + PartialEq,
    {
        // 合理性检查：确保交易属于同一交易对
        if self.instrument != trade.instrument {
            error!(
                position = ?self,
                trade = ?trade,
                "Position tried to be updated from a Trade for a different Instrument - ignoring"
            );
            return (Some(self), None);
        }

        // 将交易ID添加到当前仓位的交易列表
        self.trades.push(trade.id.clone());

        use Side::*;
        match (self.side, trade.side) {
            // 加仓：增加 LONG/SHORT 仓位
            (Buy, Buy) | (Sell, Sell) => {
                self.update_price_entry_average(trade);
                self.quantity_abs += trade.quantity.abs();
                if self.quantity_abs > self.quantity_abs_max {
                    self.quantity_abs_max = self.quantity_abs;
                }
                self.pnl_realised -= trade.fees.fees;
                self.fees_enter.fees += trade.fees.fees;
                self.time_exchange_update = trade.time_exchange;
                self.update_pnl_unrealised(trade.price);

                (Some(self), None)
            }
            // 减仓：减少 LONG/SHORT 仓位（部分平仓）
            (Buy, Sell) | (Sell, Buy) if self.quantity_abs > trade.quantity.abs() => {
                // Update pnl_realised
                self.update_pnl_realised(trade.quantity, trade.price, trade.fees.fees);

                // Update remaining Position state
                self.quantity_abs -= trade.quantity.abs();
                self.fees_exit.fees += trade.fees.fees;
                self.time_exchange_update = trade.time_exchange;

                // Update pnl_unrealised for remaining Position
                self.update_pnl_unrealised(trade.price);

                (Some(self), None)
            }
            // 平仓：完全关闭 LONG/SHORT 仓位
            (Buy, Sell) | (Sell, Buy) if self.quantity_abs == trade.quantity.abs() => {
                self.quantity_abs -= trade.quantity.abs();
                self.fees_exit.fees += trade.fees.fees;
                self.time_exchange_update = trade.time_exchange;
                self.update_pnl_realised(trade.quantity, trade.price, trade.fees.fees);
                self.update_pnl_unrealised(trade.price);

                (None, Some(PositionExited::from(self)))
            }

            // 翻仓：关闭 LONG/SHORT 仓位并用剩余的交易数量开立 SHORT/LONG 仓位
            (Buy, Sell) | (Sell, Buy) if self.quantity_abs < trade.quantity.abs() => {
                // 交易翻仓，所以为下一个仓位生成理论上的初始交易
                let next_position_quantity = trade.quantity.abs() - self.quantity_abs;
                let next_position_fee_enter =
                    trade.fees.fees * (next_position_quantity / trade.quantity.abs());
                let next_position_trade = Trade {
                    id: trade.id.clone(),
                    order_id: trade.order_id.clone(),
                    instrument: trade.instrument.clone(),
                    strategy: trade.strategy.clone(),
                    time_exchange: trade.time_exchange,
                    side: trade.side,
                    price: trade.price,
                    quantity: next_position_quantity,
                    fees: AssetFees {
                        asset: trade.fees.asset.clone(),
                        fees: next_position_fee_enter,
                    },
                };

                // Update closing Position with appropriate ratio of fees for theoretical quantity
                let fee_exit = trade.fees.fees * (self.quantity_abs / trade.quantity.abs());
                self.fees_exit.fees += fee_exit;
                self.time_exchange_update = trade.time_exchange;
                self.update_pnl_realised(self.quantity_abs, trade.price, fee_exit);
                self.quantity_abs = Decimal::ZERO;
                self.update_pnl_unrealised(trade.price);

                (
                    Some(Self::from(&next_position_trade)),
                    Some(PositionExited::from(self)),
                )
            }
            _ => unreachable!("match expression guard statements cover all cases"),
        }
    }

    /// 更新仓位的数量加权平均入场价格。
    ///
    /// 此方法在加仓时调用，使用 [`calculate_price_entry_average`] 中定义的逻辑计算新的平均价格。
    ///
    /// # 参数
    ///
    /// - `trade`: 加仓交易
    ///
    /// # 工作原理
    ///
    /// 使用公式：`(当前平均价格 * 当前数量 + 新交易价格 * 新交易数量) / (当前数量 + 新交易数量)`
    fn update_price_entry_average(&mut self, trade: &Trade<QuoteAsset, InstrumentKey>) {
        self.price_entry_average = calculate_price_entry_average(
            self.price_entry_average,
            self.quantity_abs,
            trade.price,
            trade.quantity.abs(),
        );
    }

    /// 使用提供的价格更新 [`Position::pnl_unrealised`](Position)（未实现盈亏）。
    ///
    /// 此方法计算如果以提供的价格平仓当前仓位时的估算盈亏。
    ///
    /// ## 使用场景
    ///
    /// - 使用最近的交易价格更新未实现盈亏
    /// - 使用基于公开市场数据的模型价格更新未实现盈亏
    /// - 实时计算仓位的浮动盈亏
    ///
    /// ## 注意事项
    ///
    /// - 此值包含估算的出场手续费
    /// - 未实现盈亏会随着市场价格变化而变化
    ///
    /// # 参数
    ///
    /// - `price`: 用于计算未实现盈亏的价格（通常是当前市场价格）
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// // 使用当前市场价格更新未实现盈亏
    /// position.update_pnl_unrealised(current_market_price);
    /// ```
    pub fn update_pnl_unrealised(&mut self, price: Decimal) {
        self.pnl_unrealised = calculate_pnl_unrealised(
            self.side,
            self.price_entry_average,
            self.quantity_abs,
            self.quantity_abs_max,
            self.fees_enter.fees,
            price,
        );
    }

    /// 从已平仓的仓位数量更新 [`Position`] 的 `pnl_realised`（已实现盈亏）。
    ///
    /// 此方法在减仓或平仓时调用，计算已平仓部分的盈亏并累加到总已实现盈亏中。
    ///
    /// # 参数
    ///
    /// - `closed_quantity`: 已平仓的数量
    /// - `closed_price`: 平仓价格
    /// - `closed_fee`: 平仓手续费
    ///
    /// # 工作原理
    ///
    /// 使用 [`calculate_pnl_realised`] 计算已平仓部分的盈亏，然后累加到 `pnl_realised`。
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// // 部分平仓后更新已实现盈亏
    /// position.update_pnl_realised(closed_quantity, closed_price, closed_fee);
    /// ```
    pub fn update_pnl_realised(
        &mut self,
        closed_quantity: Decimal,
        closed_price: Decimal,
        closed_fee: Decimal,
    ) {
        // 使用已平仓数量的盈亏更新总仓位的已实现盈亏
        self.pnl_realised += calculate_pnl_realised(
            self.side,
            self.price_entry_average,
            closed_quantity,
            closed_price,
            closed_fee,
        );
    }
}

impl<InstrumentKey> From<&Trade<QuoteAsset, InstrumentKey>> for Position<QuoteAsset, InstrumentKey>
where
    InstrumentKey: Clone,
{
    fn from(trade: &Trade<QuoteAsset, InstrumentKey>) -> Self {
        let mut trades = Vec::with_capacity(2);
        trades.push(trade.id.clone());
        Self {
            instrument: trade.instrument.clone(),
            side: trade.side,
            price_entry_average: trade.price,
            quantity_abs: trade.quantity.abs(),
            quantity_abs_max: trade.quantity.abs(),
            pnl_unrealised: Decimal::ZERO,
            pnl_realised: -trade.fees.fees,
            fees_enter: trade.fees.clone(),
            fees_exit: AssetFees::default(),
            time_enter: trade.time_exchange,
            time_exchange_update: trade.time_exchange,
            trades,
        }
    }
}

/// 表示已完全平仓的交易仓位。
///
/// PositionExited 包含已完全平仓的仓位的最终状态和历史记录。它用于记录和分析
/// 已完成的交易，包括最终的盈亏、手续费、交易历史等信息。
///
/// ## 类型参数
///
/// - `AssetKey`: 用于表示手续费资产的类型（例如 `AssetIndex`, `QuoteAsset` 等）
/// - `InstrumentKey`: 用于标识交易对的类型（例如 `InstrumentIndex` 等）
///
/// ## 与 Position 的区别
///
/// - `Position`: 表示当前开放的仓位，可以继续更新
/// - `PositionExited`: 表示已完全平仓的仓位，是最终状态，不可再更新
///
/// # 使用场景
///
/// - 记录已完成的交易
/// - 分析交易绩效
/// - 生成交易报告
#[derive(
    Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, Constructor,
)]
pub struct PositionExited<AssetKey, InstrumentKey = InstrumentIndex> {
    /// 已平仓的仓位对应的交易对标识符（例如 `InstrumentIndex`, `InstrumentNameInternal` 等）。
    pub instrument: InstrumentKey,

    /// 已平仓的仓位方向（`Side::Buy` => 做多 LONG，`Side::Sell` => 做空 SHORT）。
    pub side: Side,

    /// 所有加仓交易的数量加权平均入场价格。
    pub price_entry_average: Decimal,

    /// 所有开仓/加仓交易达到的最大绝对仓位数量。
    pub quantity_abs_max: Decimal,

    /// 从完全平仓整个 `quantity_abs_max` 仓位产生的累计已实现盈亏。
    ///
    /// 注意：此值包含手续费。
    pub pnl_realised: Decimal,

    /// 开仓时累计支付的手续费。
    pub fees_enter: AssetFees<AssetKey>,

    /// 平仓时累计支付的手续费。
    pub fees_exit: AssetFees<AssetKey>,

    /// 触发初始仓位开仓的交易时间戳。
    pub time_enter: DateTime<Utc>,

    /// 触发仓位平仓的交易时间戳。
    pub time_exit: DateTime<Utc>,

    /// 与此已平仓仓位相关的所有交易的 [`TradeId`] 列表。
    pub trades: Vec<TradeId>,
}

impl<AssetKey, InstrumentKey> From<Position<AssetKey, InstrumentKey>>
    for PositionExited<AssetKey, InstrumentKey>
{
    fn from(value: Position<AssetKey, InstrumentKey>) -> Self {
        Self {
            instrument: value.instrument,
            side: value.side,
            price_entry_average: value.price_entry_average,
            quantity_abs_max: value.quantity_abs_max,
            pnl_realised: value.pnl_realised,
            fees_enter: value.fees_enter,
            fees_exit: value.fees_exit,
            time_enter: value.time_enter,
            time_exit: value.time_exchange_update,
            trades: value.trades,
        }
    }
}

/// 计算在现有仓位数据中添加交易数据后的数量加权平均入场价格。
///
/// 此函数使用公式：`(当前价值 + 交易价值) / (当前数量 + 交易数量)`
///
/// 其中：
/// - 当前价值 = `current_price_entry_average * current_quantity_abs`
/// - 交易价值 = `trade_price * trade_quantity_abs`
///
/// # 参数
///
/// - `current_price_entry_average`: 仓位的当前平均入场价格
/// - `current_quantity_abs`: 仓位的当前绝对数量
/// - `trade_price`: 新交易的价格
/// - `trade_quantity_abs`: 新交易的绝对数量
///
/// # 返回值
///
/// 返回计算后的新的平均入场价格。
///
/// # 边界情况
///
/// - 如果当前数量和新交易数量都为 0，返回 0
fn calculate_price_entry_average(
    current_price_entry_average: Decimal,
    current_quantity_abs: Decimal,
    trade_price: Decimal,
    trade_quantity_abs: Decimal,
) -> Decimal {
    if current_quantity_abs.is_zero() && trade_quantity_abs.is_zero() {
        return Decimal::ZERO;
    }

    let current_value = current_price_entry_average * current_quantity_abs;
    let trade_value = trade_price * trade_quantity_abs;

    (current_value + trade_value) / (current_quantity_abs + trade_quantity_abs)
}

/// 计算以提供的价格平仓仓位 `quantity_abs` 时的估算未实现盈亏。
///
/// 此函数计算如果以指定价格平仓当前持仓数量时的估算盈亏，包括估算的出场手续费。
///
/// ## 计算公式
///
/// - **做多（LONG）**: `(当前价格 - 平均入场价格) * 数量 - 估算出场手续费`
/// - **做空（SHORT）**: `(平均入场价格 - 当前价格) * 数量 - 估算出场手续费`
///
/// ## 手续费估算
///
/// 使用 `approximate_remaining_exit_fees` 函数估算出场手续费，基于入场手续费的比例。
///
/// # 参数
///
/// - `position_side`: 仓位方向（`Side::Buy` 或 `Side::Sell`）
/// - `price_entry_average`: 平均入场价格
/// - `quantity_abs`: 当前绝对持仓数量
/// - `quantity_abs_max`: 历史最大持仓数量（用于计算手续费比例）
/// - `fees_enter`: 入场手续费
/// - `price`: 用于计算盈亏的价格（通常是当前市场价格）
///
/// # 返回值
///
/// 返回估算的未实现盈亏（包含估算的出场手续费）。
pub fn calculate_pnl_unrealised(
    position_side: Side,
    price_entry_average: Decimal,
    quantity_abs: Decimal,
    quantity_abs_max: Decimal,
    fees_enter: Decimal,
    price: Decimal,
) -> Decimal {
    let approx_exit_fees =
        approximate_remaining_exit_fees(quantity_abs, quantity_abs_max, fees_enter);

    let value_quote_current = quantity_abs * price;
    let value_quote_entry = quantity_abs * price_entry_average;

    match position_side {
        Side::Buy => value_quote_current - value_quote_entry - approx_exit_fees,
        Side::Sell => value_quote_entry - value_quote_current - approx_exit_fees,
    }
}

/// Approximate the exit fees from closing a [`Position`] with `quantity_abs`.
///
/// The `fees_enter` value was the fee cost to enter a [`Position`] of `quantity_abs_max`,
/// therefore this 'fee per quantity' ratio can be used to approximate the exit fees required to
/// close a `quantity_abs` [`Position`].
fn approximate_remaining_exit_fees(
    quantity_abs: Decimal,
    quantity_abs_max: Decimal,
    fees_enter: Decimal,
) -> Decimal {
    (quantity_abs / quantity_abs_max) * fees_enter
}

/// 计算以指定价格和手续费平仓提供的仓位数量时产生的已实现盈亏。
///
/// 此函数计算实际平仓时的盈亏，与未实现盈亏不同，这是已经确认的盈亏。
///
/// ## 计算公式
///
/// - **做多（LONG）**: `(平仓价格 - 平均入场价格) * 平仓数量 - 平仓手续费`
/// - **做空（SHORT）**: `(平均入场价格 - 平仓价格) * 平仓数量 - 平仓手续费`
///
/// # 参数
///
/// - `position_side`: 仓位方向（`Side::Buy` 或 `Side::Sell`）
/// - `price_entry_average`: 平均入场价格
/// - `closed_quantity`: 已平仓的数量
/// - `closed_price`: 平仓价格
/// - `closed_fee`: 平仓手续费
///
/// # 返回值
///
/// 返回已实现盈亏（已扣除手续费）。
pub fn calculate_pnl_realised(
    position_side: Side,
    price_entry_average: Decimal,
    closed_quantity: Decimal,
    closed_price: Decimal,
    closed_fee: Decimal,
) -> Decimal {
    let close_quantity = closed_quantity.abs();
    let value_quote_closed = close_quantity * closed_price;
    let value_quote_entry = close_quantity * price_entry_average;

    match position_side {
        Side::Buy => value_quote_closed - value_quote_entry - closed_fee,
        Side::Sell => value_quote_entry - value_quote_closed - closed_fee,
    }
}

/// 计算盈亏回报率。
///
/// 此函数计算投资的回报率（ROI），公式为：`已实现盈亏 / 投资成本`
///
/// 其中投资成本 = `平均入场价格 * 最大持仓数量`
///
/// ## 参考文档
///
/// 参见：<https://www.investopedia.com/articles/basics/10/guide-to-calculating-roi.asp>
///
/// # 参数
///
/// - `pnl_realised`: 已实现盈亏
/// - `price_entry_average`: 平均入场价格
/// - `quantity_abs_max`: 最大持仓数量
///
/// # 返回值
///
/// 返回回报率（小数形式，例如 0.1 表示 10% 的回报率）。
///
/// # 使用示例
///
/// ```rust,ignore
/// let return_rate = calculate_pnl_return(
///     dec!(100.0),  // 已实现盈亏
///     dec!(50.0),   // 平均入场价格
///     dec!(2.0),    // 最大持仓数量
/// );
/// // 返回 1.0，表示 100% 的回报率
/// ```
pub fn calculate_pnl_return(
    pnl_realised: Decimal,
    price_entry_average: Decimal,
    quantity_abs_max: Decimal,
) -> Decimal {
    pnl_realised / (price_entry_average * quantity_abs_max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{time_plus_days, trade};
    use barter_instrument::instrument::name::InstrumentNameInternal;
    use rust_decimal_macros::dec;

    #[test]
    fn test_position_update_from_trade() {
        struct TestCase {
            initial_trade: Trade<QuoteAsset, InstrumentNameInternal>,
            update_trade: Trade<QuoteAsset, InstrumentNameInternal>,
            expected_position: Option<Position<QuoteAsset, InstrumentNameInternal>>,
            expected_position_exited: Option<PositionExited<QuoteAsset, InstrumentNameInternal>>,
        }

        let base_time = DateTime::<Utc>::MIN_UTC;

        let cases = vec![
            // TC0: Increase long position
            TestCase {
                initial_trade: trade(base_time, Side::Buy, 100.0, 1.0, 10.0),
                update_trade: trade(time_plus_days(base_time, 1), Side::Buy, 120.0, 1.0, 10.0),
                expected_position: Some(Position {
                    instrument: InstrumentNameInternal::new("instrument"),
                    side: Side::Buy,
                    price_entry_average: dec!(110.0),
                    quantity_abs: dec!(2.0),
                    quantity_abs_max: dec!(2.0),
                    pnl_unrealised: dec!(0.0),
                    pnl_realised: dec!(-20.0), // Sum of fees
                    fees_enter: AssetFees {
                        asset: QuoteAsset,
                        fees: dec!(20.0),
                    },
                    fees_exit: AssetFees {
                        asset: QuoteAsset,
                        fees: dec!(0.0),
                    },
                    time_enter: base_time,
                    time_exchange_update: time_plus_days(base_time, 1),
                    trades: vec![TradeId::new("trade_id"), TradeId::new("trade_id")],
                }),
                expected_position_exited: None,
            },
            // TC1: Partial reduce long position
            TestCase {
                initial_trade: trade(base_time, Side::Buy, 100.0, 2.0, 10.0),
                update_trade: trade(time_plus_days(base_time, 1), Side::Sell, 150.0, 0.5, 5.0),
                expected_position: Some(Position {
                    instrument: InstrumentNameInternal::new("instrument"),
                    side: Side::Buy,
                    price_entry_average: dec!(100.0), // update_trade is Sell, so unchanged
                    quantity_abs: dec!(1.5),
                    quantity_abs_max: dec!(2.0),
                    pnl_unrealised: dec!(67.5), // (150-100)*(2.0-0.5) - approx_exit_fees (1.5/2 * 10)
                    pnl_realised: dec!(10.0),   // (150-100)*0.5 - 15_fees
                    fees_enter: AssetFees {
                        asset: QuoteAsset,
                        fees: dec!(10.0),
                    },
                    fees_exit: AssetFees {
                        asset: QuoteAsset,
                        fees: dec!(5.0),
                    },
                    time_enter: base_time,
                    time_exchange_update: time_plus_days(base_time, 1),
                    trades: vec![TradeId::new("trade_id"), TradeId::new("trade_id")],
                }),
                expected_position_exited: None,
            },
            // TC2: Exact position close, in profit
            TestCase {
                initial_trade: trade(base_time, Side::Buy, 100.0, 1.0, 10.0),
                update_trade: trade(time_plus_days(base_time, 1), Side::Sell, 150.0, 1.0, 10.0),
                expected_position: None,
                expected_position_exited: Some(PositionExited {
                    instrument: InstrumentNameInternal::new("instrument"),
                    side: Side::Buy,
                    price_entry_average: dec!(100.0),
                    quantity_abs_max: dec!(1.0),
                    pnl_realised: dec!(30.0), // (150-100)*1 - 20 (total fees)
                    fees_enter: AssetFees {
                        asset: QuoteAsset,
                        fees: dec!(10.0),
                    },
                    fees_exit: AssetFees {
                        asset: QuoteAsset,
                        fees: dec!(10.0),
                    },
                    time_enter: base_time,
                    time_exit: time_plus_days(base_time, 1),
                    trades: vec![TradeId::new("trade_id"), TradeId::new("trade_id")],
                }),
            },
            // TC3: Position flip (close and open new)
            TestCase {
                initial_trade: trade(base_time, Side::Buy, 100.0, 1.0, 10.0),
                update_trade: trade(time_plus_days(base_time, 1), Side::Sell, 150.0, 2.0, 20.0),
                expected_position: Some(Position {
                    instrument: InstrumentNameInternal::new("instrument"),
                    side: Side::Sell,
                    price_entry_average: dec!(150.0),
                    quantity_abs: dec!(1.0),
                    quantity_abs_max: dec!(1.0),
                    pnl_unrealised: dec!(0.0),
                    pnl_realised: dec!(-10.0), // Entry fees for new position (2-1)*(1/2)*20
                    fees_enter: AssetFees {
                        asset: QuoteAsset,
                        fees: dec!(10.0),
                    },
                    fees_exit: AssetFees {
                        asset: QuoteAsset,
                        fees: dec!(0.0),
                    },
                    time_enter: time_plus_days(base_time, 1),
                    time_exchange_update: time_plus_days(base_time, 1),
                    trades: vec![TradeId::new("trade_id")],
                }),
                expected_position_exited: Some(PositionExited {
                    instrument: InstrumentNameInternal::new("instrument"),
                    side: Side::Buy,
                    price_entry_average: dec!(100.0),
                    quantity_abs_max: dec!(1.0),
                    pnl_realised: dec!(30.0), // (150-100)*1 - 20 (total fees)
                    fees_enter: AssetFees {
                        asset: QuoteAsset,
                        fees: dec!(10.0),
                    },
                    fees_exit: AssetFees {
                        asset: QuoteAsset,
                        fees: dec!(10.0),
                    },
                    time_enter: base_time,
                    time_exit: time_plus_days(base_time, 1),
                    trades: vec![TradeId::new("trade_id"), TradeId::new("trade_id")],
                }),
            },
            // TC4: Increase short position
            TestCase {
                initial_trade: trade(base_time, Side::Sell, 100.0, 1.0, 10.0),
                update_trade: trade(base_time, Side::Sell, 80.0, 1.0, 10.0),
                expected_position: Some(Position {
                    instrument: InstrumentNameInternal::new("instrument"),
                    side: Side::Sell,
                    price_entry_average: dec!(90.0), // (100*1 + 80*1)/(1 + 1)
                    quantity_abs: dec!(2.0),
                    quantity_abs_max: dec!(2.0),
                    pnl_unrealised: dec!(0.0), // (90-80)*2 - approx_exit_fees(2/2 * 20)
                    pnl_realised: dec!(-20.0), // Sum of entry fees
                    fees_enter: AssetFees {
                        asset: QuoteAsset,
                        fees: dec!(20.0),
                    },
                    fees_exit: AssetFees {
                        asset: QuoteAsset,
                        fees: dec!(0.0),
                    },
                    time_enter: base_time,
                    time_exchange_update: base_time,
                    trades: vec![TradeId::new("trade_id"), TradeId::new("trade_id")],
                }),
                expected_position_exited: None,
            },
            // TC5: Partial reduce short position
            TestCase {
                initial_trade: trade(base_time, Side::Sell, 100.0, 2.0, 10.0),
                update_trade: trade(base_time, Side::Buy, 80.0, 0.5, 5.0),
                expected_position: Some(Position {
                    instrument: InstrumentNameInternal::new("instrument"),
                    side: Side::Sell,
                    price_entry_average: dec!(100.0), // update_trade is Buy, so unchanged
                    quantity_abs: dec!(1.5),
                    quantity_abs_max: dec!(2.0),
                    pnl_unrealised: dec!(22.5), // (100-80)*1.5 - approx_exit_fees(1.5/2 * 10)
                    pnl_realised: dec!(-5.0),   // 10_fee_entry - (100-80)*0.5 - 5_fee_exit
                    fees_enter: AssetFees {
                        asset: QuoteAsset,
                        fees: dec!(10.0),
                    },
                    fees_exit: AssetFees {
                        asset: QuoteAsset,
                        fees: dec!(5.0),
                    },
                    time_enter: base_time,
                    time_exchange_update: base_time,
                    trades: vec![TradeId::new("trade_id"), TradeId::new("trade_id")],
                }),
                expected_position_exited: None,
            },
            // TC6: Exact short position close
            TestCase {
                initial_trade: trade(base_time, Side::Sell, 100.0, 1.0, 10.0),
                update_trade: trade(base_time, Side::Buy, 80.0, 1.0, 10.0),
                expected_position: None,
                expected_position_exited: Some(PositionExited {
                    instrument: InstrumentNameInternal::new("instrument"),
                    side: Side::Sell,
                    price_entry_average: dec!(100.0),
                    quantity_abs_max: dec!(1.0),
                    pnl_realised: dec!(0.0), // (100-80)*1 - 20 (total fees)
                    fees_enter: AssetFees {
                        asset: QuoteAsset,
                        fees: dec!(10.0),
                    },
                    fees_exit: AssetFees {
                        asset: QuoteAsset,
                        fees: dec!(10.0),
                    },
                    time_enter: base_time,
                    time_exit: base_time,
                    trades: vec![TradeId::new("trade_id"), TradeId::new("trade_id")],
                }),
            },
            // TC7: Short position flip (close and open long)
            TestCase {
                initial_trade: trade(base_time, Side::Sell, 100.0, 1.0, 10.0),
                update_trade: trade(base_time, Side::Buy, 80.0, 2.0, 20.0),
                expected_position: Some(Position {
                    instrument: InstrumentNameInternal::new("instrument"),
                    side: Side::Buy,
                    price_entry_average: dec!(80.0),
                    quantity_abs: dec!(1.0),
                    quantity_abs_max: dec!(1.0),
                    pnl_unrealised: dec!(0.0),
                    pnl_realised: dec!(-10.0), // Entry fees for new position
                    fees_enter: AssetFees {
                        asset: QuoteAsset,
                        fees: dec!(10.0),
                    },
                    fees_exit: AssetFees {
                        asset: QuoteAsset,
                        fees: dec!(0.0),
                    },
                    time_enter: base_time,
                    time_exchange_update: base_time,
                    trades: vec![TradeId::new("trade_id")],
                }),
                expected_position_exited: Some(PositionExited {
                    instrument: InstrumentNameInternal::new("instrument"),
                    side: Side::Sell,
                    price_entry_average: dec!(100.0),
                    quantity_abs_max: dec!(1.0),
                    pnl_realised: dec!(0.0), // (100-80)*1 - 20 (total fees)
                    fees_enter: AssetFees {
                        asset: QuoteAsset,
                        fees: dec!(10.0),
                    },
                    fees_exit: AssetFees {
                        asset: QuoteAsset,
                        fees: dec!(10.0),
                    },
                    time_enter: base_time,
                    time_exit: base_time,
                    trades: vec![TradeId::new("trade_id"), TradeId::new("trade_id")],
                }),
            },
        ];

        for (index, test) in cases.into_iter().enumerate() {
            let position = Position::from(&test.initial_trade);
            let (updated_position, exited_position) =
                position.update_from_trade(&test.update_trade);

            assert_eq!(updated_position, test.expected_position, "TC{index} failed");
            assert_eq!(
                exited_position, test.expected_position_exited,
                "TC{index} failed"
            );
        }
    }

    #[test]
    fn test_calculate_price_entry_average() {
        struct TestCase {
            current_price_entry_average: Decimal,
            current_quantity_abs: Decimal,
            trade_price: Decimal,
            trade_quantity_abs: Decimal,
            expected: Decimal,
        }

        let cases = vec![
            // TC0: equal contribution
            TestCase {
                current_price_entry_average: dec!(100.0),
                current_quantity_abs: dec!(2.0),
                trade_price: dec!(200.0),
                trade_quantity_abs: dec!(2.0),
                expected: dec!(150.0),
            },
            // TC1: trade larger contribution
            TestCase {
                current_price_entry_average: dec!(100.0),
                current_quantity_abs: dec!(2.0),
                trade_price: dec!(200.0),
                trade_quantity_abs: dec!(4.0),
                expected: dec!(166.66666666666666666666666667),
            },
            // TC2: current larger contribution
            TestCase {
                current_price_entry_average: dec!(100.0),
                current_quantity_abs: dec!(20.0),
                trade_price: dec!(200.0),
                trade_quantity_abs: dec!(1.0),
                expected: dec!(104.76190476190476190476190476),
            },
            // TC3: zero current quantity, so expect trade price
            TestCase {
                current_price_entry_average: dec!(100.0),
                current_quantity_abs: dec!(0.0),
                trade_price: dec!(200.0),
                trade_quantity_abs: dec!(4.0),
                expected: dec!(200.0),
            },
            // TC4: zero trade quantity, so expect current price
            TestCase {
                current_price_entry_average: dec!(100.0),
                current_quantity_abs: dec!(10.0),
                trade_price: dec!(0.0),
                trade_quantity_abs: dec!(0.0),
                expected: dec!(100.0),
            },
            // TC5: both zero quantities
            TestCase {
                current_price_entry_average: dec!(100.0),
                current_quantity_abs: dec!(0.0),
                trade_price: dec!(200.0),
                trade_quantity_abs: dec!(0.0),
                expected: dec!(0.0),
            },
        ];

        for (index, test) in cases.into_iter().enumerate() {
            let actual = calculate_price_entry_average(
                test.current_price_entry_average,
                test.current_quantity_abs,
                test.trade_price,
                test.trade_quantity_abs,
            );

            assert_eq!(actual, test.expected, "TC{} failed", index)
        }
    }

    #[test]
    fn test_calculate_pnl_unrealised() {
        struct TestCase {
            position_side: Side,
            price_entry_average: Decimal,
            quantity_abs: Decimal,
            quantity_abs_max: Decimal,
            fees_enter: Decimal,
            price: Decimal,
            expected: Decimal,
        }

        let cases = vec![
            // TC0: LONG position in profit
            TestCase {
                position_side: Side::Buy,
                price_entry_average: dec!(100.0),
                quantity_abs: dec!(1.0),
                quantity_abs_max: dec!(1.0),
                fees_enter: dec!(10.0),
                price: dec!(150.0),
                expected: dec!(40.0), // (150-100)*1 - 10
            },
            // TC1: LONG position at loss
            TestCase {
                position_side: Side::Buy,
                price_entry_average: dec!(100.0),
                quantity_abs: dec!(1.0),
                quantity_abs_max: dec!(1.0),
                fees_enter: dec!(10.0),
                price: dec!(80.0),
                expected: dec!(-30.0), // (80-100)*1 - 10
            },
            // TC2: SHORT position in profit
            TestCase {
                position_side: Side::Sell,
                price_entry_average: dec!(100.0),
                quantity_abs: dec!(1.0),
                quantity_abs_max: dec!(1.0),
                fees_enter: dec!(10.0),
                price: dec!(80.0),
                expected: dec!(10.0), // (100-80)*1 - 10
            },
            // TC3: SHORT position at loss
            TestCase {
                position_side: Side::Sell,
                price_entry_average: dec!(100.0),
                quantity_abs: dec!(1.0),
                quantity_abs_max: dec!(1.0),
                fees_enter: dec!(10.0),
                price: dec!(150.0),
                expected: dec!(-60.0), // (100-150)*1 - 10
            },
            // TC4: Partial position remaining (half closed)
            TestCase {
                position_side: Side::Buy,
                price_entry_average: dec!(100.0),
                quantity_abs: dec!(0.5),
                quantity_abs_max: dec!(1.0),
                fees_enter: dec!(10.0),
                price: dec!(150.0),
                expected: dec!(20.0), // (150-100)*0.5 - (0.5/1.0)*10
            },
            // TC5: Zero quantity position
            TestCase {
                position_side: Side::Buy,
                price_entry_average: dec!(100.0),
                quantity_abs: dec!(0.0),
                quantity_abs_max: dec!(1.0),
                fees_enter: dec!(10.0),
                price: dec!(150.0),
                expected: dec!(0.0),
            },
        ];

        for (index, test) in cases.into_iter().enumerate() {
            let actual = calculate_pnl_unrealised(
                test.position_side,
                test.price_entry_average,
                test.quantity_abs,
                test.quantity_abs_max,
                test.fees_enter,
                test.price,
            );

            assert_eq!(actual, test.expected, "TC{} failed", index);
        }
    }

    #[test]
    fn test_approximate_remaining_exit_fees() {
        struct TestCase {
            quantity_abs: Decimal,
            quantity_abs_max: Decimal,
            fees_enter: Decimal,
            expected: Decimal,
        }

        let cases = vec![
            // TC0: Full position - expect full fees
            TestCase {
                quantity_abs: dec!(1.0),
                quantity_abs_max: dec!(1.0),
                fees_enter: dec!(10.0),
                expected: dec!(10.0),
            },
            // TC1: Half position - expect half fees
            TestCase {
                quantity_abs: dec!(0.5),
                quantity_abs_max: dec!(1.0),
                fees_enter: dec!(10.0),
                expected: dec!(5.0),
            },
            // TC2: Zero position - expect zero fees
            TestCase {
                quantity_abs: dec!(0.0),
                quantity_abs_max: dec!(1.0),
                fees_enter: dec!(10.0),
                expected: dec!(0.0),
            },
            // TC3: Larger current quantity than max (edge case)
            TestCase {
                quantity_abs: dec!(2.0),
                quantity_abs_max: dec!(1.0),
                fees_enter: dec!(10.0),
                expected: dec!(20.0),
            },
        ];

        for (index, test) in cases.into_iter().enumerate() {
            let actual = approximate_remaining_exit_fees(
                test.quantity_abs,
                test.quantity_abs_max,
                test.fees_enter,
            );

            assert_eq!(actual, test.expected, "TC{} failed", index);
        }
    }

    #[test]
    fn test_calculate_pnl_realised() {
        struct TestCase {
            side: Side,
            price_entry_average: Decimal,
            closed_quantity: Decimal,
            closed_price: Decimal,
            closed_fee: Decimal,
            expected: Decimal,
        }

        let cases = vec![
            // TC0: LONG in profit w/ fee deduction
            TestCase {
                side: Side::Buy,
                price_entry_average: dec!(100.0),
                closed_quantity: dec!(10.0),
                closed_price: dec!(150.0),
                closed_fee: dec!(5.0),
                expected: dec!(495.0),
            },
            // TC1: LONG in profit w/o fee deduction
            TestCase {
                side: Side::Buy,
                price_entry_average: dec!(100.0),
                closed_quantity: dec!(10.0),
                closed_price: dec!(150.0),
                closed_fee: dec!(0.0),
                expected: dec!(500.0),
            },
            // TC2: LONG in profit w/ fee rebate
            TestCase {
                side: Side::Buy,
                price_entry_average: dec!(100.0),
                closed_quantity: dec!(10.0),
                closed_price: dec!(150.0),
                closed_fee: dec!(-5.0),
                expected: dec!(505.0),
            },
            // TC3: LONG in loss w/ fee deduction
            TestCase {
                side: Side::Buy,
                price_entry_average: dec!(100.0),
                closed_quantity: dec!(10.0),
                closed_price: dec!(50.0),
                closed_fee: dec!(5.0),
                expected: dec!(-505.0),
            },
            // TC4: LONG in loss w/o fee deduction
            TestCase {
                side: Side::Buy,
                price_entry_average: dec!(100.0),
                closed_quantity: dec!(10.0),
                closed_price: dec!(50.0),
                closed_fee: dec!(0.0),
                expected: dec!(-500.0),
            },
            // TC5: LONG in loss w/ fee rebate
            TestCase {
                side: Side::Buy,
                price_entry_average: dec!(100.0),
                closed_quantity: dec!(10.0),
                closed_price: dec!(50.0),
                closed_fee: dec!(-5.0),
                expected: dec!(-495.0),
            },
            // TC6: SHORT in profit w/ fee deduction
            TestCase {
                side: Side::Sell,
                price_entry_average: dec!(100.0),
                closed_quantity: dec!(10.0),
                closed_price: dec!(50.0),
                closed_fee: dec!(5.0),
                expected: dec!(495.0),
            },
            // TC7: SHORT in profit w/o fee deduction
            TestCase {
                side: Side::Sell,
                price_entry_average: dec!(100.0),
                closed_quantity: dec!(10.0),
                closed_price: dec!(50.0),
                closed_fee: dec!(0.0),
                expected: dec!(500.0),
            },
            // TC8: SHORT in profit w/ fee rebate
            TestCase {
                side: Side::Sell,
                price_entry_average: dec!(100.0),
                closed_quantity: dec!(10.0),
                closed_price: dec!(50.0),
                closed_fee: dec!(-5.0),
                expected: dec!(505.0),
            },
            // TC9: SHORT in loss w/ fee deduction
            TestCase {
                side: Side::Sell,
                price_entry_average: dec!(100.0),
                closed_quantity: dec!(10.0),
                closed_price: dec!(150.0),
                closed_fee: dec!(5.0),
                expected: dec!(-505.0),
            },
            // TC10: SHORT in loss w/o fee deduction
            TestCase {
                side: Side::Sell,
                price_entry_average: dec!(100.0),
                closed_quantity: dec!(10.0),
                closed_price: dec!(150.0),
                closed_fee: dec!(0.0),
                expected: dec!(-500.0),
            },
            // TC10: SHORT in loss w/ fee rebate
            TestCase {
                side: Side::Sell,
                price_entry_average: dec!(100.0),
                closed_quantity: dec!(10.0),
                closed_price: dec!(150.0),
                closed_fee: dec!(-5.0),
                expected: dec!(-495.0),
            },
        ];

        for (index, test) in cases.into_iter().enumerate() {
            let actual = calculate_pnl_realised(
                test.side,
                test.price_entry_average.into(),
                test.closed_quantity.into(),
                test.closed_price.into(),
                test.closed_fee.into(),
            );

            assert_eq!(actual, test.expected, "TC{} failed", index);
        }
    }

    #[test]
    fn test_calculate_pnl_return() {
        struct TestCase {
            pnl_realised: Decimal,
            price_entry_average: Decimal,
            quantity_abs_max: Decimal,
            expected: Decimal,
        }

        let cases = vec![
            // TC0: Break even (0% return)
            TestCase {
                pnl_realised: dec!(0.0),
                price_entry_average: dec!(100.0),
                quantity_abs_max: dec!(1.0),
                expected: dec!(0.0),
            },
            // TC1: 100% return
            TestCase {
                pnl_realised: dec!(100.0),
                price_entry_average: dec!(100.0),
                quantity_abs_max: dec!(1.0),
                expected: dec!(1.0),
            },
            // TC2: -50% return
            TestCase {
                pnl_realised: dec!(-50.0),
                price_entry_average: dec!(100.0),
                quantity_abs_max: dec!(1.0),
                expected: dec!(-0.5),
            },
            // TC3: Complex case with larger position
            TestCase {
                pnl_realised: dec!(500.0),
                price_entry_average: dec!(100.0),
                quantity_abs_max: dec!(10.0),
                expected: dec!(0.5), // 500/(100*10)
            },
        ];

        for (index, test) in cases.into_iter().enumerate() {
            let actual = calculate_pnl_return(
                test.pnl_realised.into(),
                test.price_entry_average.into(),
                test.quantity_abs_max.into(),
            );

            assert_eq!(actual, test.expected, "TC{} failed", index);
        }
    }
}
