//! Engine 资产状态模块
//!
//! 本模块定义了资产状态管理，用于跟踪每个交易所的资产余额和统计信息。
//!
//! # 核心概念
//!
//! - **AssetStates**: 资产状态集合，维护所有交易所资产的状态
//! - **AssetState**: 单个资产的状态，包括余额和统计信息
//! - **AssetFilter**: 资产过滤器，用于筛选资产数据
//!
//! # 使用场景
//!
//! - 跟踪资产余额
//! - 更新资产状态
//! - 生成资产统计摘要

use crate::{
    Timed, engine::state::asset::filter::AssetFilter,
    statistic::summary::asset::TearSheetAssetGenerator,
};
use barter_execution::balance::{AssetBalance, Balance};
use barter_instrument::{
    asset::{
        Asset, AssetIndex, ExchangeAsset,
        name::{AssetNameExchange, AssetNameInternal},
    },
    index::IndexedInstruments,
};
use barter_integration::{collection::FnvIndexMap, snapshot::Snapshot};
use chrono::Utc;
use derive_more::Constructor;
use itertools::Either;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// 定义 `AssetFilter`，用于过滤以资产为中心的数据结构。
pub mod filter;

/// 按 [`AssetIndex`] 索引的交易所 [`AssetState`] 集合。
///
/// AssetStates 维护所有交易所资产的状态映射。注意，不同交易所上的同名资产会有
/// 各自的 [`AssetState`]（例如，Binance 的 BTC 和 Coinbase 的 BTC 是分开跟踪的）。
///
/// ## 索引方式
///
/// 使用 `ExchangeAsset<AssetNameInternal>` 作为键，这样可以区分不同交易所的同名资产。
///
/// ## 使用场景
///
/// - 跟踪所有交易所的资产余额
/// - 更新资产状态
/// - 生成资产统计摘要
///
/// # 使用示例
///
/// ```rust,ignore
/// let asset_states = AssetStates::default();
///
/// // 获取特定资产的状态
/// let btc_state = asset_states.asset_index(&btc_index);
///
/// // 更新资产余额
/// asset_states.asset_index_mut(&btc_index).update_from_balance(&balance_snapshot);
///
/// // 过滤资产
/// for asset_state in asset_states.filtered(&AssetFilter::Exchanges(...)) {
///     // 处理筛选的资产
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
pub struct AssetStates(
    /// 以 ExchangeAsset 为键的资产状态映射
    pub FnvIndexMap<ExchangeAsset<AssetNameInternal>, AssetState>,
);

impl AssetStates {
    /// 返回与 `AssetIndex` 关联的 `AssetState` 的引用。
    ///
    /// 如果与 `AssetIndex` 关联的 `AssetState` 不存在，则 panic。
    ///
    /// # 参数
    ///
    /// - `key`: 资产索引
    ///
    /// # 返回值
    ///
    /// 返回资产状态的不可变引用。
    pub fn asset_index(&self, key: &AssetIndex) -> &AssetState {
        self.0
            .get_index(key.index())
            .map(|(_key, state)| state)
            .unwrap_or_else(|| panic!("AssetStates does not contain: {key}"))
    }

    /// 返回与 `AssetIndex` 关联的 `AssetState` 的可变引用。
    ///
    /// 如果与 `AssetIndex` 关联的 `AssetState` 不存在，则 panic。
    ///
    /// # 参数
    ///
    /// - `key`: 资产索引
    ///
    /// # 返回值
    ///
    /// 返回资产状态的可变引用。
    pub fn asset_index_mut(&mut self, key: &AssetIndex) -> &mut AssetState {
        self.0
            .get_index_mut(key.index())
            .map(|(_key, state)| state)
            .unwrap_or_else(|| panic!("AssetStates does not contain: {key}"))
    }

    /// Return a reference to the `AssetState` associated with an `ExchangeAsset<AssetNameInternal>`.
    ///
    /// Panics if the `AssetState` associated with the `ExchangeAsset<AssetNameInternal>`
    /// does not exist.
    pub fn asset(&self, key: &ExchangeAsset<AssetNameInternal>) -> &AssetState {
        self.0
            .get(key)
            .unwrap_or_else(|| panic!("AssetStates does not contain: {key:?}"))
    }

    /// Return a mutable reference to the `AssetState` associated with an
    /// `ExchangeAsset<AssetNameInternal>`.
    ///
    /// Panics if the `AssetState` associated with the `ExchangeAsset<AssetNameInternal>`
    /// does not exist.
    pub fn asset_mut(&mut self, key: &ExchangeAsset<AssetNameInternal>) -> &mut AssetState {
        self.0
            .get_mut(key)
            .unwrap_or_else(|| panic!("AssetStates does not contain: {key:?}"))
    }

    /// 基于提供的 [`AssetFilter`] 返回过滤后的 `AssetState` 迭代器。
    ///
    /// 此方法根据过滤器筛选资产状态，支持按交易所过滤。
    ///
    /// # 参数
    ///
    /// - `filter`: 资产过滤器
    ///
    /// # 返回值
    ///
    /// 返回过滤后的资产状态迭代器。
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// // 获取所有资产状态
    /// for state in asset_states.filtered(&AssetFilter::None) {
    ///     // 处理所有资产
    /// }
    ///
    /// // 获取特定交易所的资产状态
    /// for state in asset_states.filtered(&AssetFilter::Exchanges(...)) {
    ///     // 处理筛选的资产
    /// }
    /// ```
    pub fn filtered<'a>(&'a self, filter: &'a AssetFilter) -> impl Iterator<Item = &'a AssetState> {
        use filter::AssetFilter::*;
        match filter {
            None => Either::Left(self.assets()),
            Exchanges(exchanges) => Either::Right(self.0.iter().filter_map(|(asset, state)| {
                if exchanges.contains(&asset.exchange) {
                    Some(state)
                } else {
                    Option::<&AssetState>::None
                }
            })),
        }
    }

    /// Returns an `Iterator` of all `AssetState`s being tracked.
    pub fn assets(&self) -> impl Iterator<Item = &AssetState> {
        self.0.values()
    }
}

/// 表示资产的当前状态，包括其 [`Balance`] 和最后更新的 `time_exchange`。
///
/// AssetState 维护单个资产的状态信息。在 [`AssetStates`] 的上下文中使用时，此状态用于
/// 交易所资产，但也可以用于"全局"资产，聚合多个交易所上同名资产的数据。
///
/// ## 状态组成
///
/// - **asset**: 资产名称数据（内部名称和交易所名称）
/// - **statistics**: 交易会话变更摘要生成器
/// - **balance**: 当前余额和关联的交易所时间戳（可选）
///
/// ## 使用场景
///
/// - 跟踪资产余额
/// - 生成资产统计摘要
/// - 更新资产状态
///
/// # 使用示例
///
/// ```rust,ignore
/// let mut asset_state = AssetState::new(
///     asset,
///     TearSheetAssetGenerator::default(),
///     None,
/// );
///
/// // 从余额快照更新
/// asset_state.update_from_balance(&balance_snapshot);
///
/// // 访问余额
/// if let Some(balance) = &asset_state.balance {
///     println!("Balance: {:?}", balance.value);
/// }
/// ```
#[derive(Debug, Clone, PartialEq, PartialOrd, Deserialize, Serialize, Constructor)]
pub struct AssetState {
    /// 资产名称数据，包含内部名称和交易所名称。
    pub asset: Asset,

    /// 交易会话变更摘要生成器，用于汇总资产在交易会话中的变化。
    pub statistics: TearSheetAssetGenerator,

    /// 资产的当前余额和关联的交易所时间戳（如果存在）。
    pub balance: Option<Timed<Balance>>,
}

impl AssetState {
    /// 从 [`AssetBalance`] 快照更新 `AssetState`，如果快照更新则应用更新。
    ///
    /// 此方法通过仅应用至少与当前状态一样新的快照更新来确保时间一致性。
    /// 这防止了旧数据覆盖新数据的问题。
    ///
    /// ## 工作原理
    ///
    /// 1. 如果当前没有余额，直接设置新余额
    /// 2. 如果快照时间戳 >= 当前余额时间戳，更新余额
    /// 3. 如果快照时间戳 < 当前余额时间戳，忽略更新（防止数据回退）
    ///
    /// # 参数
    ///
    /// - `snapshot`: 资产余额快照
    ///
    /// # 类型参数
    ///
    /// - `AssetKey`: 资产键类型
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let snapshot = Snapshot(AssetBalance {
    ///     asset: asset.clone(),
    ///     balance: Balance { total: 1000.0, free: 900.0 },
    ///     time_exchange: Utc::now(),
    /// });
    ///
    /// asset_state.update_from_balance(snapshot.as_ref());
    /// ```
    pub fn update_from_balance<AssetKey>(&mut self, snapshot: Snapshot<&AssetBalance<AssetKey>>) {
        let Some(balance) = &mut self.balance else {
            self.balance = Some(Timed::new(snapshot.0.balance, snapshot.0.time_exchange));
            self.statistics.update_from_balance(snapshot);
            return;
        };

        if balance.time <= snapshot.value().time_exchange {
            balance.time = snapshot.value().time_exchange;
            balance.value = snapshot.value().balance;
            self.statistics.update_from_balance(snapshot);
        }
    }
}

impl From<&AssetState> for AssetBalance<AssetNameExchange> {
    fn from(value: &AssetState) -> Self {
        let AssetState {
            asset,
            statistics: _,
            balance,
        } = value;

        let (balance, time_exchange) = match balance {
            None => (Balance::default(), Utc::now()),
            Some(balance) => (balance.value, balance.time),
        };

        Self {
            asset: asset.name_exchange.clone(),
            balance,
            time_exchange,
        }
    }
}

/// 生成包含默认资产余额数据的索引化 [`AssetStates`]。
///
/// 此函数为提供的交易对集合中的所有资产创建空的资产状态。所有余额初始化为 `None`，
/// 时间戳设置为 `DateTime::<Utc>::MIN_UTC`。
///
/// ## 使用场景
///
/// - 初始化 EngineState 时调用
/// - 创建新的资产状态集合
///
/// # 参数
///
/// - `instruments`: 对 [`IndexedInstruments`] 的引用，包含要跟踪的资产
///
/// # 返回值
///
/// 返回新创建的 `AssetStates`，所有资产状态初始化为空。
///
/// # 注意事项
///
/// 注意 `time_exchange` 设置为 `DateTime::<Utc>::MIN_UTC`。
///
/// # 使用示例
///
/// ```rust,ignore
/// let asset_states = generate_empty_indexed_asset_states(&indexed_instruments);
/// ```
pub fn generate_empty_indexed_asset_states(instruments: &IndexedInstruments) -> AssetStates {
    AssetStates(
        instruments
            .assets()
            .iter()
            .map(|asset| {
                (
                    ExchangeAsset::new(
                        asset.value.exchange,
                        asset.value.asset.name_internal.clone(),
                    ),
                    AssetState::new(
                        asset.value.asset.clone(),
                        TearSheetAssetGenerator::default(),
                        None,
                    ),
                )
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::asset_state;
    use barter_instrument::asset::name::AssetNameExchange;
    use chrono::{DateTime, TimeZone, Utc};
    use rust_decimal_macros::dec;

    #[test]
    fn test_update_from_balance_with_first_ever_snapshot() {
        let mut state = AssetState {
            asset: Asset {
                name_internal: AssetNameInternal::new("btc"),
                name_exchange: AssetNameExchange::new("btc"),
            },
            statistics: Default::default(),
            balance: None,
        };

        let snapshot = Snapshot(AssetBalance {
            asset: Asset {
                name_internal: AssetNameInternal::new("btc"),
                name_exchange: AssetNameExchange::new("btc"),
            },
            balance: Balance {
                total: dec!(1100.0),
                free: dec!(1100.0),
            },
            time_exchange: DateTime::<Utc>::MIN_UTC,
        });

        state.update_from_balance(snapshot.as_ref());

        let expected = asset_state("btc", 1100.0, 1100.0, DateTime::<Utc>::MIN_UTC);

        assert_eq!(state, expected)
    }

    #[test]
    fn test_update_from_balance_with_more_recent_snapshot() {
        let mut state = asset_state("btc", 1000.0, 1000.0, DateTime::<Utc>::MIN_UTC);

        let snapshot = Snapshot(AssetBalance {
            asset: Asset {
                name_internal: AssetNameInternal::new("btc"),
                name_exchange: AssetNameExchange::new("xbt"),
            },
            balance: Balance {
                total: dec!(1100.0),
                free: dec!(1100.0),
            },
            time_exchange: DateTime::<Utc>::MAX_UTC,
        });

        state.update_from_balance(snapshot.as_ref());

        let expected = asset_state("btc", 1100.0, 1100.0, DateTime::<Utc>::MAX_UTC);

        assert_eq!(state, expected)
    }

    #[test]
    fn test_update_from_balance_with_equal_timestamp() {
        // Test case: Verify state updates when snapshot has equal timestamp
        let time = Utc.timestamp_opt(1000, 0).unwrap();

        let mut state = asset_state("btc", 1000.0, 900.0, time);

        let snapshot = Snapshot(AssetBalance {
            asset: Asset {
                name_internal: AssetNameInternal::new("btc"),
                name_exchange: AssetNameExchange::new("xbt"),
            },
            balance: Balance {
                total: dec!(1000.0),
                free: dec!(800.0),
            },
            time_exchange: time,
        });

        state.update_from_balance(snapshot.as_ref());

        assert_eq!(state.balance.unwrap().value.total, dec!(1000.0));
        assert_eq!(state.balance.unwrap().value.free, dec!(800.0));
        assert_eq!(state.balance.unwrap().time, time);
    }

    #[test]
    fn test_update_from_balance_with_stale_snapshot() {
        let mut state = asset_state("btc", 1000.0, 900.0, DateTime::<Utc>::MAX_UTC);

        let snapshot = Snapshot(AssetBalance {
            asset: Asset {
                name_internal: AssetNameInternal::new("btc"),
                name_exchange: AssetNameExchange::new("xbt"),
            },
            balance: Balance {
                total: dec!(1000.0),
                free: dec!(800.0),
            },
            time_exchange: DateTime::<Utc>::MIN_UTC,
        });

        state.update_from_balance(snapshot.as_ref());

        let expected = asset_state("btc", 1000.0, 900.0, DateTime::<Utc>::MAX_UTC);

        assert_eq!(state, expected)
    }
}
