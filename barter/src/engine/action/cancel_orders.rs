//! Engine 取消订单操作模块
//!
//! 本模块定义了 Engine 如何取消开仓订单请求。此操作根据提供的过滤器筛选要取消的订单，
//! 然后发送取消请求到执行管理器。
//!
//! # 核心概念
//!
//! - **CancelOrders**: Trait，定义取消订单的接口
//! - **工作流程**: 筛选订单 → 生成取消请求 → 发送请求 → 记录在途请求
//!
//! # 注意事项
//!
//! 此操作**绕过风险检查**，直接发送取消请求。这是因为取消订单通常是安全的操作，
//! 不需要额外的风险检查。

use crate::engine::{
    Engine,
    action::send_requests::{SendRequests, SendRequestsOutput},
    execution_tx::ExecutionTxMap,
    state::{
        EngineState,
        instrument::filter::InstrumentFilter,
        order::{in_flight_recorder::InFlightRequestRecorder, manager::OrderManager},
    },
};
use barter_execution::order::{Order, request::RequestCancel};
use barter_instrument::{asset::AssetIndex, exchange::ExchangeIndex, instrument::InstrumentIndex};

/// 定义 [`Engine`] 如何取消开仓订单请求的 Trait。
///
/// CancelOrders 定义了根据过滤器取消订单的标准接口。此操作会筛选符合条件的订单，
/// 生成取消请求，并发送到执行管理器。
///
/// ## 类型参数
///
/// - `ExchangeKey`: 用于标识交易所的类型（默认为 [`ExchangeIndex`]）
/// - `AssetKey`: 用于标识资产的类型（默认为 [`AssetIndex`]）
/// - `InstrumentKey`: 用于标识交易对的类型（默认为 [`InstrumentIndex`]）
///
/// ## 注意事项
///
/// 此操作**绕过风险检查**，直接发送取消请求。这是因为取消订单通常是安全的操作。
///
/// # 使用示例
///
/// ```rust,ignore
/// // 取消特定交易对的所有订单
/// let output = engine.cancel_orders(&InstrumentFilter::new()
///     .with_instrument(instrument_index));
///
/// // 检查结果
/// if !output.is_empty() {
///     println!("Cancelled {} orders", output.sent.len());
/// }
/// ```
pub trait CancelOrders<
    ExchangeKey = ExchangeIndex,
    AssetKey = AssetIndex,
    InstrumentKey = InstrumentIndex,
>
{
    /// 生成取消订单请求。
    ///
    /// 此方法使用提供的 [`InstrumentFilter`] 来确定要取消哪些订单。
    ///
    /// ## 工作流程
    ///
    /// 1. 根据过滤器筛选订单
    /// 2. 将订单转换为取消请求
    /// 3. 发送取消请求（绕过风险检查）
    /// 4. 记录在途取消请求
    ///
    /// # 参数
    ///
    /// - `filter`: 交易对过滤器，用于筛选要取消的订单
    ///
    /// # 返回值
    ///
    /// 返回 `SendRequestsOutput`，包含发送的取消请求和错误信息。
    fn cancel_orders(
        &mut self,
        filter: &InstrumentFilter<ExchangeKey, AssetKey, InstrumentKey>,
    ) -> SendRequestsOutput<RequestCancel, ExchangeKey, InstrumentKey>;
}

impl<Clock, GlobalData, InstrumentData, ExecutionTxs, Strategy, Risk> CancelOrders
    for Engine<Clock, EngineState<GlobalData, InstrumentData>, ExecutionTxs, Strategy, Risk>
where
    InstrumentData: InFlightRequestRecorder,
    ExecutionTxs: ExecutionTxMap,
{
    /// 取消订单操作的实现。
    ///
    /// 此实现执行以下步骤：
    ///
    /// 1. **筛选订单**: 根据过滤器筛选符合条件的订单
    /// 2. **生成取消请求**: 将订单转换为取消请求
    /// 3. **发送请求**: 发送取消请求（绕过风险检查）
    /// 4. **记录在途**: 记录已发送的取消请求
    ///
    /// ## 注意事项
    ///
    /// 此操作**绕过风险检查**，直接发送取消请求。这是因为取消订单通常是安全的操作。
    fn cancel_orders(
        &mut self,
        filter: &InstrumentFilter<ExchangeIndex, AssetIndex, InstrumentIndex>,
    ) -> SendRequestsOutput<RequestCancel, ExchangeIndex, InstrumentIndex> {
        // 步骤1-2：根据过滤器筛选订单并生成取消请求
        let requests = self
            .state
            .instruments
            .orders(filter)
            .flat_map(|state| state.orders().filter_map(Order::to_request_cancel));

        // 步骤3：发送订单请求（绕过风险检查）
        let cancels = self.send_requests(requests);

        // 步骤4：记录在途订单请求
        self.state.record_in_flight_cancels(&cancels.sent);

        cancels
    }
}
