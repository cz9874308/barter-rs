//! ExecutionRequest 执行请求模块
//!
//! 本模块定义了 Engine 与 ExecutionManager 之间的通信协议。
//! ExecutionRequest 是 Engine 向 ExecutionManager 发送的请求类型。
//!
//! # 核心概念
//!
//! - **ExecutionRequest**: Engine 向 ExecutionManager 发送的请求枚举
//! - **RequestFuture**: 带超时的请求 Future 包装器

use barter_execution::order::request::{OrderRequestCancel, OrderRequestOpen};
use barter_instrument::{exchange::ExchangeIndex, instrument::InstrumentIndex};
use derive_more::From;
use serde::{Deserialize, Serialize};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

/// 表示 `Engine` 向 `ExecutionManager` 发送的请求。
///
/// ExecutionRequest 是 Engine 与 ExecutionManager 之间的通信协议。
/// 它定义了所有可能的请求类型，包括关闭、取消订单和开仓订单。
///
/// ## 类型参数
///
/// - `ExchangeKey`: 交易所键类型，默认为 `ExchangeIndex`
/// - `InstrumentKey`: 交易对键类型，默认为 `InstrumentIndex`
///
/// ## 变体说明
///
/// - **Shutdown**: 请求 ExecutionManager 关闭
/// - **Cancel**: 请求取消现有订单
/// - **Open**: 请求开仓新订单
///
/// # 使用示例
///
/// ```rust,ignore
/// // 发送关闭请求
/// execution_tx.send(ExecutionRequest::Shutdown).await?;
///
/// // 发送取消订单请求
/// execution_tx.send(ExecutionRequest::Cancel(cancel_request)).await?;
///
/// // 发送开仓订单请求
/// execution_tx.send(ExecutionRequest::Open(open_request)).await?;
/// ```
#[derive(Debug, Clone, PartialEq, PartialOrd, Deserialize, Serialize, From)]
pub enum ExecutionRequest<ExchangeKey = ExchangeIndex, InstrumentKey = InstrumentIndex> {
    /// 请求 `ExecutionManager` 关闭。
    Shutdown,

    /// 请求取消现有 `Order`。
    Cancel(OrderRequestCancel<ExchangeKey, InstrumentKey>),

    /// 请求开仓新 `Order`。
    Open(OrderRequestOpen<ExchangeKey, InstrumentKey>),
}

/// 带超时的请求 Future 包装器。
///
/// RequestFuture 包装一个响应 Future，并为其添加超时功能。
/// 如果响应在超时时间内未完成，Future 会返回错误（原始请求）。
///
/// ## 类型参数
///
/// - `Request`: 请求类型
/// - `ResponseFut`: 响应 Future 类型
///
/// ## 工作原理
///
/// 1. 包装响应 Future 并添加超时
/// 2. 如果响应在超时前完成，返回响应结果
/// 3. 如果超时，返回错误（原始请求）
#[derive(Debug)]
#[pin_project::pin_project]
pub(super) struct RequestFuture<Request, ResponseFut> {
    /// 原始请求（用于超时错误）。
    request: Request,
    /// 带超时的响应 Future。
    #[pin]
    response_future: tokio::time::Timeout<ResponseFut>,
}

impl<Request, ResponseFut> Future for RequestFuture<Request, ResponseFut>
where
    Request: Clone,
    ResponseFut: Future,
{
    type Output = Result<ResponseFut::Output, Request>;

    /// 轮询 Future。
    ///
    /// 如果响应完成，返回 `Ok(response)`；如果超时，返回 `Err(request)`。
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        this.response_future
            .poll(cx)
            .map(|result| result.map_err(|_| this.request.clone()))
    }
}

impl<Request, ResponseFut> RequestFuture<Request, ResponseFut>
where
    ResponseFut: Future,
{
    /// 创建新的 RequestFuture。
    ///
    /// # 参数
    ///
    /// - `future`: 响应 Future
    /// - `timeout`: 超时时间
    /// - `request`: 原始请求（用于超时错误）
    ///
    /// # 返回值
    ///
    /// 返回新创建的 RequestFuture 实例。
    pub fn new(future: ResponseFut, timeout: std::time::Duration, request: Request) -> Self {
        Self {
            request,
            response_future: tokio::time::timeout(timeout, future),
        }
    }
}
