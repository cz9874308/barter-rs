//! EngineState 构建器模块
//!
//! 本模块提供了 `EngineStateBuilder`，用于便捷地构建和初始化 `EngineState`。
//! 构建器模式允许灵活地配置 EngineState 的各个组件，支持可选参数和默认值。
//!
//! # 核心概念
//!
//! - **构建器模式**: 使用链式调用逐步配置 EngineState
//! - **可选参数**: 支持可选配置，未提供时使用默认值
//! - **默认值**: 为未提供的参数提供合理的默认值
//!
//! # 使用场景
//!
//! - 初始化 EngineState
//! - 回测场景中设置初始余额和时间
//! - 自定义 EngineState 配置

use crate::engine::state::{
    EngineState, asset::generate_empty_indexed_asset_states,
    connectivity::generate_empty_indexed_connectivity_states,
    instrument::generate_indexed_instrument_states, order::Orders, position::PositionManager,
    trading::TradingState,
};
use barter_execution::balance::{AssetBalance, Balance};
use barter_instrument::{
    Keyed,
    asset::{AssetIndex, ExchangeAsset, name::AssetNameInternal},
    exchange::{ExchangeId, ExchangeIndex},
    index::IndexedInstruments,
    instrument::{Instrument, InstrumentIndex},
};
use barter_integration::snapshot::Snapshot;
use chrono::{DateTime, Utc};
use fnv::FnvHashMap;
use tracing::debug;

/// [`EngineState`] 实例的构建器工具。
///
/// EngineStateBuilder 使用构建器模式，允许通过链式调用逐步配置 EngineState 的各个组件。
/// 支持可选参数，未提供的参数将使用默认值。
///
/// ## 构建器模式的优势
///
/// - **灵活性**: 可以只配置需要的参数，其他使用默认值
/// - **可读性**: 链式调用使代码更清晰易读
/// - **类型安全**: 编译时检查配置的正确性
///
/// ## 可选配置
///
/// - `trading_state`: 初始交易状态（默认：`TradingState::Disabled`）
/// - `time_engine_start`: Engine 启动时间（默认：`Utc::now()`）
/// - `balances`: 初始资产余额（默认：空余额）
///
/// ## 必需配置
///
/// - `instruments`: 索引化的交易对集合
/// - `global`: 全局数据
/// - `instrument_data_init`: 交易对数据初始化函数
///
/// # 类型参数
///
/// - `'a`: 索引化交易对集合的生命周期
/// - `GlobalData`: 全局数据类型
/// - `FnInstrumentData`: 交易对数据初始化函数类型
///
/// # 使用示例
///
/// ```rust,ignore
/// // 基本使用（使用默认值）
/// let engine_state = EngineStateBuilder::new(
///     &indexed_instruments,
///     global_data,
///     |instrument| InstrumentData::new(instrument),
/// )
/// .build();
///
/// // 完整配置
/// let engine_state = EngineStateBuilder::new(
///     &indexed_instruments,
///     global_data,
///     |instrument| InstrumentData::new(instrument),
/// )
/// .trading_state(TradingState::Enabled)
/// .time_engine_start(historical_time)
/// .balances(initial_balances)
/// .build();
/// ```
#[derive(Debug, Clone)]
pub struct EngineStateBuilder<'a, GlobalData, FnInstrumentData> {
    /// 索引化的交易对集合，用于初始化状态结构
    instruments: &'a IndexedInstruments,
    /// 可选的初始交易状态（默认：`TradingState::Disabled`）
    trading_state: Option<TradingState>,
    /// 可选的 Engine 启动时间（默认：`Utc::now()`）
    time_engine_start: Option<DateTime<Utc>>,
    /// 全局数据
    global: GlobalData,
    /// 初始资产余额映射（交易所资产 -> 余额）
    balances: FnvHashMap<ExchangeAsset<AssetNameInternal>, Balance>,
    /// 交易对数据初始化函数
    instrument_data_init: FnInstrumentData,
}

impl<'a, GlobalData, FnInstrumentData> EngineStateBuilder<'a, GlobalData, FnInstrumentData> {
    /// 使用从 [`IndexedInstruments`] 派生的布局构造新的 `EngineStateBuilder`。
    ///
    /// 此方法创建一个新的构建器实例，使用提供的参数初始化必需字段，其他字段使用默认值。
    ///
    /// ## 默认行为
    ///
    /// - `ConnectivityStates` 将使用 `generate_empty_indexed_connectivity_states` 生成，
    ///   默认所有连接状态为 `Health::Reconnecting`
    /// - `trading_state` 默认为 `None`（构建时使用 `TradingState::Disabled`）
    /// - `time_engine_start` 默认为 `None`（构建时使用 `Utc::now()`）
    /// - `balances` 默认为空映射（构建时使用零余额）
    ///
    /// ## 注意事项
    ///
    /// 如果只需要默认值，可以直接调用 `build()` 而不需要配置其他参数。
    ///
    /// # 参数
    ///
    /// - `instruments`: 索引化的交易对集合，用于确定状态结构的布局
    /// - `global`: 全局数据，用户自定义的全局状态
    /// - `instrument_data_init`: 交易对数据初始化函数，为每个交易对创建对应的数据
    ///
    /// # 返回值
    ///
    /// 返回新创建的 `EngineStateBuilder` 实例。
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let builder = EngineStateBuilder::new(
    ///     &indexed_instruments,
    ///     global_data,
    ///     |instrument| InstrumentData::new(instrument),
    /// );
    /// ```
    pub fn new(
        instruments: &'a IndexedInstruments,
        global: GlobalData,
        instrument_data_init: FnInstrumentData,
    ) -> Self {
        Self {
            instruments,
            time_engine_start: None,
            trading_state: None,
            global,
            balances: FnvHashMap::default(),
            instrument_data_init,
        }
    }

    /// 可选地提供初始 `TradingState`。
    ///
    /// 此方法用于设置 Engine 的初始交易状态。如果未调用此方法，默认使用 `TradingState::Disabled`。
    ///
    /// # 参数
    ///
    /// - `value`: 交易状态（`TradingState::Enabled` 或 `TradingState::Disabled`）
    ///
    /// # 返回值
    ///
    /// 返回更新后的构建器，支持方法链式调用。
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let builder = builder.trading_state(TradingState::Enabled);
    /// ```
    pub fn trading_state(self, value: TradingState) -> Self {
        Self {
            trading_state: Some(value),
            ..self
        }
    }

    /// 可选地提供 `time_engine_start`（Engine 启动时间）。
    ///
    /// 此方法用于设置 Engine 的启动时间。这在回测场景中特别有用，因为需要将时间
    /// 设置为历史时钟时间（例如，从第一个历史 `MarketEvent` 获取）。
    ///
    /// ## 使用场景
    ///
    /// - **回测**: 使用历史数据的第一个事件时间作为启动时间
    /// - **时间同步**: 确保 Engine 时间与历史数据时间一致
    ///
    /// ## 默认值
    ///
    /// 如果未调用此方法，默认使用 `Utc::now()`。
    ///
    /// # 参数
    ///
    /// - `value`: Engine 启动时间（UTC 格式）
    ///
    /// # 返回值
    ///
    /// 返回更新后的构建器，支持方法链式调用。
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// // 回测场景：使用第一个历史事件的时间
    /// let first_event_time = historical_events.first().unwrap().time_exchange;
    /// let builder = builder.time_engine_start(first_event_time);
    /// ```
    pub fn time_engine_start(self, value: DateTime<Utc>) -> Self {
        Self {
            time_engine_start: Some(value),
            ..self
        }
    }

    /// 可选地提供初始交易所资产 `Balance`（余额）。
    ///
    /// 此方法用于设置 EngineState 的初始资产余额。这在回测场景中特别有用，因为需要
    /// 使用初始余额来初始化 EngineState。
    ///
    /// ## 使用场景
    ///
    /// - **回测**: 使用历史账户快照的初始余额
    /// - **模拟交易**: 设置模拟账户的初始余额
    /// - **状态恢复**: 从持久化状态恢复余额
    ///
    /// ## 注意事项
    ///
    /// - 内部实现使用 `HashMap`，因此重复的 `ExchangeAsset<AssetNameInternal>` 键会被覆盖
    /// - 余额按交易所和资产分组（`ExchangeAsset`）
    ///
    /// # 类型参数
    ///
    /// - `BalanceIter`: 余额迭代器类型
    /// - `KeyedBalance`: 键值对类型，必须可以转换为 `Keyed<ExchangeAsset<AssetNameInternal>, Balance>`
    ///
    /// # 参数
    ///
    /// - `balances`: 初始余额集合（可以是任何可迭代的键值对）
    ///
    /// # 返回值
    ///
    /// 返回更新后的构建器，支持方法链式调用。
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// // 从历史快照设置初始余额
    /// let initial_balances = vec![
    ///     Keyed {
    ///         key: ExchangeAsset::new(exchange_id, asset_name),
    ///         value: Balance::new(1000.0),
    ///     },
    /// ];
    /// let builder = builder.balances(initial_balances);
    /// ```
    pub fn balances<BalanceIter, KeyedBalance>(mut self, balances: BalanceIter) -> Self
    where
        BalanceIter: IntoIterator<Item = KeyedBalance>,
        KeyedBalance: Into<Keyed<ExchangeAsset<AssetNameInternal>, Balance>>,
    {
        // 将键值对转换为 (key, value) 元组并插入到 HashMap 中
        self.balances.extend(balances.into_iter().map(|keyed| {
            let Keyed { key, value } = keyed.into();

            (key, value)
        }));
        self
    }

    /// 使用构建器数据生成对应的 [`EngineState`]。
    ///
    /// 此方法使用构建器中配置的所有参数生成最终的 EngineState。如果可选数据未提供
    /// （例如余额），则使用默认值（例如零余额）。
    ///
    /// ## 构建过程
    ///
    /// 1. **设置默认值**: 如果可选参数未提供，使用默认值
    /// 2. **生成连接状态**: 使用 `generate_empty_indexed_connectivity_states` 生成空连接状态
    /// 3. **生成资产状态**: 使用 `generate_empty_indexed_asset_states` 生成空资产状态，然后从提供的余额更新
    /// 4. **生成交易对状态**: 使用 `generate_indexed_instrument_states` 生成交易对状态
    /// 5. **组装 EngineState**: 将所有组件组装成最终的 EngineState
    ///
    /// ## 默认值
    ///
    /// - `time_engine_start`: `Utc::now()`（如果未提供）
    /// - `trading_state`: `TradingState::Disabled`（如果未提供）
    /// - `balances`: 空映射（如果未提供，所有资产余额为零）
    ///
    /// # 类型参数
    ///
    /// - `InstrumentData`: 交易对数据类型
    ///
    /// # 返回值
    ///
    /// 返回完全初始化的 `EngineState` 实例。
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let engine_state = builder
    ///     .trading_state(TradingState::Enabled)
    ///     .time_engine_start(start_time)
    ///     .balances(initial_balances)
    ///     .build();
    /// ```
    pub fn build<InstrumentData>(self) -> EngineState<GlobalData, InstrumentData>
    where
        FnInstrumentData: Fn(
            &'a Keyed<InstrumentIndex, Instrument<Keyed<ExchangeIndex, ExchangeId>, AssetIndex>>,
        ) -> InstrumentData,
    {
        let Self {
            instruments,
            time_engine_start,
            trading_state,
            global,
            balances,
            instrument_data_init,
        } = self;

        // 如果未提供，使用默认值
        let time_engine_start = time_engine_start.unwrap_or_else(|| {
            debug!("EngineStateBuilder using Utc::now as time_engine_start default");
            Utc::now()
        });
        let trading = trading_state.unwrap_or_default();

        // 构造空的连接状态（默认所有连接为 Reconnecting）
        let connectivity = generate_empty_indexed_connectivity_states(instruments);

        // 从提供的交易所资产余额更新空的资产状态
        let mut assets = generate_empty_indexed_asset_states(instruments);
        for (key, balance) in balances {
            assets
                .asset_mut(&key)
                .update_from_balance(Snapshot(&AssetBalance {
                    asset: key.asset,
                    balance,
                    time_exchange: time_engine_start,
                }))
        }

        // 使用提供的 FnInstrumentData 等生成空的交易对状态
        let instruments = generate_indexed_instrument_states(
            instruments,
            time_engine_start,
            PositionManager::default,
            Orders::default,
            instrument_data_init,
        );

        EngineState {
            trading,
            global,
            connectivity,
            assets,
            instruments,
        }
    }
}
