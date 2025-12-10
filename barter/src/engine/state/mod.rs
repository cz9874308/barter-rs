//! Engine 状态管理模块
//!
//! 本模块定义了 Engine 的完整状态结构，包括交易状态、连接状态、资产状态、交易对状态等。
//! EngineState 是 Engine 的核心数据结构，维护了所有交易相关的状态信息。
//!
//! # 核心概念
//!
//! - **EngineState**: Engine 的完整状态，包含所有子状态
//! - **TradingState**: 交易状态（启用/禁用）
//! - **ConnectivityStates**: 连接状态（市场数据和账户连接的健康状态）
//! - **AssetStates**: 资产状态（每个资产的状态）
//! - **InstrumentStates**: 交易对状态（每个交易对的状态）
//! - **GlobalData**: 用户自定义的全局数据
//! - **InstrumentData**: 用户自定义的交易对数据
//!
//! # 状态更新流程
//!
//! 1. **账户事件更新**: `update_from_account()` 处理账户事件，更新资产和交易对状态
//! 2. **市场事件更新**: `update_from_market()` 处理市场事件，更新交易对数据
//! 3. **状态转换**: 根据事件类型自动更新连接状态、资产状态、交易对状态
//!
//! # 使用场景
//!
//! - 维护 Engine 的完整状态
//! - 处理账户和市场事件
//! - 生成账户快照
//! - 状态查询和过滤

use crate::engine::{
    Processor,
    state::{
        asset::{AssetStates, filter::AssetFilter},
        builder::EngineStateBuilder,
        connectivity::ConnectivityStates,
        instrument::{
            InstrumentStates, data::InstrumentDataState, filter::InstrumentFilter,
            generate_unindexed_instrument_account_snapshot,
        },
        position::PositionExited,
        trading::TradingState,
    },
};
use barter_data::event::MarketEvent;
use barter_execution::{
    AccountEvent, AccountEventKind, UnindexedAccountSnapshot, balance::AssetBalance,
};
use barter_instrument::{
    Keyed,
    asset::{AssetIndex, QuoteAsset},
    exchange::{ExchangeId, ExchangeIndex},
    index::IndexedInstruments,
    instrument::{Instrument, InstrumentIndex},
};
use barter_integration::{collection::one_or_many::OneOrMany, snapshot::Snapshot};
use derive_more::Constructor;
use fnv::FnvHashMap;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// 资产中心的状态及其相关的状态管理逻辑。
pub mod asset;

/// 连接状态，跟踪全局连接健康状态以及每个交易所的市场数据和账户连接状态。
pub mod connectivity;

/// 交易对级别的状态及其相关的状态管理逻辑。
pub mod instrument;

/// 定义同步的 `OrderManager`，跟踪交易所订单的生命周期。
pub mod order;

/// 仓位数据结构和相关的状态管理逻辑。
pub mod position;

/// 定义 `Engine` 的 `TradingState`（即交易启用和交易禁用），及其更新逻辑。
pub mod trading;

/// [`EngineState`] 构建器工具。
pub mod builder;

/// 定义默认的 `GlobalData` 实现，可用于不需要特定全局数据的系统。
pub mod global;

/// 算法交易 `Engine` 的状态。
///
/// EngineState 是 Engine 的核心数据结构，维护了所有交易相关的状态信息。它包含了
/// 交易状态、连接状态、资产状态、交易对状态等所有子状态。
///
/// ## 状态组成
///
/// - **trading**: 当前交易状态（启用/禁用）
/// - **global**: 用户自定义的全局数据
/// - **connectivity**: 连接健康状态（全局和每个交易所）
/// - **assets**: 所有资产的状态（如 "btc", "usdt" 等）
/// - **instruments**: 所有交易对的状态（如 "okx_spot_btc_usdt" 等）
///
/// ## 类型参数
///
/// - `GlobalData`: 用户自定义的全局数据类型，必须实现 `Processor<&AccountEvent>` 和 `Processor<&MarketEvent>`
/// - `InstrumentData`: 用户自定义的交易对数据类型，必须实现 `InstrumentDataState`
///
/// ## 状态更新
///
/// EngineState 通过以下方法更新状态：
///
/// - `update_from_account()`: 从账户事件更新状态
/// - `update_from_market()`: 从市场事件更新状态
///
/// ## 使用场景
///
/// - 维护 Engine 的完整状态
/// - 处理账户和市场事件
/// - 生成账户快照
/// - 状态查询和过滤
///
/// # 使用示例
///
/// ```rust,ignore
/// // 使用构建器创建 EngineState
/// let engine_state = EngineState::builder(
///     &indexed_instruments,
///     global_data,
///     |instrument| InstrumentData::new(instrument),
/// )
/// .build();
///
/// // 更新状态
/// engine_state.update_from_account(&account_event);
/// engine_state.update_from_market(&market_event);
///
/// // 访问状态
/// let trading_state = engine_state.trading;
/// let asset_state = engine_state.assets.asset_index(&asset_index);
/// ```
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Constructor)]
pub struct EngineState<GlobalData, InstrumentData> {
    /// 当前 `Engine` 的 `TradingState`（交易启用/禁用）。
    pub trading: TradingState,

    /// 可配置的 `GlobalData` 状态（用户自定义的全局数据）。
    pub global: GlobalData,

    /// 全局连接 [`Health`](connectivity::Health)，以及每个交易所的市场数据和账户连接的健康状态。
    pub connectivity: ConnectivityStates,

    /// 被 `Engine` 跟踪的每个资产的状态（例如 "btc", "usdt" 等）。
    pub assets: AssetStates,

    /// 被 `Engine` 跟踪的每个交易对的状态（例如 "okx_spot_btc_usdt", "bybit_perpetual_btc_usdt" 等）。
    pub instruments: InstrumentStates<InstrumentData, ExchangeIndex, AssetIndex, InstrumentIndex>,
}

impl<GlobalData, InstrumentData> EngineState<GlobalData, InstrumentData> {
    /// 构造一个 [`EngineStateBuilder`] 以辅助 `EngineState` 的初始化。
    ///
    /// 此方法提供了一个便捷的方式来构建 EngineState，使用构建器模式可以更灵活地
    /// 配置和初始化状态。
    ///
    /// # 参数
    ///
    /// - `instruments`: 索引化的交易对集合，用于初始化交易对状态
    /// - `global`: 全局数据，用户自定义的全局状态
    /// - `instrument_data_init`: 初始化函数，为每个交易对创建对应的 `InstrumentData`
    ///
    /// # 返回值
    ///
    /// 返回 `EngineStateBuilder` 实例，可以进一步配置并构建 `EngineState`。
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let builder = EngineState::builder(
    ///     &indexed_instruments,
    ///     global_data,
    ///     |instrument| {
    ///         // 为每个交易对创建自定义数据
    ///         InstrumentData::new(instrument)
    ///     },
    /// );
    ///
    /// let engine_state = builder.build();
    /// ```
    pub fn builder<FnInstrumentData>(
        instruments: &IndexedInstruments,
        global: GlobalData,
        instrument_data_init: FnInstrumentData,
    ) -> EngineStateBuilder<'_, GlobalData, FnInstrumentData>
    where
        FnInstrumentData: Fn(
            &Keyed<InstrumentIndex, Instrument<Keyed<ExchangeIndex, ExchangeId>, AssetIndex>>,
        ) -> InstrumentData,
    {
        EngineStateBuilder::new(instruments, global, instrument_data_init)
    }

    /// 从 `AccountEvent` 更新内部状态。
    ///
    /// 此方法处理账户事件，更新相关的状态（连接状态、资产状态、交易对状态等）。
    /// 如果账户事件导致仓位退出，返回 `PositionExited`。
    ///
    /// ## 处理流程
    ///
    /// 1. **更新连接状态**: 如果账户连接之前处于重连状态，将其设置为健康状态
    /// 2. **处理事件类型**: 根据事件类型更新相应的状态
    /// 3. **更新全局数据**: 使用账户事件更新用户自定义的全局数据
    ///
    /// ## 事件类型处理
    ///
    /// - **Snapshot**: 更新所有资产余额和交易对状态
    /// - **BalanceSnapshot**: 更新单个资产余额
    /// - **OrderSnapshot**: 更新订单状态
    /// - **OrderCancelled**: 更新取消响应状态
    /// - **Trade**: 更新交易状态，可能返回 `PositionExited`
    ///
    /// # 参数
    ///
    /// - `event`: 账户事件
    ///
    /// # 返回值
    ///
    /// - `Some(PositionExited)`: 如果事件导致仓位退出
    /// - `None`: 如果事件没有导致仓位退出
    ///
    /// # 类型约束
    ///
    /// - `GlobalData`: 必须实现 `Processor<&AccountEvent>`
    /// - `InstrumentData`: 必须实现 `Processor<&AccountEvent>`
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// // 处理账户事件
    /// if let Some(position_exited) = engine_state.update_from_account(&account_event) {
    ///     // 处理仓位退出
    ///     println!("Position exited: {:?}", position_exited);
    /// }
    /// ```
    pub fn update_from_account(
        &mut self,
        event: &AccountEvent,
    ) -> Option<PositionExited<QuoteAsset>>
    where
        GlobalData: for<'a> Processor<&'a AccountEvent>,
        InstrumentData: for<'a> Processor<&'a AccountEvent>,
    {
        // 如果账户连接之前处于重连状态，将其设置为健康状态
        self.connectivity.update_from_account_event(&event.exchange);

        // 根据事件类型更新相应的状态
        let output = match &event.kind {
            AccountEventKind::Snapshot(snapshot) => {
                // 更新所有资产余额
                for balance in &snapshot.balances {
                    self.assets
                        .asset_index_mut(&balance.asset)
                        .update_from_balance(Snapshot(balance))
                }
                // 更新所有交易对状态
                for instrument in &snapshot.instruments {
                    let instrument_state = self
                        .instruments
                        .instrument_index_mut(&instrument.instrument);

                    instrument_state.update_from_account_snapshot(instrument);
                    instrument_state.data.process(event);
                }
                None
            }
            AccountEventKind::BalanceSnapshot(balance) => {
                // 更新单个资产余额
                self.assets
                    .asset_index_mut(&balance.0.asset)
                    .update_from_balance(balance.as_ref());
                None
            }
            AccountEventKind::OrderSnapshot(order) => {
                // 更新订单状态
                let instrument_state = self
                    .instruments
                    .instrument_index_mut(&order.value().key.instrument);

                instrument_state.update_from_order_snapshot(order.as_ref());
                instrument_state.data.process(event);
                None
            }
            AccountEventKind::OrderCancelled(response) => {
                // 更新取消响应状态
                let instrument_state = self
                    .instruments
                    .instrument_index_mut(&response.key.instrument);

                instrument_state.update_from_cancel_response(response);
                instrument_state.data.process(event);
                None
            }
            AccountEventKind::Trade(trade) => {
                // 更新交易状态，可能返回仓位退出
                let instrument_state = self.instruments.instrument_index_mut(&trade.instrument);

                instrument_state.data.process(event);
                instrument_state.update_from_trade(trade)
            }
        };

        // 更新用户自定义的全局数据状态
        self.global.process(event);

        output
    }

    /// 从 `MarketEvent` 更新内部状态。
    ///
    /// 此方法处理市场事件，更新连接状态、全局数据和交易对数据。
    ///
    /// ## 处理流程
    ///
    /// 1. **更新连接状态**: 如果市场数据连接之前处于重连状态，将其设置为健康状态
    /// 2. **更新全局数据**: 使用市场事件更新用户自定义的全局数据
    /// 3. **更新交易对数据**: 使用市场事件更新对应交易对的数据
    ///
    /// ## 与账户事件的区别
    ///
    /// - 市场事件只更新交易对数据，不更新资产余额或订单状态
    /// - 市场事件不会导致仓位退出
    /// - 市场事件主要用于更新价格、成交量等市场数据
    ///
    /// # 参数
    ///
    /// - `event`: 市场事件
    ///
    /// # 类型约束
    ///
    /// - `GlobalData`: 必须实现 `Processor<&MarketEvent>`
    /// - `InstrumentData`: 必须实现 `InstrumentDataState`
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// // 处理市场事件
    /// engine_state.update_from_market(&market_event);
    ///
    /// // 访问更新后的交易对数据
    /// let instrument_state = engine_state.instruments.instrument_index(&instrument_index);
    /// let market_data = &instrument_state.data;
    /// ```
    pub fn update_from_market(
        &mut self,
        event: &MarketEvent<InstrumentIndex, InstrumentData::MarketEventKind>,
    ) where
        GlobalData:
            for<'a> Processor<&'a MarketEvent<InstrumentIndex, InstrumentData::MarketEventKind>>,
        InstrumentData: InstrumentDataState,
    {
        // 如果市场数据连接之前处于重连状态，将其设置为健康状态
        self.connectivity.update_from_market_event(&event.exchange);

        // 获取对应的交易对状态
        let instrument_state = self.instruments.instrument_index_mut(&event.instrument);

        // 更新全局数据和交易对数据
        self.global.process(event);
        instrument_state.data.process(event);
    }
}

impl<GlobalData, InstrumentData> From<&EngineState<GlobalData, InstrumentData>>
    for FnvHashMap<ExchangeId, UnindexedAccountSnapshot>
{
    /// 从 `EngineState` 生成未索引的账户快照映射。
    ///
    /// 此实现将 EngineState 转换为按交易所分组的账户快照映射。这对于生成账户快照、
    /// 持久化状态或与外部系统交互非常有用。
    ///
    /// ## 转换过程
    ///
    /// 1. 遍历所有交易所
    /// 2. 为每个交易所收集资产余额（通过 `AssetFilter` 过滤）
    /// 3. 为每个交易所收集交易对快照（通过 `InstrumentFilter` 过滤）
    /// 4. 生成 `UnindexedAccountSnapshot` 并插入映射
    ///
    /// ## 数据过滤
    ///
    /// - **资产余额**: 使用 `AssetFilter::Exchanges` 过滤特定交易所的资产
    /// - **交易对快照**: 使用 `InstrumentFilter::Exchanges` 过滤特定交易所的交易对
    ///
    /// # 参数
    ///
    /// - `value`: EngineState 引用
    ///
    /// # 返回值
    ///
    /// 返回按交易所 ID 索引的账户快照映射。
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// // 生成账户快照映射
    /// let snapshots: FnvHashMap<ExchangeId, UnindexedAccountSnapshot> =
    ///     (&engine_state).into();
    ///
    /// // 访问特定交易所的快照
    /// if let Some(snapshot) = snapshots.get(&exchange_id) {
    ///     println!("Exchange {} balances: {:?}", exchange_id, snapshot.balances);
    /// }
    /// ```
    fn from(value: &EngineState<GlobalData, InstrumentData>) -> Self {
        let EngineState {
            trading: _,
            global: _,
            connectivity,
            assets,
            instruments,
        } = value;

        // 根据交易所数量预分配容量
        let mut snapshots =
            FnvHashMap::with_capacity_and_hasher(connectivity.exchanges.len(), Default::default());

        // 为每个交易所插入未索引的账户快照
        for (index, exchange) in connectivity.exchange_ids().enumerate() {
            snapshots.insert(
                *exchange,
                UnindexedAccountSnapshot {
                    exchange: *exchange,
                    // 收集该交易所的资产余额
                    balances: assets
                        .filtered(&AssetFilter::Exchanges(OneOrMany::One(*exchange)))
                        .map(AssetBalance::from)
                        .collect(),
                    // 收集该交易所的交易对快照
                    instruments: instruments
                        .instruments(&InstrumentFilter::Exchanges(OneOrMany::One(ExchangeIndex(
                            index,
                        ))))
                        .map(|snapshot| {
                            generate_unindexed_instrument_account_snapshot(*exchange, snapshot)
                        })
                        .collect::<Vec<_>>(),
                },
            );
        }

        snapshots
    }
}
