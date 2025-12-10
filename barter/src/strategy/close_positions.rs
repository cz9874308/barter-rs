//! 平仓策略模块
//!
//! 本模块定义了平仓策略接口，用于生成平仓订单请求。策略可以根据不同的需求
//! 实现不同的平仓逻辑，如使用市价单、限价单，或通过反向相关交易对来对冲风险。
//!
//! # 核心概念
//!
//! - **ClosePositionsStrategy**: Trait，定义平仓策略接口
//! - **close_open_positions_with_market_orders**: 使用市价单平仓的简单实现
//! - **build_ioc_market_order_to_close_position**: 构建 IOC 市价单来平仓
//!
//! # 平仓策略选项
//!
//! 不同的策略可以实现不同的平仓方式：
//! - 使用不同的订单类型（市价单、限价单等）
//! - 优先选择某些交易所
//! - 通过反向相关交易对来对冲风险
//! - 等等

use crate::engine::state::{
    EngineState,
    instrument::{InstrumentState, data::InstrumentDataState, filter::InstrumentFilter},
    position::Position,
};
use barter_execution::order::{
    OrderKey, OrderKind, TimeInForce,
    id::{ClientOrderId, StrategyId},
    request::{OrderRequestCancel, OrderRequestOpen, RequestOpen},
};
use barter_instrument::{
    Side, asset::AssetIndex, exchange::ExchangeIndex, instrument::InstrumentIndex,
};
use rust_decimal::Decimal;

/// 生成用于平仓的开仓和取消订单请求的策略接口。
///
/// ClosePositionsStrategy 允许完全自定义策略如何平仓。不同的策略可以实现不同的
/// 平仓逻辑，以满足不同的交易需求。
///
/// ## 策略选项
///
/// 不同的策略可以实现：
/// - 使用不同的订单类型（市价单、限价单等）
/// - 优先选择某些交易所
/// - 通过增加反向相关交易对的仓位来对冲风险
/// - 等等
///
/// ## 类型参数
///
/// - `ExchangeKey`: 用于标识交易所的类型（默认为 [`ExchangeIndex`]）
/// - `AssetKey`: 用于标识资产的类型（默认为 [`AssetIndex`]）
/// - `InstrumentKey`: 用于标识交易对的类型（默认为 [`InstrumentIndex`]）
///
/// ## 关联类型
///
/// - **State**: 策略使用的状态类型，通常是完整的 `EngineState`
///
/// ## 实现示例
///
/// 对于 Barter 生态系统策略，State 通常是完整的交易系统 `EngineState`。
/// 例如：`EngineState<DefaultGlobalData, DefaultInstrumentMarketData>`
///
/// # 使用示例
///
/// ```rust,ignore
/// struct MyCloseStrategy {
///     // 策略配置
/// }
///
/// impl ClosePositionsStrategy for MyCloseStrategy {
///     type State = EngineState<DefaultGlobalData, DefaultInstrumentMarketData>;
///
///     fn close_positions_requests<'a>(
///         &'a self,
///         state: &'a Self::State,
///         filter: &'a InstrumentFilter,
///     ) -> (impl IntoIterator<Item = OrderRequestCancel> + 'a, impl IntoIterator<Item = OrderRequestOpen> + 'a) {
///         // 实现自定义平仓逻辑
///         (cancels, opens)
///     }
/// }
/// ```
pub trait ClosePositionsStrategy<
    ExchangeKey = ExchangeIndex,
    AssetKey = AssetIndex,
    InstrumentKey = InstrumentIndex,
>
{
    /// `ClosePositionsStrategy` 用于确定生成哪些开仓和取消请求的状态类型。
    ///
    /// 对于 Barter 生态系统策略，这是交易系统的完整 `EngineState`。
    ///
    /// 例如：`EngineState<DefaultGlobalData, DefaultInstrumentMarketData>`
    type State;

    /// 基于当前系统 `State` 生成平仓订单。
    ///
    /// 此方法根据过滤器和当前状态，生成用于平仓的取消和开仓订单请求。
    ///
    /// ## 返回值
    ///
    /// 返回一个元组，包含两个迭代器：
    /// - 第一个迭代器：取消订单请求（用于取消未成交订单）
    /// - 第二个迭代器：开仓订单请求（用于开仓反向订单来平仓）
    ///
    /// ## 注意事项
    ///
    /// - 生成的订单请求会绕过风险检查（平仓是风险管理操作）
    /// - 策略应该确保生成的订单能够有效平仓
    ///
    /// # 参数
    ///
    /// - `state`: 当前系统状态
    /// - `filter`: 交易对过滤器，用于筛选要平仓的仓位
    ///
    /// # 返回值
    ///
    /// 返回包含取消和开仓订单请求迭代器的元组。
    fn close_positions_requests<'a>(
        &'a self,
        state: &'a Self::State,
        filter: &'a InstrumentFilter<ExchangeKey, AssetKey, InstrumentKey>,
    ) -> (
        impl IntoIterator<Item = OrderRequestCancel<ExchangeKey, InstrumentKey>> + 'a,
        impl IntoIterator<Item = OrderRequestOpen<ExchangeKey, InstrumentKey>> + 'a,
    )
    where
        ExchangeKey: 'a,
        AssetKey: 'a,
        InstrumentKey: 'a;
}

/// 使用市价单平仓的简单 `ClosePositionsStrategy` 逻辑。
///
/// 此函数查找所有开放仓位，并生成相等但方向相反的 `Side` 市价单来平仓。
///
/// ## 工作原理
///
/// 1. 根据过滤器筛选交易对
///// 2. 对于每个有开放仓位的交易对，生成反向市价单
/// 3. 使用 IOC（立即成交或取消）订单类型确保快速执行
///
/// ## 注意事项
///
/// - 只生成开仓订单，不生成取消订单
/// - 需要有效的市场数据（价格）才能生成订单
/// - 使用市价单可能产生滑点
///
/// # 类型参数
///
/// - `GlobalData`: 全局数据类型
/// - `InstrumentData`: 交易对数据类型
///
/// # 参数
///
/// - `strategy_id`: 策略 ID
/// - `state`: 当前系统状态
/// - `filter`: 交易对过滤器
/// - `gen_cid`: 生成客户端订单 ID 的函数
///
/// # 返回值
///
/// 返回一个元组，包含空迭代器（取消订单）和开仓订单请求迭代器。
pub fn close_open_positions_with_market_orders<'a, GlobalData, InstrumentData>(
    strategy_id: &'a StrategyId,
    state: &'a EngineState<GlobalData, InstrumentData>,
    filter: &'a InstrumentFilter,
    gen_cid: impl Fn(&InstrumentState<InstrumentData>) -> ClientOrderId + Copy + 'a,
) -> (
    impl IntoIterator<Item = OrderRequestCancel<ExchangeIndex, InstrumentIndex>> + 'a,
    impl IntoIterator<Item = OrderRequestOpen<ExchangeIndex, InstrumentIndex>> + 'a,
)
where
    InstrumentData: InstrumentDataState,
    ExchangeIndex: 'a,
    AssetIndex: 'a,
    InstrumentIndex: 'a,
{
    // 为每个有开放仓位的交易对生成平仓订单
    let open_requests = state
        .instruments
        .instruments(filter)
        .filter_map(move |state| {
            // 只有当存在仓位且有市场数据时才生成订单
            let position = state.position.current.as_ref()?;
            let price = state.data.price()?;

            Some(build_ioc_market_order_to_close_position(
                state.instrument.exchange,
                position,
                strategy_id.clone(),
                price,
                || gen_cid(state),
            ))
        });

    (std::iter::empty(), open_requests)
}

/// 构建一个相等但方向相反的 `Side` `ImmediateOrCancel` `Market` 订单，用于平仓提供的 [`Position`]。
///
/// 此函数根据仓位方向生成反向市价单。例如，如果 [`Position`] 是多头 100，
/// 则构建一个卖出 100 的市价单请求。
///
/// ## 订单特性
///
/// - **订单类型**: 市价单（Market）
/// - **时间有效性**: IOC（ImmediateOrCancel，立即成交或取消）
/// - **方向**: 与仓位方向相反
/// - **数量**: 与仓位数量相等
///
/// ## 类型参数
///
/// - `ExchangeKey`: 交易所键类型
/// - `AssetKey`: 资产键类型
/// - `InstrumentKey`: 交易对键类型
///
/// # 参数
///
/// - `exchange`: 交易所标识
/// - `position`: 要平仓的仓位
/// - `strategy_id`: 策略 ID
/// - `price`: 当前市场价格（用于参考）
/// - `gen_cid`: 生成客户端订单 ID 的函数
///
/// # 返回值
///
/// 返回一个开仓订单请求，用于平仓指定的仓位。
///
/// # 使用示例
///
/// ```rust,ignore
/// let position = // ... 获取仓位
/// let price = // ... 获取当前价格
/// let order = build_ioc_market_order_to_close_position(
///     exchange,
///     &position,
///     strategy_id,
///     price,
///     || ClientOrderId::random(),
/// );
/// ```
pub fn build_ioc_market_order_to_close_position<ExchangeKey, AssetKey, InstrumentKey>(
    exchange: ExchangeKey,
    position: &Position<AssetKey, InstrumentKey>,
    strategy_id: StrategyId,
    price: Decimal,
    gen_cid: impl Fn() -> ClientOrderId,
) -> OrderRequestOpen<ExchangeKey, InstrumentKey>
where
    ExchangeKey: Clone,
    InstrumentKey: Clone,
{
    OrderRequestOpen {
        key: OrderKey {
            exchange: exchange.clone(),
            instrument: position.instrument.clone(),
            strategy: strategy_id,
            cid: gen_cid(),
        },
        state: RequestOpen {
            // 生成与仓位方向相反的订单方向
            side: match position.side {
                Side::Buy => Side::Sell,
                Side::Sell => Side::Buy,
            },
            price,
            // 使用仓位的绝对数量
            quantity: position.quantity_abs,
            kind: OrderKind::Market,
            time_in_force: TimeInForce::ImmediateOrCancel,
        },
    }
}
