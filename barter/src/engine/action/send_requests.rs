//! Engine 发送请求操作模块
//!
//! 本模块定义了 Engine 如何向执行管理器发送订单请求。这是 Engine 与执行管理器之间的
//! 接口，负责将订单请求路由到正确的交易所执行通道。
//!
//! # 核心概念
//!
//! - **SendRequests**: Trait，定义发送请求的接口
//! - **SendRequestsOutput**: 发送请求操作的输出
//! - **SendCancelsAndOpensOutput**: 取消和开仓请求的合并输出
//! - **错误处理**: 区分可恢复和不可恢复错误
//!
//! # 工作流程
//!
//! 1. 查找对应交易所的执行通道
//! 2. 将订单请求转换为 ExecutionRequest
//! 3. 通过通道发送请求
//! 4. 处理发送错误（可恢复/不可恢复）

use crate::{
    engine::{
        Engine,
        error::{EngineError, RecoverableEngineError, UnrecoverableEngineError},
        execution_tx::ExecutionTxMap,
    },
    execution::request::ExecutionRequest,
};
use barter_execution::order::{
    OrderEvent,
    request::{RequestCancel, RequestOpen},
};
use barter_instrument::{exchange::ExchangeIndex, instrument::InstrumentIndex};
use barter_integration::{Unrecoverable, channel::Tx, collection::none_one_or_many::NoneOneOrMany};
use derive_more::Constructor;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use tracing::error;

/// 定义 [`Engine`] 如何发送订单请求的 Trait。
///
/// SendRequests 定义了 Engine 向执行管理器发送订单请求的标准接口。它支持发送单个请求
/// 或批量请求，并处理发送过程中的错误。
///
/// ## 类型参数
///
/// - `ExchangeKey`: 用于标识交易所的类型（默认为 [`ExchangeIndex`]）
/// - `InstrumentKey`: 用于标识交易对的类型（默认为 [`InstrumentIndex`]）
///
/// ## 错误处理
///
/// - **可恢复错误**: 通道不健康（临时性问题）
/// - **不可恢复错误**: 通道已终止（永久性问题）
///
/// # 使用示例
///
/// ```rust,ignore
/// // 发送单个请求
/// match engine.send_request(&order_request) {
///     Ok(()) => println!("Request sent"),
///     Err(error) => println!("Error: {}", error),
/// }
///
/// // 批量发送请求
/// let output = engine.send_requests(requests);
/// ```
pub trait SendRequests<ExchangeKey = ExchangeIndex, InstrumentKey = InstrumentIndex> {
    /// 批量发送订单请求。
    ///
    /// 此方法发送多个订单请求，并返回发送结果。成功发送的请求和被拒绝的请求都会被记录。
    ///
    /// # 类型参数
    ///
    /// - `Kind`: 请求类型（`RequestCancel` 或 `RequestOpen`）
    ///
    /// # 参数
    ///
    /// - `requests`: 订单请求迭代器
    ///
    /// # 返回值
    ///
    /// 返回 `SendRequestsOutput`，包含成功发送的请求和错误信息。
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let output = engine.send_requests(order_requests);
    /// println!("Sent: {} requests", output.sent.len());
    /// ```
    fn send_requests<Kind>(
        &self,
        requests: impl IntoIterator<Item = OrderEvent<Kind, ExchangeKey, InstrumentKey>>,
    ) -> SendRequestsOutput<Kind, ExchangeKey, InstrumentKey>
    where
        Kind: Debug + Clone,
        ExecutionRequest<ExchangeKey, InstrumentKey>:
            From<OrderEvent<Kind, ExchangeKey, InstrumentKey>>;

    /// 发送单个订单请求。
    ///
    /// 此方法发送单个订单请求到对应的交易所执行通道。
    ///
    /// ## 工作流程
    ///
    /// 1. 查找对应交易所的执行通道
    /// 2. 将订单请求转换为 `ExecutionRequest`
    /// 3. 通过通道发送请求
    /// 4. 处理发送错误
    ///
    /// ## 错误处理
    ///
    /// - **通道已终止**: 返回不可恢复错误
    /// - **通道不健康**: 返回可恢复错误
    ///
    /// # 类型参数
    ///
    /// - `Kind`: 请求类型（`RequestCancel` 或 `RequestOpen`）
    ///
    /// # 参数
    ///
    /// - `request`: 订单请求
    ///
    /// # 返回值
    ///
    /// - `Ok(())`: 请求发送成功
    /// - `Err(EngineError)`: 请求发送失败
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// match engine.send_request(&order_request) {
    ///     Ok(()) => println!("Request sent successfully"),
    ///     Err(EngineError::Unrecoverable(err)) => {
    ///         error!("Fatal error: {}", err);
    ///         engine.shutdown().await;
    ///     }
    ///     Err(EngineError::Recoverable(err)) => {
    ///         warn!("Recoverable error: {}", err);
    ///     }
    /// }
    /// ```
    fn send_request<Kind>(
        &self,
        request: &OrderEvent<Kind, ExchangeKey, InstrumentKey>,
    ) -> Result<(), EngineError>
    where
        Kind: Debug + Clone,
        ExecutionRequest<ExchangeKey, InstrumentKey>:
            From<OrderEvent<Kind, ExchangeKey, InstrumentKey>>;
}

impl<Clock, State, ExecutionTxs, Strategy, Risk, ExchangeKey, InstrumentKey>
    SendRequests<ExchangeKey, InstrumentKey> for Engine<Clock, State, ExecutionTxs, Strategy, Risk>
where
    ExecutionTxs: ExecutionTxMap<ExchangeKey, InstrumentKey>,
    ExchangeKey: Debug + Clone,
    InstrumentKey: Debug + Clone,
{
    /// 批量发送订单请求的实现。
    ///
    /// 此实现遍历所有请求，逐个发送，并将结果分为成功和失败两类。
    ///
    /// ## 工作原理
    ///
    /// 1. 遍历所有请求
    /// 2. 对每个请求调用 `send_request()`
    /// 3. 使用 `partition_result()` 将结果分为成功和失败
    /// 4. 返回包含所有结果的输出
    ///
    /// ## 错误处理
    ///
    /// 失败的请求会被记录在 `errors` 字段中，包含请求本身和错误信息。
    fn send_requests<Kind>(
        &self,
        requests: impl IntoIterator<Item = OrderEvent<Kind, ExchangeKey, InstrumentKey>>,
    ) -> SendRequestsOutput<Kind, ExchangeKey, InstrumentKey>
    where
        Kind: Debug + Clone,
        ExecutionRequest<ExchangeKey, InstrumentKey>:
            From<OrderEvent<Kind, ExchangeKey, InstrumentKey>>,
    {
        // 发送订单请求，将结果分为成功和失败
        let (sent, errors): (Vec<_>, Vec<_>) = requests
            .into_iter()
            .map(|request| {
                self.send_request(&request)
                    .map_err(|error| (request.clone(), error))
                    .map(|_| request)
            })
            .partition_result();

        SendRequestsOutput::new(NoneOneOrMany::from(sent), NoneOneOrMany::from(errors))
    }

    /// 发送单个订单请求的实现。
    ///
    /// 此实现执行以下步骤：
    ///
    /// 1. 查找对应交易所的执行通道
    /// 2. 将订单请求转换为 `ExecutionRequest`
    /// 3. 通过通道发送请求
    /// 4. 根据错误类型返回相应的 `EngineError`
    ///
    /// ## 错误处理
    ///
    /// - **通道已终止**: 返回 `UnrecoverableEngineError::ExecutionChannelTerminated`
    /// - **通道不健康**: 返回 `RecoverableEngineError::ExecutionChannelUnhealthy`
    /// - **通道不存在**: 返回 `UnrecoverableEngineError::IndexError`（从 `find()` 返回）
    fn send_request<Kind>(
        &self,
        request: &OrderEvent<Kind, ExchangeKey, InstrumentKey>,
    ) -> Result<(), EngineError>
    where
        Kind: Debug + Clone,
        ExecutionRequest<ExchangeKey, InstrumentKey>:
            From<OrderEvent<Kind, ExchangeKey, InstrumentKey>>,
    {
        // 查找执行通道并发送请求
        match self
            .execution_txs
            .find(&request.key.exchange)?
            .send(ExecutionRequest::from(request.clone()))
        {
            Ok(()) => Ok(()),
            // 通道已终止（不可恢复错误）
            Err(error) if error.is_unrecoverable() => {
                error!(
                    exchange = ?request.key.exchange,
                    ?request,
                    ?error,
                    "failed to send ExecutionRequest due to terminated channel"
                );
                Err(EngineError::Unrecoverable(
                    UnrecoverableEngineError::ExecutionChannelTerminated(format!(
                        "{:?} execution channel terminated: {:?}",
                        request.key.exchange, error
                    )),
                ))
            }
            // 通道不健康（可恢复错误）
            Err(error) => {
                error!(
                    exchange = ?request.key.exchange,
                    ?request,
                    ?error,
                    "failed to send ExecutionRequest due to unhealthy channel"
                );
                Err(EngineError::Recoverable(
                    RecoverableEngineError::ExecutionChannelUnhealthy(format!(
                        "{:?} execution channel unhealthy: {:?}",
                        request.key.exchange, error
                    )),
                ))
            }
        }
    }
}

/// [`Engine`] 发送给 `ExecutionManager` 的取消和开仓订单请求摘要。
///
/// SendCancelsAndOpensOutput 合并了取消请求和开仓请求的发送结果，提供统一的接口
/// 来访问两种类型的请求结果。
///
/// ## 类型参数
///
/// - `ExchangeKey`: 交易所键类型，默认为 `ExchangeIndex`
/// - `InstrumentKey`: 交易对键类型，默认为 `InstrumentIndex`
///
/// ## 使用场景
///
/// - 检查发送的取消和开仓请求数量
/// - 分析发送错误
/// - 处理操作结果
///
/// # 使用示例
///
/// ```rust,ignore
/// let output = SendCancelsAndOpensOutput::new(cancels, opens);
///
/// // 检查是否为空
/// if !output.is_empty() {
///     // 处理发送的请求
/// }
///
/// // 检查错误
/// if let Some(errors) = output.unrecoverable_errors().as_ref() {
///     // 处理错误
/// }
/// ```
#[derive(
    Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, Constructor,
)]
pub struct SendCancelsAndOpensOutput<ExchangeKey = ExchangeIndex, InstrumentKey = InstrumentIndex> {
    /// 已发送执行的取消订单请求。
    pub cancels: SendRequestsOutput<RequestCancel, ExchangeKey, InstrumentKey>,
    /// 已发送执行的开仓订单请求。
    pub opens: SendRequestsOutput<RequestOpen, ExchangeKey, InstrumentKey>,
}

impl<ExchangeKey, InstrumentKey> SendCancelsAndOpensOutput<ExchangeKey, InstrumentKey> {
    /// 如果 `SendCancelsAndOpensOutput` 完全为空，返回 `true`。
    ///
    /// 此方法检查取消请求和开仓请求是否都为空。
    ///
    /// # 返回值
    ///
    /// - `true`: 如果输出完全为空（没有发送的请求，没有错误）
    /// - `false`: 如果输出包含任何内容
    pub fn is_empty(&self) -> bool {
        self.cancels.is_empty() && self.opens.is_empty()
    }

    /// 返回在订单请求发送期间发生的任何不可恢复错误。
    ///
    /// 此方法从取消请求和开仓请求的错误中提取不可恢复错误，并合并返回。
    ///
    /// # 返回值
    ///
    /// 返回包含所有不可恢复错误的 `NoneOneOrMany`。
    pub fn unrecoverable_errors(&self) -> NoneOneOrMany<UnrecoverableEngineError> {
        self.cancels
            .unrecoverable_errors()
            .extend(self.opens.unrecoverable_errors())
    }
}

impl<ExchangeKey, InstrumentKey> Default for SendCancelsAndOpensOutput<ExchangeKey, InstrumentKey> {
    fn default() -> Self {
        Self {
            cancels: SendRequestsOutput::default(),
            opens: SendRequestsOutput::default(),
        }
    }
}

/// [`Engine`] 发送给 `ExecutionManager` 的订单请求（取消或开仓）摘要。
///
/// SendRequestsOutput 包含订单请求发送的完整结果，包括成功发送的请求和失败的请求。
///
/// ## 输出组成
///
/// - **sent**: 成功发送的订单请求
/// - **errors**: 发送失败的订单请求及其错误信息
///
/// ## 类型参数
///
/// - `Kind`: 请求类型（`RequestCancel` 或 `RequestOpen`）
/// - `ExchangeKey`: 交易所键类型，默认为 `ExchangeIndex`
/// - `InstrumentKey`: 交易对键类型，默认为 `InstrumentIndex`
///
/// ## 使用场景
///
/// - 检查发送的请求数量
/// - 分析发送错误
/// - 处理失败请求的重试
///
/// # 使用示例
///
/// ```rust,ignore
/// let output = engine.send_requests(requests);
///
/// // 检查发送的请求
/// if let Some(sent) = output.sent.as_ref() {
///     println!("Sent {} requests", sent.len());
/// }
///
/// // 检查错误
/// if let Some(errors) = output.errors.as_ref() {
///     for (request, error) in errors {
///         println!("Failed to send {:?}: {}", request, error);
///     }
/// }
/// ```
#[derive(
    Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, Constructor,
)]
pub struct SendRequestsOutput<Kind, ExchangeKey = ExchangeIndex, InstrumentKey = InstrumentIndex> {
    /// 成功发送的订单请求。
    pub sent: NoneOneOrMany<OrderEvent<Kind, ExchangeKey, InstrumentKey>>,
    /// 发送失败的订单请求及其错误信息。
    pub errors: NoneOneOrMany<(OrderEvent<Kind, ExchangeKey, InstrumentKey>, EngineError)>,
}

impl<Kind, ExchangeKey, InstrumentKey> SendRequestsOutput<Kind, ExchangeKey, InstrumentKey> {
    /// 如果 `SendRequestsOutput` 完全为空，返回 `true`。
    ///
    /// 此方法检查是否没有发送的请求，也没有错误。
    ///
    /// # 返回值
    ///
    /// - `true`: 如果输出完全为空
    /// - `false`: 如果输出包含任何内容
    pub fn is_empty(&self) -> bool {
        self.sent.is_none() && self.errors.is_none()
    }

    /// 返回在订单请求发送期间发生的任何不可恢复错误。
    ///
    /// 此方法从错误列表中过滤出不可恢复错误，忽略可恢复错误。
    ///
    /// # 返回值
    ///
    /// 返回包含所有不可恢复错误的 `NoneOneOrMany`。
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let output = engine.send_requests(requests);
    ///
    /// if let Some(errors) = output.unrecoverable_errors().as_ref() {
    ///     // 处理不可恢复错误
    ///     for error in errors {
    ///         error!("Unrecoverable error: {}", error);
    ///     }
    /// }
    /// ```
    pub fn unrecoverable_errors(&self) -> NoneOneOrMany<UnrecoverableEngineError> {
        // 从错误列表中过滤出不可恢复错误
        self.errors
            .iter()
            .filter_map(|(_order, error)| match error {
                EngineError::Unrecoverable(error) => Some(error.clone()),
                _ => None,
            })
            .collect()
    }
}

impl<ExchangeKey, InstrumentKey, Kind> Default
    for SendRequestsOutput<ExchangeKey, InstrumentKey, Kind>
{
    fn default() -> Self {
        Self {
            sent: NoneOneOrMany::default(),
            errors: NoneOneOrMany::default(),
        }
    }
}
