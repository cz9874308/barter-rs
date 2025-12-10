//! Risk 风险管理模块
//!
//! 本模块定义了 Engine 的风险管理系统，用于审查和过滤策略生成的订单请求。
//! 风险管理系统是交易系统的关键组件，确保所有订单都符合风险控制要求。
//!
//! # 核心概念
//!
//! - **RiskManager**: Trait，定义风险管理器接口
//! - **RiskApproved**: 通过风险检查的订单请求
//! - **RiskRefused**: 被风险管理系统拒绝的订单请求（包含拒绝原因）
//! - **DefaultRiskManager**: 默认风险管理器（仅用于演示，不执行任何检查）
//!
//! # 风险管理功能
//!
//! 风险管理器可以实现以下功能：
//! - 过滤会导致过度风险的订单
//! - 过滤数量过大的订单
//! - 调整订单数量
//! - 过滤会穿越订单簿的订单
//! - 等等

use barter_execution::order::request::{OrderRequestCancel, OrderRequestOpen};
use barter_instrument::{exchange::ExchangeIndex, instrument::InstrumentIndex};
use barter_integration::Unrecoverable;
use derive_more::{Constructor, Display, From};
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, hash::Hash, marker::PhantomData};

/// RiskManager 检查和工具。
pub mod check;

/// 审查并可选地过滤由 [`AlgoStrategy`](super::strategy::algo::AlgoStrategy) 生成的
/// 取消和开仓订单请求的 RiskManager 接口。
///
/// RiskManager 是交易系统的关键组件，负责确保所有订单都符合风险控制要求。
/// 它会在订单发送执行之前进行最后的风险检查。
///
/// ## 风险管理功能
///
/// 风险管理器实现可以：
/// - 过滤会导致过度风险的订单
/// - 过滤数量过大的订单
/// - 调整订单数量
/// - 过滤会穿越订单簿的订单
/// - 检查持仓限制
/// - 检查资金限制
/// - 等等
///
/// ## 类型参数
///
/// - `ExchangeKey`: 用于标识交易所的类型（默认为 [`ExchangeIndex`]）
/// - `InstrumentKey`: 用于标识交易对的类型（默认为 [`InstrumentIndex`]）
///
/// ## 关联类型
///
/// - **State**: 风险管理器使用的状态类型，通常是完整的 `EngineState`
///
/// ## 返回值
///
/// 返回一个元组，包含四个迭代器：
/// - 第一个迭代器：通过风险检查的取消订单请求
/// - 第二个迭代器：通过风险检查的开仓订单请求
/// - 第三个迭代器：被拒绝的取消订单请求（包含拒绝原因）
/// - 第四个迭代器：被拒绝的开仓订单请求（包含拒绝原因）
///
/// # 使用示例
///
/// ```rust,ignore
/// struct MyRiskManager {
///     // 风险配置
/// }
///
/// impl<ExchangeKey, InstrumentKey> RiskManager<ExchangeKey, InstrumentKey> for MyRiskManager {
///     type State = EngineState<DefaultGlobalData, DefaultInstrumentMarketData>;
///
///     fn check(
///         &self,
///         state: &Self::State,
///         cancels: impl IntoIterator<Item = OrderRequestCancel>,
///         opens: impl IntoIterator<Item = OrderRequestOpen>,
///     ) -> (impl IntoIterator<Item = RiskApproved<OrderRequestCancel>>, ...) {
///         // 实现风险检查逻辑
///     }
/// }
/// ```
pub trait RiskManager<ExchangeKey = ExchangeIndex, InstrumentKey = InstrumentIndex> {
    /// 风险管理器使用的状态类型。
    type State;

    /// 检查订单请求并分类为批准或拒绝。
    ///
    /// 此方法对策略生成的所有订单请求进行风险检查，并将它们分为通过和拒绝两类。
    ///
    /// ## 工作流程
    ///
    /// 1. 分析当前系统状态
    /// 2. 对每个订单请求执行风险检查
    /// 3. 根据检查结果分类订单请求
    /// 4. 返回批准和拒绝的订单请求
    ///
    /// ## 注意事项
    ///
    /// - 只有通过风险检查的订单才会被发送执行
    /// - 被拒绝的订单会包含拒绝原因，可用于日志和调试
    ///
    /// # 参数
    ///
    /// - `state`: 当前系统状态
    /// - `cancels`: 取消订单请求迭代器
    /// - `opens`: 开仓订单请求迭代器
    ///
    /// # 返回值
    ///
    /// 返回包含四个迭代器的元组：批准的取消订单、批准的开仓订单、
    /// 拒绝的取消订单、拒绝的开仓订单。
    fn check(
        &self,
        state: &Self::State,
        cancels: impl IntoIterator<Item = OrderRequestCancel<ExchangeKey, InstrumentKey>>,
        opens: impl IntoIterator<Item = OrderRequestOpen<ExchangeKey, InstrumentKey>>,
    ) -> (
        impl IntoIterator<Item = RiskApproved<OrderRequestCancel<ExchangeKey, InstrumentKey>>>,
        impl IntoIterator<Item = RiskApproved<OrderRequestOpen<ExchangeKey, InstrumentKey>>>,
        impl IntoIterator<Item = RiskRefused<OrderRequestCancel<ExchangeKey, InstrumentKey>>>,
        impl IntoIterator<Item = RiskRefused<OrderRequestOpen<ExchangeKey, InstrumentKey>>>,
    );
}

/// 包装已通过 [`RiskManager`] 检查的订单请求的新类型。
///
/// RiskApproved 是一个标记类型，用于标识已通过风险检查的订单请求。
/// 这些订单请求可以安全地发送到执行管理器。
///
/// ## 类型参数
///
/// - `T`: 被包装的订单请求类型（`OrderRequestCancel` 或 `OrderRequestOpen`）
///
/// ## 使用场景
///
/// - 标识通过风险检查的订单
/// - 类型安全的风险管理流程
///
/// # 使用示例
///
/// ```rust,ignore
/// let approved = RiskApproved::new(order_request);
/// let order = approved.into_item();
/// ```
#[derive(
    Debug,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Deserialize,
    Serialize,
    Display,
    From,
    Constructor,
)]
pub struct RiskApproved<T>(pub T);

impl<T> RiskApproved<T> {
    /// 提取被包装的订单请求。
    ///
    /// # 返回值
    ///
    /// 返回被包装的订单请求。
    pub fn into_item(self) -> T {
        self.0
    }
}

/// 包装未通过 [`RiskManager`] 检查的订单请求的类型，包括失败原因。
///
/// RiskRefused 用于标记被风险检查拒绝的订单请求，并记录拒绝原因。
/// 这对于日志记录、审计和调试非常重要。
///
/// ## 类型参数
///
/// - `T`: 订单请求类型（`OrderRequestCancel` 或 `OrderRequestOpen`）
/// - `Reason`: 拒绝原因类型，默认为 `String`
///
/// ## 使用场景
///
/// - 记录被拒绝的订单及其原因
/// - 审计和合规
/// - 调试和问题排查
///
/// # 使用示例
///
/// ```rust,ignore
/// let refused = RiskRefused::new(
///     order_request,
///     "Order size exceeds maximum allowed"
/// );
/// println!("Order refused: {}", refused.reason);
/// ```
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub struct RiskRefused<T, Reason = String> {
    /// 被拒绝的订单请求。
    pub item: T,
    /// 拒绝原因。
    pub reason: Reason,
}

impl<T> RiskRefused<T> {
    /// 创建新的 RiskRefused 实例。
    ///
    /// # 参数
    ///
    /// - `item`: 被拒绝的订单请求
    /// - `reason`: 拒绝原因（会被转换为 String）
    ///
    /// # 返回值
    ///
    /// 返回新创建的 RiskRefused 实例。
    pub fn new(item: T, reason: impl Into<String>) -> Self {
        Self {
            item,
            reason: reason.into(),
        }
    }
}

impl<T, Reason> RiskRefused<T, Reason> {
    /// 提取内部订单请求。
    ///
    /// # 返回值
    ///
    /// 返回被拒绝的订单请求。
    pub fn into_item(self) -> T {
        self.item
    }
}

impl<T, Reason> Unrecoverable for RiskRefused<T, Reason>
where
    Reason: Unrecoverable,
{
    /// 检查拒绝原因是否为不可恢复错误。
    ///
    /// 如果拒绝原因本身是不可恢复错误，则整个 RiskRefused 也被视为不可恢复。
    fn is_unrecoverable(&self) -> bool {
        self.reason.is_unrecoverable()
    }
}

/// [`RiskManager`] 接口的简单实现，**不进行任何风险检查**就批准所有订单。
///
/// **仅用于演示目的，切勿用于真实交易或生产环境**。
///
/// 此实现会批准所有订单请求，不进行任何风险检查。这对于测试和演示系统架构
/// 很有用，但绝对不能用于实际交易。
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
/// ⚠️ **此风险管理器不执行任何风险检查，会批准所有订单。**
/// 在生产环境中，必须实现自定义风险管理器来执行实际的风险检查。
///
/// # 使用示例
///
/// ```rust,ignore
/// // 仅用于测试
/// let risk_manager = DefaultRiskManager::default();
/// ```
#[derive(Debug, Clone)]
pub struct DefaultRiskManager<State> {
    /// 状态类型标记。
    phantom: PhantomData<State>,
}

impl<State> Default for DefaultRiskManager<State> {
    /// 创建默认的 DefaultRiskManager 实例。
    fn default() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<State, ExchangeKey, InstrumentKey> RiskManager<ExchangeKey, InstrumentKey>
    for DefaultRiskManager<State>
{
    type State = State;

    /// DefaultRiskManager 的风险检查实现。
    ///
    /// 此实现不进行任何风险检查，直接批准所有订单请求。
    fn check(
        &self,
        _: &Self::State,
        cancels: impl IntoIterator<Item = OrderRequestCancel<ExchangeKey, InstrumentKey>>,
        opens: impl IntoIterator<Item = OrderRequestOpen<ExchangeKey, InstrumentKey>>,
    ) -> (
        impl IntoIterator<Item = RiskApproved<OrderRequestCancel<ExchangeKey, InstrumentKey>>>,
        impl IntoIterator<Item = RiskApproved<OrderRequestOpen<ExchangeKey, InstrumentKey>>>,
        impl IntoIterator<Item = RiskRefused<OrderRequestCancel<ExchangeKey, InstrumentKey>>>,
        impl IntoIterator<Item = RiskRefused<OrderRequestOpen<ExchangeKey, InstrumentKey>>>,
    ) {
        (
            cancels.into_iter().map(RiskApproved::new),
            opens.into_iter().map(RiskApproved::new),
            std::iter::empty(),
            std::iter::empty(),
        )
    }
}
