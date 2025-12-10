//! Engine 生成算法订单操作模块
//!
//! 本模块定义了 Engine 如何生成和发送算法订单请求。这是 Engine 的核心操作之一，
//! 它将策略生成的订单请求经过风险管理检查后发送到执行管理器。
//!
//! # 核心概念
//!
//! - **GenerateAlgoOrders**: Trait，定义生成算法订单的接口
//! - **GenerateAlgoOrdersOutput**: 生成算法订单操作的输出
//! - **工作流程**: 策略生成 → 风险管理检查 → 发送请求 → 记录在途请求
//!
//! # 工作流程
//!
//! 1. 策略生成订单请求（取消和开仓）
//! 2. 风险管理检查订单请求
//! 3. 发送通过风险检查的订单请求
//! 4. 记录在途订单请求
//! 5. 返回操作结果（包括被拒绝的订单）

use crate::{
    engine::{
        Engine,
        action::send_requests::{SendCancelsAndOpensOutput, SendRequests, SendRequestsOutput},
        error::UnrecoverableEngineError,
        execution_tx::ExecutionTxMap,
        state::order::in_flight_recorder::InFlightRequestRecorder,
    },
    risk::{RiskApproved, RiskManager, RiskRefused},
    strategy::algo::AlgoStrategy,
};
use barter_execution::order::request::{
    OrderRequestCancel, OrderRequestOpen, RequestCancel, RequestOpen,
};
use barter_instrument::{exchange::ExchangeIndex, instrument::InstrumentIndex};
use barter_integration::collection::{none_one_or_many::NoneOneOrMany, one_or_many::OneOrMany};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// 定义 [`Engine`] 如何生成和发送算法订单请求的 Trait。
///
/// GenerateAlgoOrders 是 Engine 的核心操作接口，它定义了生成算法订单的完整流程：
/// 从策略生成订单请求，到风险管理检查，再到发送请求和执行。
///
/// ## 工作流程
///
/// 1. **策略生成**: 调用策略的 `generate_algo_orders()` 生成订单请求
/// 2. **风险管理**: 使用风险管理器检查订单请求
/// 3. **发送请求**: 发送通过风险检查的订单请求
/// 4. **记录在途**: 记录已发送的订单请求（用于跟踪）
///
/// ## 类型参数
///
/// - `ExchangeKey`: 用于标识交易所的类型（默认为 [`ExchangeIndex`]）
/// - `InstrumentKey`: 用于标识交易对的类型（默认为 [`InstrumentIndex`]）
///
/// # 使用示例
///
/// ```rust,ignore
/// // Engine 实现了 GenerateAlgoOrders
/// let output = engine.generate_algo_orders();
///
/// // 检查结果
/// if !output.is_empty() {
///     // 处理生成的订单
/// }
/// ```
pub trait GenerateAlgoOrders<ExchangeKey = ExchangeIndex, InstrumentKey = InstrumentIndex> {
    /// 生成并发送算法订单请求。
    ///
    /// 此方法执行完整的算法订单生成流程，包括策略生成、风险管理检查和请求发送。
    ///
    /// ## 返回值
    ///
    /// 返回 [`GenerateAlgoOrdersOutput`]，包含完成的工作：
    /// - 由 [`RiskManager`] 批准并发送执行的生成订单
    /// - 由 [`RiskManager`] 拒绝的生成取消请求
    /// - 由 [`RiskManager`] 拒绝的生成开仓请求
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let output = engine.generate_algo_orders();
    ///
    /// // 检查被拒绝的订单
    /// if let Some(refused) = output.opens_refused.as_ref() {
    ///     // 处理被拒绝的开仓请求
    /// }
    /// ```
    fn generate_algo_orders(&mut self) -> GenerateAlgoOrdersOutput<ExchangeKey, InstrumentKey>;
}

impl<Clock, State, ExecutionTxs, Strategy, Risk, ExchangeKey, InstrumentKey>
    GenerateAlgoOrders<ExchangeKey, InstrumentKey>
    for Engine<Clock, State, ExecutionTxs, Strategy, Risk>
where
    State: InFlightRequestRecorder<ExchangeKey, InstrumentKey>,
    ExecutionTxs: ExecutionTxMap<ExchangeKey, InstrumentKey>,
    Strategy: AlgoStrategy<ExchangeKey, InstrumentKey, State = State>,
    Risk: RiskManager<ExchangeKey, InstrumentKey, State = State>,
    ExchangeKey: Debug + Clone,
    InstrumentKey: Debug + Clone,
{
    /// 生成并发送算法订单请求的实现。
    ///
    /// 此实现执行以下步骤：
    ///
    /// 1. **策略生成**: 调用策略生成订单请求（取消和开仓）
    /// 2. **风险管理**: 使用风险管理器检查订单请求，分为批准和拒绝两类
    /// 3. **发送请求**: 发送通过风险检查的订单请求到执行管理器
    /// 4. **记录在途**: 记录已发送的订单请求，用于跟踪订单状态
    /// 5. **返回结果**: 返回包含所有结果的输出
    ///
    /// ## 错误处理
    ///
    /// 如果在发送请求时发生错误，错误会被包含在输出中，可以通过
    /// `unrecoverable_errors()` 方法提取。
    fn generate_algo_orders(&mut self) -> GenerateAlgoOrdersOutput<ExchangeKey, InstrumentKey> {
        // 步骤1：策略生成订单请求（取消和开仓）
        let (cancels, opens) = self.strategy.generate_algo_orders(&self.state);

        // 步骤2：风险管理检查订单请求（批准和拒绝）
        let (cancels, opens, refused_cancels, refused_opens) =
            self.risk.check(&self.state, cancels, opens);

        // 步骤3：发送通过风险检查的订单请求
        let cancels = self.send_requests(cancels.into_iter().map(|RiskApproved(cancel)| cancel));
        let opens = self.send_requests(opens.into_iter().map(|RiskApproved(open)| open));

        // 步骤4：收集剩余的迭代器（以便可以访问 &mut self）
        let cancels_refused = refused_cancels.into_iter().collect();
        let opens_refused = refused_opens.into_iter().collect();

        // 步骤5：记录在途订单请求（用于跟踪订单状态）
        self.state.record_in_flight_cancels(cancels.sent.iter());
        self.state.record_in_flight_opens(opens.sent.iter());

        // 步骤6：返回包含所有结果的输出
        GenerateAlgoOrdersOutput::new(cancels, opens, cancels_refused, opens_refused)
    }
}

/// [`Engine`] 操作 [`GenerateAlgoOrders::generate_algo_orders`] 完成的工作摘要。
///
/// GenerateAlgoOrdersOutput 包含算法订单生成操作的完整结果，包括成功的订单、
/// 被风险管理系统拒绝的订单，以及发生的任何错误。
///
/// ## 输出组成
///
/// - **cancels_and_opens**: 由 [`RiskManager`] 批准并发送执行的订单
/// - **cancels_refused**: 由 [`RiskManager`] 拒绝的取消请求
/// - **opens_refused**: 由 [`RiskManager`] 拒绝的开仓请求
///
/// ## 类型参数
///
/// - `ExchangeKey`: 交易所键类型，默认为 `ExchangeIndex`
/// - `InstrumentKey`: 交易对键类型，默认为 `InstrumentIndex`
///
/// ## 使用场景
///
/// - 检查生成的订单数量
/// - 分析被拒绝的订单原因
/// - 处理操作错误
///
/// # 使用示例
///
/// ```rust,ignore
/// let output = engine.generate_algo_orders();
///
/// // 检查是否为空
/// if !output.is_empty() {
///     // 处理生成的订单
/// }
///
/// // 检查被拒绝的订单
/// if let Some(refused) = output.opens_refused.as_ref() {
///     // 处理被拒绝的开仓请求
/// }
///
/// // 检查错误
/// if let Some(errors) = output.unrecoverable_errors() {
///     // 处理错误
/// }
/// ```
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub struct GenerateAlgoOrdersOutput<ExchangeKey = ExchangeIndex, InstrumentKey = InstrumentIndex> {
    /// 由 [`RiskManager`] 批准并发送执行的生成订单（取消和开仓）。
    pub cancels_and_opens: SendCancelsAndOpensOutput<ExchangeKey, InstrumentKey>,
    /// 由 [`RiskManager`] 拒绝的生成取消请求。
    pub cancels_refused: NoneOneOrMany<RiskRefused<OrderRequestCancel<ExchangeKey, InstrumentKey>>>,
    /// 由 [`RiskManager`] 拒绝的生成开仓请求。
    pub opens_refused: NoneOneOrMany<RiskRefused<OrderRequestOpen<ExchangeKey, InstrumentKey>>>,
}

impl<ExchangeKey, InstrumentKey> GenerateAlgoOrdersOutput<ExchangeKey, InstrumentKey> {
    /// 构造新的 [`GenerateAlgoOrdersOutput`]。
    ///
    /// # 参数
    ///
    /// - `cancels`: 取消请求的发送结果
    /// - `opens`: 开仓请求的发送结果
    /// - `cancels_refused`: 被拒绝的取消请求
    /// - `opens_refused`: 被拒绝的开仓请求
    ///
    /// # 返回值
    ///
    /// 返回新创建的 `GenerateAlgoOrdersOutput` 实例。
    pub fn new(
        cancels: SendRequestsOutput<RequestCancel, ExchangeKey, InstrumentKey>,
        opens: SendRequestsOutput<RequestOpen, ExchangeKey, InstrumentKey>,
        cancels_refused: NoneOneOrMany<RiskRefused<OrderRequestCancel<ExchangeKey, InstrumentKey>>>,
        opens_refused: NoneOneOrMany<RiskRefused<OrderRequestOpen<ExchangeKey, InstrumentKey>>>,
    ) -> Self {
        Self {
            cancels_and_opens: SendCancelsAndOpensOutput::new(cancels, opens),
            cancels_refused,
            opens_refused,
        }
    }

    /// 如果 `GenerateAlgoOrdersOutput` 完全为空，返回 `true`。
    ///
    /// 此方法检查输出是否包含任何订单或拒绝的请求。如果所有字段都为空，返回 `true`。
    ///
    /// # 返回值
    ///
    /// - `true`: 如果输出完全为空（没有订单，没有拒绝的请求）
    /// - `false`: 如果输出包含任何内容
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let output = engine.generate_algo_orders();
    ///
    /// if output.is_empty() {
    ///     // 没有生成任何订单
    /// }
    /// ```
    pub fn is_empty(&self) -> bool {
        self.cancels_and_opens.is_empty()
            && self.cancels_refused.is_none()
            && self.opens_refused.is_none()
    }

    /// 返回在订单请求生成和发送期间发生的任何不可恢复错误。
    ///
    /// 此方法从 `cancels_and_opens` 中提取不可恢复错误。这些错误通常需要立即处理。
    ///
    /// # 返回值
    ///
    /// - `Some(OneOrMany<UnrecoverableEngineError>)`: 如果发生了不可恢复错误
    /// - `None`: 如果没有发生错误
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let output = engine.generate_algo_orders();
    ///
    /// if let Some(errors) = output.unrecoverable_errors() {
    ///     // 处理错误
    /// }
    /// ```
    pub fn unrecoverable_errors(&self) -> Option<OneOrMany<UnrecoverableEngineError>> {
        self.cancels_and_opens.unrecoverable_errors().into_option()
    }
}

impl<ExchangeKey, InstrumentKey> Default for GenerateAlgoOrdersOutput<ExchangeKey, InstrumentKey> {
    fn default() -> Self {
        Self {
            cancels_and_opens: SendCancelsAndOpensOutput::default(),
            cancels_refused: NoneOneOrMany::None,
            opens_refused: NoneOneOrMany::None,
        }
    }
}
