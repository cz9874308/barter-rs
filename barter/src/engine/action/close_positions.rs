//! Engine 平仓操作模块
//!
//! 本模块定义了 Engine 如何生成和发送平仓订单请求。此操作根据提供的过滤器筛选要平仓的仓位，
//! 使用策略生成平仓订单，然后发送到执行管理器。
//!
//! # 核心概念
//!
//! - **ClosePositions**: Trait，定义平仓的接口
//! - **工作流程**: 筛选仓位 → 策略生成平仓订单 → 发送请求 → 记录在途请求
//!
//! # 注意事项
//!
//! 此操作**绕过风险检查**，直接发送平仓订单。这是因为平仓通常是风险管理的直接操作，
//! 不需要额外的风险检查。

use crate::{
    engine::{
        Engine,
        action::send_requests::{SendCancelsAndOpensOutput, SendRequests},
        execution_tx::ExecutionTxMap,
        state::{
            instrument::filter::InstrumentFilter,
            order::in_flight_recorder::InFlightRequestRecorder,
        },
    },
    strategy::close_positions::ClosePositionsStrategy,
};
use barter_instrument::{asset::AssetIndex, exchange::ExchangeIndex, instrument::InstrumentIndex};
use std::fmt::Debug;

/// 定义 [`Engine`] 如何生成和发送平仓订单请求的 Trait。
///
/// ClosePositions 定义了根据过滤器平仓的标准接口。此操作会筛选符合条件的仓位，
/// 使用策略生成平仓订单（取消未成交订单和开仓反向订单），并发送到执行管理器。
///
/// ## 类型参数
///
/// - `ExchangeKey`: 用于标识交易所的类型（默认为 [`ExchangeIndex`]）
/// - `AssetKey`: 用于标识资产的类型（默认为 [`AssetIndex`]）
/// - `InstrumentKey`: 用于标识交易对的类型（默认为 [`InstrumentIndex`]）
///
/// ## 注意事项
///
/// 此操作**绕过风险检查**，直接发送平仓订单。这是因为平仓通常是风险管理的直接操作。
///
/// # 使用示例
///
/// ```rust,ignore
/// // 平仓特定交易对的所有仓位
/// let output = engine.close_positions(&InstrumentFilter::new()
///     .with_instrument(instrument_index));
///
/// // 检查结果
/// if !output.is_empty() {
///     println!("Closed positions, sent {} orders", output.opens.sent.len());
/// }
/// ```
pub trait ClosePositions<
    ExchangeKey = ExchangeIndex,
    AssetKey = AssetIndex,
    InstrumentKey = InstrumentIndex,
>
{
    /// 生成并发送平仓订单请求。
    ///
    /// 此方法使用提供的 [`InstrumentFilter`] 来确定要平仓哪些仓位。
    ///
    /// ## 工作流程
    ///
    /// 1. 根据过滤器筛选仓位
    /// 2. 使用策略生成平仓订单（取消未成交订单和开仓反向订单）
    /// 3. 发送订单请求（绕过风险检查）
    /// 4. 记录在途订单请求
    ///
    /// # 参数
    ///
    /// - `filter`: 交易对过滤器，用于筛选要平仓的仓位
    ///
    /// # 返回值
    ///
    /// 返回 `SendCancelsAndOpensOutput`，包含发送的取消和开仓请求。
    fn close_positions(
        &mut self,
        filter: &InstrumentFilter<ExchangeKey, AssetKey, InstrumentKey>,
    ) -> SendCancelsAndOpensOutput<ExchangeKey, InstrumentKey>;
}

impl<Clock, State, ExecutionTxs, Strategy, Risk, ExchangeKey, AssetKey, InstrumentKey>
    ClosePositions<ExchangeKey, AssetKey, InstrumentKey>
    for Engine<Clock, State, ExecutionTxs, Strategy, Risk>
where
    State: InFlightRequestRecorder<ExchangeKey, InstrumentKey>,
    ExecutionTxs: ExecutionTxMap<ExchangeKey, InstrumentKey>,
    Strategy: ClosePositionsStrategy<ExchangeKey, AssetKey, InstrumentKey, State = State>,
    ExchangeKey: Debug + Clone,
    InstrumentKey: Debug + Clone,
{
    /// 平仓操作的实现。
    ///
    /// 此实现执行以下步骤：
    ///
    /// 1. **策略生成**: 使用策略生成平仓订单（取消未成交订单和开仓反向订单）
    /// 2. **发送请求**: 发送取消和开仓请求（绕过风险检查）
    /// 3. **记录在途**: 记录已发送的订单请求
    ///
    /// ## 平仓策略
    ///
    /// 平仓通常包括两个步骤：
    /// - **取消未成交订单**: 取消该交易对的所有未成交订单
    /// - **开仓反向订单**: 如果存在开放仓位，开仓反向订单来平仓
    ///
    /// ## 注意事项
    ///
    /// 此操作**绕过风险检查**，直接发送平仓订单。这是因为平仓通常是风险管理的直接操作。
    fn close_positions(
        &mut self,
        filter: &InstrumentFilter<ExchangeKey, AssetKey, InstrumentKey>,
    ) -> SendCancelsAndOpensOutput<ExchangeKey, InstrumentKey> {
        // 步骤1：使用策略生成平仓订单（取消未成交订单和开仓反向订单）
        let (cancels, opens) = self.strategy.close_positions_requests(&self.state, filter);

        // 步骤2：发送订单请求（绕过风险检查）
        let cancels = self.send_requests(cancels);
        let opens = self.send_requests(opens);

        // 步骤3：记录在途订单请求
        self.state.record_in_flight_cancels(&cancels.sent);
        self.state.record_in_flight_opens(&opens.sent);

        SendCancelsAndOpensOutput::new(cancels, opens)
    }
}
