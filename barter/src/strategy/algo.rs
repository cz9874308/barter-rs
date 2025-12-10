//! 算法交易策略模块
//!
//! 本模块定义了算法交易策略接口，用于根据当前 EngineState 生成算法订单请求。
//! 这是策略系统的核心接口，所有算法交易策略都必须实现此接口。
//!
//! # 核心概念
//!
//! - **AlgoStrategy**: Trait，定义算法订单生成的接口
//! - **工作流程**: 分析状态 → 生成取消请求 → 生成开仓请求
//!
//! # 使用场景
//!
//! - 实现自定义交易策略
//! - 根据市场数据生成订单
//! - 动态调整交易决策

use barter_execution::order::request::{OrderRequestCancel, OrderRequestOpen};
use barter_instrument::{exchange::ExchangeIndex, instrument::InstrumentIndex};

/// 基于当前 `EngineState` 生成算法开仓和取消订单请求的策略接口。
///
/// AlgoStrategy 是策略系统的核心接口，定义了如何根据当前系统状态生成交易订单。
/// 实现此接口的策略可以分析市场数据、持仓状态等信息，并生成相应的订单请求。
///
/// ## 工作流程
///
/// 1. 分析当前 `EngineState`（市场数据、持仓、订单等）
/// 2. 根据策略逻辑决定需要取消的订单
/// 3. 根据策略逻辑决定需要开仓的订单
/// 4. 返回取消和开仓订单请求的迭代器
///
/// ## 类型参数
///
/// - `ExchangeKey`: 用于标识交易所的类型（默认为 [`ExchangeIndex`]）
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
/// struct MyStrategy {
///     // 策略配置
/// }
///
/// impl AlgoStrategy for MyStrategy {
///     type State = EngineState<DefaultGlobalData, DefaultInstrumentMarketData>;
///
///     fn generate_algo_orders(
///         &self,
///         state: &Self::State,
///     ) -> (impl IntoIterator<Item = OrderRequestCancel>, impl IntoIterator<Item = OrderRequestOpen>) {
///         // 分析状态并生成订单
///         let cancels = self.analyze_and_generate_cancels(state);
///         let opens = self.analyze_and_generate_opens(state);
///         (cancels, opens)
///     }
/// }
/// ```
pub trait AlgoStrategy<ExchangeKey = ExchangeIndex, InstrumentKey = InstrumentIndex> {
    /// `AlgoStrategy` 用于确定生成哪些开仓和取消请求的状态类型。
    ///
    /// 对于 Barter 生态系统策略，这是交易系统的完整 `EngineState`。
    ///
    /// 例如：`EngineState<DefaultGlobalData, DefaultInstrumentMarketData>`
    type State;

    /// 基于当前系统 `State` 生成算法订单。
    ///
    /// 此方法分析当前状态，并根据策略逻辑生成取消和开仓订单请求。
    ///
    /// ## 返回值
    ///
    /// 返回一个元组，包含两个迭代器：
    /// - 第一个迭代器：取消订单请求
    /// - 第二个迭代器：开仓订单请求
    ///
    /// ## 注意事项
    ///
    /// - 生成的订单请求会经过风险管理检查
    /// - 只有通过风险检查的订单才会被发送执行
    /// - 策略应该只生成必要的订单，避免过度交易
    ///
    /// # 参数
    ///
    /// - `state`: 当前系统状态
    ///
    /// # 返回值
    ///
    /// 返回包含取消和开仓订单请求迭代器的元组。
    fn generate_algo_orders(
        &self,
        state: &Self::State,
    ) -> (
        impl IntoIterator<Item = OrderRequestCancel<ExchangeKey, InstrumentKey>>,
        impl IntoIterator<Item = OrderRequestOpen<ExchangeKey, InstrumentKey>>,
    );
}
