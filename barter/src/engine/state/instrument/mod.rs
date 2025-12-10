//! Engine 交易对状态模块
//!
//! 本模块定义了交易对状态管理，用于跟踪每个交易对的仓位、订单、统计信息和自定义数据。
//!
//! # 核心概念
//!
//! - **InstrumentStates**: 交易对状态集合，维护所有交易对的状态
//! - **InstrumentState**: 单个交易对的状态，包括仓位、订单、统计信息和自定义数据
//! - **InstrumentDataState**: 交易对数据状态接口，用于自定义交易对数据
//! - **InstrumentFilter**: 交易对过滤器，用于筛选交易对数据
//!
//! # 使用场景
//!
//! - 跟踪交易对的仓位和订单
//! - 更新交易对状态
//! - 生成交易对统计摘要
//! - 管理自定义交易对数据

use crate::{
    engine::state::{
        instrument::{data::InstrumentDataState, filter::InstrumentFilter},
        order::{Orders, manager::OrderManager},
        position::{PositionExited, PositionManager},
    },
    statistic::summary::instrument::TearSheetGenerator,
};
use barter_data::event::MarketEvent;
use barter_execution::{
    InstrumentAccountSnapshot,
    order::{
        Order, OrderKey,
        request::OrderResponseCancel,
        state::{ActiveOrderState, OrderState},
    },
    trade::Trade,
};
use barter_instrument::{
    Keyed,
    asset::{AssetIndex, QuoteAsset, name::AssetNameExchange},
    exchange::{ExchangeId, ExchangeIndex},
    index::IndexedInstruments,
    instrument::{
        Instrument, InstrumentIndex,
        name::{InstrumentNameExchange, InstrumentNameInternal},
    },
};
use barter_integration::{collection::FnvIndexMap, snapshot::Snapshot};
use chrono::{DateTime, Utc};
use derive_more::Constructor;
use itertools::Either;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// 定义状态接口 [`InstrumentDataState`]，可用于实现自定义的交易对级别数据状态。
pub mod data;

/// 定义 `InstrumentFilter`，用于过滤以交易对为中心的数据结构。
pub mod filter;

/// 按 [`InstrumentIndex`] 索引的 [`InstrumentState`] 集合。
///
/// InstrumentStates 维护所有交易对的状态映射。注意，具有相同 [`InstrumentNameExchange`]
/// （例如 "btc_usdt"）但在不同交易所上的交易对会有各自的 [`InstrumentState`]。
///
/// ## 索引方式
///
/// 使用 `InstrumentNameInternal` 作为键，这样可以区分不同交易所的同名交易对。
///
/// ## 类型参数
///
/// - `InstrumentData`: 用户自定义的交易对数据类型
/// - `ExchangeKey`: 交易所键类型，默认为 `ExchangeIndex`
/// - `AssetKey`: 资产键类型，默认为 `AssetIndex`
/// - `InstrumentKey`: 交易对键类型，默认为 `InstrumentIndex`
///
/// ## 使用场景
///
/// - 跟踪所有交易对的状态
/// - 更新交易对状态
/// - 生成交易对统计摘要
/// - 过滤和查询交易对
///
/// # 使用示例
///
/// ```rust,ignore
/// let instrument_states = InstrumentStates::default();
///
/// // 获取特定交易对的状态
/// let btc_usdt_state = instrument_states.instrument_index(&btc_usdt_index);
///
/// // 更新交易对状态
/// instrument_states.instrument_index_mut(&btc_usdt_index).update_from_trade(&trade);
///
/// // 过滤交易对
/// for state in instrument_states.instruments(&InstrumentFilter::Exchanges(...)) {
///     // 处理筛选的交易对
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct InstrumentStates<
    InstrumentData,
    ExchangeKey = ExchangeIndex,
    AssetKey = AssetIndex,
    InstrumentKey = InstrumentIndex,
>(
    /// 以 InstrumentNameInternal 为键的交易对状态映射
    pub  FnvIndexMap<
        InstrumentNameInternal,
        InstrumentState<InstrumentData, ExchangeKey, AssetKey, InstrumentKey>,
    >,
);

impl<InstrumentData> InstrumentStates<InstrumentData> {
    /// 返回与 `InstrumentIndex` 关联的 `InstrumentState` 的引用。
    ///
    /// 如果与 `InstrumentIndex` 关联的 `InstrumentState` 不存在，则 panic。
    ///
    /// # 参数
    ///
    /// - `key`: 交易对索引
    ///
    /// # 返回值
    ///
    /// 返回交易对状态的不可变引用。
    pub fn instrument_index(&self, key: &InstrumentIndex) -> &InstrumentState<InstrumentData> {
        self.0
            .get_index(key.index())
            .map(|(_key, state)| state)
            .unwrap_or_else(|| panic!("InstrumentStates does not contain: {key}"))
    }

    /// 返回与 `InstrumentIndex` 关联的 `InstrumentState` 的可变引用。
    ///
    /// 如果与 `InstrumentIndex` 关联的 `InstrumentState` 不存在，则 panic。
    ///
    /// # 参数
    ///
    /// - `key`: 交易对索引
    ///
    /// # 返回值
    ///
    /// 返回交易对状态的可变引用。
    pub fn instrument_index_mut(
        &mut self,
        key: &InstrumentIndex,
    ) -> &mut InstrumentState<InstrumentData> {
        self.0
            .get_index_mut(key.index())
            .map(|(_key, state)| state)
            .unwrap_or_else(|| panic!("InstrumentStates does not contain: {key}"))
    }

    /// Return a reference to the `InstrumentState` associated with an `InstrumentNameInternal`.
    ///
    /// Panics if `InstrumentState` associated with the `InstrumentNameInternal` does not exist.
    pub fn instrument(&self, key: &InstrumentNameInternal) -> &InstrumentState<InstrumentData> {
        self.0
            .get(key)
            .unwrap_or_else(|| panic!("InstrumentStates does not contain: {key}"))
    }

    /// Return a mutable reference to the `InstrumentState` associated with an
    /// `InstrumentNameInternal`.
    ///
    /// Panics if `InstrumentState` associated with the `InstrumentNameInternal` does not exist.
    pub fn instrument_mut(
        &mut self,
        key: &InstrumentNameInternal,
    ) -> &mut InstrumentState<InstrumentData> {
        self.0
            .get_mut(key)
            .unwrap_or_else(|| panic!("InstrumentStates does not contain: {key}"))
    }

    /// 返回被跟踪的 `InstrumentState` 的引用迭代器，可选择性地通过提供的 `InstrumentFilter` 过滤。
    ///
    /// # 参数
    ///
    /// - `filter`: 交易对过滤器
    ///
    /// # 返回值
    ///
    /// 返回过滤后的交易对状态迭代器。
    pub fn instruments<'a>(
        &'a self,
        filter: &'a InstrumentFilter,
    ) -> impl Iterator<Item = &'a InstrumentState<InstrumentData>> {
        self.filtered(filter)
    }

    /// 返回被跟踪的 `InstrumentState` 的可变引用迭代器，可选择性地通过提供的 `InstrumentFilter` 过滤。
    ///
    /// # 参数
    ///
    /// - `filter`: 交易对过滤器
    ///
    /// # 返回值
    ///
    /// 返回过滤后的交易对状态可变引用迭代器。
    pub fn instruments_mut<'a>(
        &'a mut self,
        filter: &'a InstrumentFilter,
    ) -> impl Iterator<Item = &'a mut InstrumentState<InstrumentData>> {
        self.filtered_mut(filter)
    }

    /// Return an `Iterator` of references to instrument `TearSheetGenerator`s, optionally
    /// filtered by the provided `InstrumentFilter`.
    pub fn tear_sheets<'a>(
        &'a self,
        filter: &'a InstrumentFilter,
    ) -> impl Iterator<Item = &'a TearSheetGenerator>
    where
        InstrumentData: 'a,
    {
        self.filtered(filter).map(|state| &state.tear_sheet)
    }

    /// Return an `Iterator` of references to instrument `PositionManager`s, optionally
    /// filtered by the provided `InstrumentFilter`.
    pub fn positions<'a>(
        &'a self,
        filter: &'a InstrumentFilter,
    ) -> impl Iterator<Item = &'a PositionManager>
    where
        InstrumentData: 'a,
    {
        self.filtered(filter).map(|state| &state.position)
    }

    /// Return an `Iterator` of references to instrument `Orders`, optionally filtered by the
    /// provided `InstrumentFilter`.
    pub fn orders<'a>(&'a self, filter: &'a InstrumentFilter) -> impl Iterator<Item = &'a Orders>
    where
        InstrumentData: 'a,
    {
        self.filtered(filter).map(|state| &state.orders)
    }

    /// Return an `Iterator` of references to custom instrument level data state, optionally
    /// filtered by the provided `InstrumentFilter`.
    pub fn instrument_datas<'a>(
        &'a self,
        filter: &'a InstrumentFilter,
    ) -> impl Iterator<Item = &'a InstrumentData>
    where
        InstrumentData: 'a,
    {
        self.filtered(filter).map(|state| &state.data)
    }

    /// Return an `Iterator` of mutable references to custom instrument level data state,
    /// optionally filtered by the provided `InstrumentFilter`.
    pub fn instrument_datas_mut<'a>(
        &'a mut self,
        filter: &'a InstrumentFilter,
    ) -> impl Iterator<Item = &'a mut InstrumentData>
    where
        InstrumentData: 'a,
    {
        self.filtered_mut(filter).map(|state| &mut state.data)
    }

    /// Return a filtered `Iterator` of `InstrumentState`s based on the provided `InstrumentFilter`.
    fn filtered<'a>(
        &'a self,
        filter: &'a InstrumentFilter,
    ) -> impl Iterator<Item = &'a InstrumentState<InstrumentData>>
    where
        InstrumentData: 'a,
    {
        use filter::InstrumentFilter::*;
        match filter {
            None => Either::Left(Either::Left(self.0.values())),
            Exchanges(exchanges) => Either::Left(Either::Right(
                self.0
                    .values()
                    .filter(|state| exchanges.contains(&state.instrument.exchange)),
            )),
            Instruments(instruments) => Either::Right(Either::Right(
                self.0
                    .values()
                    .filter(|state| instruments.contains(&state.key)),
            )),
            Underlyings(underlying) => Either::Right(Either::Left(
                self.0
                    .values()
                    .filter(|state| underlying.contains(&state.instrument.underlying)),
            )),
        }
    }

    /// Return a filtered `Iterator` of mutable `InstrumentState`s based on the
    /// provided `InstrumentFilter`.
    fn filtered_mut<'a>(
        &'a mut self,
        filter: &'a InstrumentFilter,
    ) -> impl Iterator<Item = &'a mut InstrumentState<InstrumentData>>
    where
        InstrumentData: 'a,
    {
        use filter::InstrumentFilter::*;
        match filter {
            None => Either::Left(Either::Left(self.0.values_mut())),
            Exchanges(exchanges) => Either::Left(Either::Right(
                self.0
                    .values_mut()
                    .filter(|state| exchanges.contains(&state.instrument.exchange)),
            )),
            Instruments(instruments) => Either::Right(Either::Right(
                self.0
                    .values_mut()
                    .filter(|state| instruments.contains(&state.key)),
            )),
            Underlyings(underlying) => Either::Right(Either::Left(
                self.0
                    .values_mut()
                    .filter(|state| underlying.contains(&state.instrument.underlying)),
            )),
        }
    }
}

/// 表示交易对的当前状态，包括其 [`Position`](super::position::Position)、[`Orders`] 和
/// 用户提供的交易对数据。
///
/// InstrumentState 聚合单个交易对的所有状态和数据，提供交易对的全面视图。
///
/// ## 状态组成
///
/// - **key**: 交易对的唯一标识符
/// - **instrument**: 完整的交易对定义
/// - **tear_sheet**: 交易性能摘要生成器
/// - **position**: 当前仓位管理器
/// - **orders**: 活跃订单和订单管理
/// - **data**: 用户自定义的交易对数据（市场数据、策略数据、风险数据等）
///
/// ## 类型参数
///
/// - `InstrumentData`: 用户自定义的交易对数据类型，必须实现 `InstrumentDataState`
/// - `ExchangeKey`: 交易所键类型，默认为 `ExchangeIndex`
/// - `AssetKey`: 资产键类型，默认为 `AssetIndex`
/// - `InstrumentKey`: 交易对键类型，默认为 `InstrumentIndex`
///
/// ## 使用场景
///
/// - 跟踪交易对的完整状态
/// - 更新仓位、订单和自定义数据
/// - 生成交易对统计摘要
///
/// # 使用示例
///
/// ```rust,ignore
/// let mut instrument_state = InstrumentState::new(
///     instrument_key,
///     instrument,
///     TearSheetGenerator::default(),
///     PositionManager::default(),
///     Orders::default(),
///     instrument_data,
/// );
///
/// // 从交易更新状态
/// instrument_state.update_from_trade(&trade);
///
/// // 从市场事件更新状态
/// instrument_state.update_from_market(&market_event);
/// ```
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Constructor)]
pub struct InstrumentState<
    InstrumentData,
    ExchangeKey = ExchangeIndex,
    AssetKey = AssetIndex,
    InstrumentKey = InstrumentIndex,
> {
    /// 此状态关联的交易对的唯一 `InstrumentKey` 标识符。
    pub key: InstrumentKey,

    /// 完整的交易对定义。
    pub instrument: Instrument<ExchangeKey, AssetKey>,

    /// 用于汇总与交易对相关的交易性能的 TearSheet 生成器。
    pub tear_sheet: TearSheetGenerator,

    /// 当前 `PositionManager`（仓位管理器）。
    pub position: PositionManager<InstrumentKey>,

    /// 活跃订单和相关的订单管理。
    pub orders: Orders<ExchangeKey, InstrumentKey>,

    /// 用户提供的交易对级别数据状态。可以包括市场数据、策略数据、风险数据、
    /// 期权定价数据或任何其他交易对特定信息。
    pub data: InstrumentData,
}

impl<InstrumentData, ExchangeKey, AssetKey, InstrumentKey>
    InstrumentState<InstrumentData, ExchangeKey, AssetKey, InstrumentKey>
{
    /// 使用来自交易所的账户快照更新交易对状态。
    ///
    /// 此方法更新交易对的活跃订单，在相关情况下使用时间戳以确保应用最新的订单状态。
    ///
    /// ## 工作原理
    ///
    /// 遍历快照中的所有订单，逐个调用 `update_from_order_snapshot` 更新订单状态。
    ///
    /// # 参数
    ///
    /// - `snapshot`: 交易对账户快照
    ///
    /// # 类型约束
    ///
    /// - `ExchangeKey`: 必须实现 `Debug + Clone`
    /// - `InstrumentKey`: 必须实现 `Debug + Clone`
    /// - `AssetKey`: 必须实现 `Debug + Clone`
    pub fn update_from_account_snapshot(
        &mut self,
        snapshot: &InstrumentAccountSnapshot<ExchangeKey, AssetKey, InstrumentKey>,
    ) where
        ExchangeKey: Debug + Clone,
        InstrumentKey: Debug + Clone,
        AssetKey: Debug + Clone,
    {
        // 遍历快照中的所有订单并更新
        for order in &snapshot.orders {
            self.update_from_order_snapshot(Snapshot(order))
        }
    }

    /// 从 [`Order`] 快照更新交易对状态。
    ///
    /// 此方法将订单快照转发给订单管理器进行更新。
    ///
    /// # 参数
    ///
    /// - `order`: 订单快照
    ///
    /// # 类型约束
    ///
    /// - `ExchangeKey`: 必须实现 `Debug + Clone`
    /// - `AssetKey`: 必须实现 `Debug + Clone`
    /// - `InstrumentKey`: 必须实现 `Debug + Clone`
    pub fn update_from_order_snapshot(
        &mut self,
        order: Snapshot<&Order<ExchangeKey, InstrumentKey, OrderState<AssetKey, InstrumentKey>>>,
    ) where
        ExchangeKey: Debug + Clone,
        AssetKey: Debug + Clone,
        InstrumentKey: Debug + Clone,
    {
        self.orders.update_from_order_snapshot(order);
    }

    /// 从 [`OrderRequestCancel`](barter_execution::order::request::OrderRequestCancel) 响应更新交易对状态。
    ///
    /// 此方法将取消响应转发给订单管理器进行更新。
    ///
    /// # 参数
    ///
    /// - `response`: 订单取消响应
    ///
    /// # 类型约束
    ///
    /// - `ExchangeKey`: 必须实现 `Debug + Clone`
    /// - `AssetKey`: 必须实现 `Debug + Clone`
    /// - `InstrumentKey`: 必须实现 `Debug + Clone`
    pub fn update_from_cancel_response(
        &mut self,
        response: &OrderResponseCancel<ExchangeKey, AssetKey, InstrumentKey>,
    ) where
        ExchangeKey: Debug + Clone,
        AssetKey: Debug + Clone,
        InstrumentKey: Debug + Clone,
    {
        self.orders
            .update_from_cancel_response::<AssetKey>(response);
    }

    /// 基于新交易更新交易对状态。
    ///
    /// 此方法处理：
    /// - 基于新交易开仓/更新当前仓位状态
    /// - 如果仓位退出，更新内部的 [`TearSheetGenerator`]
    ///
    /// ## 工作原理
    ///
    /// 1. 调用 `position.update_from_trade()` 更新仓位
    /// 2. 如果仓位退出，使用 `inspect` 更新 TearSheet
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
    /// if let Some(position_exited) = instrument_state.update_from_trade(&trade) {
    ///     // 处理已平仓的仓位
    /// }
    /// ```
    pub fn update_from_trade(
        &mut self,
        trade: &Trade<QuoteAsset, InstrumentKey>,
    ) -> Option<PositionExited<QuoteAsset, InstrumentKey>>
    where
        InstrumentKey: Debug + Clone + PartialEq,
    {
        // 更新仓位，如果仓位退出则更新 TearSheet
        self.position
            .update_from_trade(trade)
            .inspect(|closed| self.tear_sheet.update_from_position(closed))
    }

    /// 基于新市场事件更新交易对状态。
    ///
    /// 此方法处理市场事件并更新交易对数据。如果市场事件包含价格（例如 `PublicTrade`、
    /// `OrderBookL1`），则重新计算任何开放 [`Position`](super::position::Position) 的
    /// `pnl_unrealised`（未实现盈亏）。
    ///
    /// ## 工作原理
    ///
    /// 1. 使用市场事件更新交易对数据（`data.process()`）
    /// 2. 如果存在开放仓位，从数据中提取价格
    /// 3. 如果价格可用，更新仓位的未实现盈亏
    ///
    /// # 参数
    ///
    /// - `event`: 市场事件
    ///
    /// # 类型约束
    ///
    /// - `InstrumentData`: 必须实现 `InstrumentDataState`
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// // 处理市场事件
    /// instrument_state.update_from_market(&market_event);
    ///
    /// // 如果市场事件包含价格，仓位的未实现盈亏会自动更新
    /// ```
    pub fn update_from_market(
        &mut self,
        event: &MarketEvent<InstrumentKey, InstrumentData::MarketEventKind>,
    ) where
        InstrumentData: InstrumentDataState<ExchangeKey, AssetKey, InstrumentKey>,
    {
        // 更新交易对数据
        self.data.process(event);

        // 如果存在开放仓位，尝试更新未实现盈亏
        let Some(position) = &mut self.position.current else {
            return;
        };

        // 从交易对数据中提取价格
        let Some(price) = self.data.price() else {
            return;
        };

        // 使用价格更新未实现盈亏
        position.update_pnl_unrealised(price);
    }
}

/// 从未索引的交易对状态生成未索引的交易对账户快照。
///
/// 此函数将索引化的 `InstrumentState` 转换为未索引的 `InstrumentAccountSnapshot`，
/// 用于生成账户快照或与外部系统交互。
///
/// ## 转换过程
///
/// 1. 提取交易对的交易所名称
/// 2. 收集所有活跃的 Open 订单
/// 3. 将订单转换为未索引格式（使用交易所名称而不是索引）
///
/// ## 注意事项
///
/// - 只包含状态为 `Open` 的订单
/// - 订单键中的交易所和交易对使用名称而不是索引
///
/// # 类型参数
///
/// - `InstrumentData`: 交易对数据类型
/// - `ExchangeKey`: 交易所键类型
/// - `AssetKey`: 资产键类型
/// - `InstrumentKey`: 交易对键类型
///
/// # 参数
///
/// - `exchange`: 交易所 ID
/// - `state`: 交易对状态
///
/// # 返回值
///
/// 返回未索引的交易对账户快照。
///
/// # 使用示例
///
/// ```rust,ignore
/// let snapshot = generate_unindexed_instrument_account_snapshot(
///     exchange_id,
///     &instrument_state,
/// );
/// ```
pub fn generate_unindexed_instrument_account_snapshot<
    InstrumentData,
    ExchangeKey,
    AssetKey,
    InstrumentKey,
>(
    exchange: ExchangeId,
    state: &InstrumentState<InstrumentData, ExchangeKey, AssetKey, InstrumentKey>,
) -> InstrumentAccountSnapshot<ExchangeId, AssetNameExchange, InstrumentNameExchange>
where
    ExchangeKey: Debug + Clone,
    InstrumentKey: Debug + Clone,
{
    let InstrumentState {
        key: _,
        instrument,
        tear_sheet: _,
        position: _,
        orders,
        data: _,
    } = state;

    InstrumentAccountSnapshot {
        instrument: instrument.name_exchange.clone(),
        // 收集所有活跃的 Open 订单，转换为未索引格式
        orders: orders
            .orders()
            .filter_map(|order| {
                // 只处理 Open 状态的订单
                let Order {
                    key,
                    side,
                    price,
                    quantity,
                    kind,
                    time_in_force,
                    state: ActiveOrderState::Open(open),
                } = order
                else {
                    return None;
                };

                // 转换为未索引格式（使用交易所名称和交易对名称）
                Some(Order {
                    key: OrderKey {
                        exchange,
                        instrument: instrument.name_exchange.clone(),
                        strategy: key.strategy.clone(),
                        cid: key.cid.clone(),
                    },
                    side: *side,
                    price: *price,
                    quantity: *quantity,
                    kind: *kind,
                    time_in_force: *time_in_force,
                    state: OrderState::active(open.clone()),
                })
            })
            .collect(),
    }
}

/// 生成索引化的 [`InstrumentStates`]。
///
/// 此函数为提供的交易对集合中的每个交易对创建 `InstrumentState`，使用提供的初始化函数
/// 来创建仓位管理器、订单管理器和交易对数据。
///
/// ## 初始化过程
///
/// 1. 遍历所有交易对
/// 2. 为每个交易对创建 `InstrumentState`，包括：
///    - 交易对键和定义
///    - TearSheet 生成器（使用 `time_engine_start` 初始化）
///    - 仓位管理器（使用 `position_manager_init` 创建）
///    - 订单管理器（使用 `orders_init` 创建）
///    - 交易对数据（使用 `instrument_data_init` 创建）
///
/// ## 类型参数
///
/// - `'a`: 生命周期参数，绑定到 `IndexedInstruments` 的生命周期
/// - `FnPosMan`: 仓位管理器初始化函数类型
/// - `FnOrders`: 订单管理器初始化函数类型
/// - `FnInsData`: 交易对数据初始化函数类型
/// - `InstrumentData`: 交易对数据类型
///
/// # 参数
///
/// - `instruments`: 索引化的交易对集合
/// - `time_engine_start`: Engine 启动时间，用于初始化 TearSheet
/// - `position_manager_init`: 仓位管理器初始化函数
/// - `orders_init`: 订单管理器初始化函数
/// - `instrument_data_init`: 交易对数据初始化函数
///
/// # 返回值
///
/// 返回新创建的 `InstrumentStates`，包含所有交易对的状态。
///
/// # 使用示例
///
/// ```rust,ignore
/// let instrument_states = generate_indexed_instrument_states(
///     &indexed_instruments,
///     time_engine_start,
///     PositionManager::default,
///     Orders::default,
///     |instrument| InstrumentData::new(instrument),
/// );
/// ```
pub fn generate_indexed_instrument_states<'a, FnPosMan, FnOrders, FnInsData, InstrumentData>(
    instruments: &'a IndexedInstruments,
    time_engine_start: DateTime<Utc>,
    position_manager_init: FnPosMan,
    orders_init: FnOrders,
    instrument_data_init: FnInsData,
) -> InstrumentStates<InstrumentData>
where
    FnPosMan: Fn() -> PositionManager,
    FnOrders: Fn() -> Orders,
    FnInsData: Fn(
        &'a Keyed<InstrumentIndex, Instrument<Keyed<ExchangeIndex, ExchangeId>, AssetIndex>>,
    ) -> InstrumentData,
{
    InstrumentStates(
        // 遍历所有交易对，为每个交易对创建状态
        instruments
            .instruments()
            .iter()
            .map(|instrument| {
                // 提取交易所索引
                let exchange_index = instrument.value.exchange.key;

                (
                    // 使用内部名称作为键
                    instrument.value.name_internal.clone(),
                    // 创建交易对状态
                    InstrumentState::new(
                        instrument.key,
                        // 将交易所键从 ExchangeId 映射到 ExchangeIndex
                        instrument.value.clone().map_exchange_key(exchange_index),
                        // 使用启动时间初始化 TearSheet
                        TearSheetGenerator::init(time_engine_start),
                        // 使用初始化函数创建仓位管理器
                        position_manager_init(),
                        // 使用初始化函数创建订单管理器
                        orders_init(),
                        // 使用初始化函数创建交易对数据
                        instrument_data_init(instrument),
                    ),
                )
            })
            .collect(),
    )
}
