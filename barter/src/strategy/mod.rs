//! Strategy 策略模块
//!
//! 本模块定义了 Engine 的策略接口，包括算法交易策略、平仓策略、断开连接处理策略
//! 和交易禁用处理策略。策略是 Engine 的核心组件，负责根据当前状态生成交易决策。
//!
//! # 核心概念
//!
//! - **AlgoStrategy**: 算法交易策略，根据状态生成订单请求
//! - **ClosePositionsStrategy**: 平仓策略，生成平仓订单请求
//! - **OnDisconnectStrategy**: 断开连接处理策略
//! - **OnTradingDisabled**: 交易禁用处理策略
//! - **DefaultStrategy**: 默认策略实现（仅用于演示）
//!
//! # 策略接口
//!
//! 策略接口定义了 Engine 在不同场景下的行为：
//! - 算法订单生成
//! - 平仓操作
//! - 异常情况处理

use crate::{
    engine::{
        Engine,
        state::{
            EngineState,
            instrument::{data::InstrumentDataState, filter::InstrumentFilter},
        },
    },
    strategy::{
        algo::AlgoStrategy,
        close_positions::{ClosePositionsStrategy, close_open_positions_with_market_orders},
        on_disconnect::OnDisconnectStrategy,
        on_trading_disabled::OnTradingDisabled,
    },
};
use barter_execution::order::{
    id::{ClientOrderId, StrategyId},
    request::{OrderRequestCancel, OrderRequestOpen},
};
use barter_instrument::{
    asset::AssetIndex,
    exchange::{ExchangeId, ExchangeIndex},
    instrument::InstrumentIndex,
};
use std::marker::PhantomData;

/// 定义基于当前 `EngineState` 生成算法开仓和取消订单请求的策略接口。
pub mod algo;

/// 定义生成用于平仓的开仓和取消订单请求的策略接口。
pub mod close_positions;

/// 定义在交易所断开连接时执行自定义 [`Engine`] 操作的策略接口。
pub mod on_disconnect;

/// 定义在 `TradingState` 设置为 `TradingState::Disabled` 时执行自定义 [`Engine`] 操作的策略接口。
pub mod on_trading_disabled;

/// 所有策略接口的简单实现。
///
/// **仅用于演示目的，切勿用于真实交易或生产环境**。
///
/// 此策略的行为：
/// - 不生成算法订单（AlgoStrategy）
/// - 通过简单的 [`close_open_positions_with_market_orders`] 逻辑平仓（ClosePositionsStrategy）
/// - 交易所断开连接时不执行任何操作（OnDisconnectStrategy）
/// - 交易状态设置为禁用时不执行任何操作（OnTradingDisabled）
///
/// ## 类型参数
///
/// - `State`: Engine 状态类型
///
/// ## 使用场景
///
/// 仅用于测试和演示，不应在生产环境中使用。
///
/// # 警告
///
/// ⚠️ **此策略不执行任何实际的交易逻辑，仅用于演示系统架构。**
/// 在生产环境中，必须实现自定义策略来处理实际的交易决策。
///
/// # 使用示例
///
/// ```rust,ignore
/// // 仅用于测试
/// let strategy = DefaultStrategy::default();
/// ```
#[derive(Debug, Clone)]
pub struct DefaultStrategy<State> {
    /// 策略 ID。
    pub id: StrategyId,
    /// 状态类型标记。
    phantom: PhantomData<State>,
}

impl<State> Default for DefaultStrategy<State> {
    fn default() -> Self {
        Self {
            id: StrategyId::new("default"),
            phantom: PhantomData,
        }
    }
}

impl<State, ExchangeKey, InstrumentKey> AlgoStrategy<ExchangeKey, InstrumentKey>
    for DefaultStrategy<State>
{
    type State = State;

    /// DefaultStrategy 的算法订单生成实现。
    ///
    /// 此实现不生成任何订单，返回空迭代器。
    fn generate_algo_orders(
        &self,
        _: &Self::State,
    ) -> (
        impl IntoIterator<Item = OrderRequestCancel<ExchangeKey, InstrumentKey>>,
        impl IntoIterator<Item = OrderRequestOpen<ExchangeKey, InstrumentKey>>,
    ) {
        (std::iter::empty(), std::iter::empty())
    }
}

impl<GlobalData, InstrumentData> ClosePositionsStrategy
    for DefaultStrategy<EngineState<GlobalData, InstrumentData>>
where
    InstrumentData: InstrumentDataState,
{
    type State = EngineState<GlobalData, InstrumentData>;

    /// DefaultStrategy 的平仓请求生成实现。
    ///
    /// 此实现使用简单的市场订单逻辑来平仓，通过 [`close_open_positions_with_market_orders`]
    /// 函数生成平仓订单请求。
    fn close_positions_requests<'a>(
        &'a self,
        state: &'a Self::State,
        filter: &'a InstrumentFilter,
    ) -> (
        impl IntoIterator<Item = OrderRequestCancel<ExchangeIndex, InstrumentIndex>> + 'a,
        impl IntoIterator<Item = OrderRequestOpen<ExchangeIndex, InstrumentIndex>> + 'a,
    )
    where
        ExchangeIndex: 'a,
        AssetIndex: 'a,
        InstrumentIndex: 'a,
    {
        close_open_positions_with_market_orders(&self.id, state, filter, |_| {
            ClientOrderId::random()
        })
    }
}

impl<Clock, State, ExecutionTxs, Risk> OnDisconnectStrategy<Clock, State, ExecutionTxs, Risk>
    for DefaultStrategy<State>
{
    type OnDisconnect = ();

    /// DefaultStrategy 的断开连接处理实现。
    ///
    /// 此实现不执行任何操作，直接返回。
    fn on_disconnect(
        _: &mut Engine<Clock, State, ExecutionTxs, Self, Risk>,
        _: ExchangeId,
    ) -> Self::OnDisconnect {
    }
}

impl<Clock, State, ExecutionTxs, Risk> OnTradingDisabled<Clock, State, ExecutionTxs, Risk>
    for DefaultStrategy<State>
{
    type OnTradingDisabled = ();

    /// DefaultStrategy 的交易禁用处理实现。
    ///
    /// 此实现不执行任何操作，直接返回。
    fn on_trading_disabled(
        _: &mut Engine<Clock, State, ExecutionTxs, Self, Risk>,
    ) -> Self::OnTradingDisabled {
    }
}
