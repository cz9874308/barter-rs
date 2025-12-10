//! Engine 连接状态模块
//!
//! 本模块定义了 Engine 的连接健康状态，包括全局连接状态和每个交易所的市场数据
//! 和账户连接状态。
//!
//! # 核心概念
//!
//! - **ConnectivityStates**: 连接状态集合，维护全局和每个交易所的连接状态
//! - **Health**: 连接健康状态（Healthy 或 Reconnecting）
//! - **ConnectivityState**: 单个交易所的连接状态（市场数据和账户连接）
//!
//! # 使用场景
//!
//! - 监控连接健康状态
//! - 检测连接断开和重连
//! - 根据连接状态调整交易行为

use barter_instrument::{
    exchange::{ExchangeId, ExchangeIndex},
    index::IndexedInstruments,
};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// 维护全局连接 [`Health`]，以及每个交易所的市场数据和账户连接状态。
///
/// ConnectivityStates 跟踪所有交易所的连接健康状态。全局健康状态只有在所有交易所的
/// 市场数据和账户连接都健康时才被认为是 `Healthy`。
///
/// ## 连接状态管理
///
/// - **全局状态**: 所有交易所连接都健康时，全局状态为 `Healthy`
/// - **交易所状态**: 每个交易所独立跟踪市场数据和账户连接状态
/// - **自动更新**: 根据事件自动更新连接状态
///
/// # 使用示例
///
/// ```rust,ignore
/// let mut connectivity = ConnectivityStates::default();
///
/// // 更新账户连接状态
/// connectivity.update_from_account_event(&exchange_index);
///
/// // 检查全局连接状态
/// if connectivity.global == Health::Healthy {
///     // 所有连接都健康
/// }
/// ```
#[derive(Debug, Clone, Eq, PartialEq, Default, Deserialize, Serialize)]
pub struct ConnectivityStates {
    /// 全局连接 [`Health`]。
    ///
    /// 只有当所有交易所的市场数据和账户连接都是 `Healthy` 时，全局健康状态才被认为是 `Healthy`。
    pub global: Health,

    /// 按交易所索引的市场数据和账户连接的连接 `Health`。
    pub exchanges: IndexMap<ExchangeId, ConnectivityState>,
}

impl ConnectivityStates {
    /// 从交易所账户流断开连接事件更新状态。
    ///
    /// 当账户流断开时，将此交易所的账户 `ConnectivityState` 设置为 [`Health::Reconnecting`]，
    /// 并将全局状态也设置为 `Reconnecting`。
    ///
    /// # 参数
    ///
    /// - `exchange`: 交易所 ID
    ///
    /// # 使用场景
    ///
    /// - 账户流断开时调用
    /// - 网络连接中断时调用
    pub fn update_from_account_reconnecting(&mut self, exchange: &ExchangeId) {
        warn!(%exchange, "EngineState received AccountStream disconnected event");
        self.global = Health::Reconnecting;
        self.connectivity_mut(exchange).account = Health::Reconnecting;
    }

    /// 从交易所账户流事件更新状态，如果之前不是健康状态，则将 `ConnectivityState` 的账户
    /// 连接设置为 [`Health::Healthy`]。
    ///
    /// 如果更新后所有 `ConnectivityState` 都是健康的，则将全局健康状态设置为 `Health::Healthy`。
    ///
    /// ## 工作原理
    ///
    /// 1. 如果全局状态已经是 `Healthy`，直接返回（优化）
    /// 2. 检查该交易所的账户连接状态
    /// 3. 如果账户连接不是 `Healthy`，设置为 `Healthy`
    /// 4. 检查所有交易所的连接状态，如果都健康，设置全局状态为 `Healthy`
    ///
    /// # 参数
    ///
    /// - `exchange`: 交易所索引
    ///
    /// # 使用场景
    ///
    /// - 账户流事件到达时调用
    /// - 账户连接恢复时调用
    pub fn update_from_account_event(&mut self, exchange: &ExchangeIndex) {
        if self.global == Health::Healthy {
            return;
        }

        let state = self.connectivity_index_mut(exchange);
        if state.account == Health::Healthy {
            return;
        }

        info!(
            %exchange,
            "EngineState received AccountStream event - setting connection to Healthy"
        );
        state.account = Health::Healthy;

        if self.exchange_states().all(ConnectivityState::all_healthy) {
            info!("EngineState setting global connectivity to Healthy");
            self.global = Health::Healthy
        }
    }

    /// 从交易所市场流断开连接事件更新状态。
    ///
    /// 当市场流断开时，将此交易所的市场数据 `ConnectivityState` 设置为 [`Health::Reconnecting`]，
    /// 并将全局状态也设置为 `Reconnecting`。
    ///
    /// # 参数
    ///
    /// - `exchange`: 交易所 ID
    ///
    /// # 使用场景
    ///
    /// - 市场流断开时调用
    /// - 网络连接中断时调用
    pub fn update_from_market_reconnecting(&mut self, exchange: &ExchangeId) {
        warn!(%exchange, "EngineState received MarketStream disconnect event");
        self.global = Health::Reconnecting;
        self.connectivity_mut(exchange).market_data = Health::Reconnecting
    }

    /// 从交易所市场流事件更新状态，如果之前不是健康状态，则将 `ConnectivityState` 的市场数据
    /// 连接设置为 [`Health::Healthy`]。
    ///
    /// 如果更新后所有 `ConnectivityState` 都是健康的，则将全局健康状态设置为 `Health::Healthy`。
    ///
    /// ## 工作原理
    ///
    /// 1. 如果全局状态已经是 `Healthy`，直接返回（优化）
    /// 2. 检查该交易所的市场数据连接状态
    /// 3. 如果市场数据连接不是 `Healthy`，设置为 `Healthy`
    /// 4. 检查所有交易所的连接状态，如果都健康，设置全局状态为 `Healthy`
    ///
    /// # 参数
    ///
    /// - `exchange`: 交易所 ID
    ///
    /// # 使用场景
    ///
    /// - 市场流事件到达时调用
    /// - 市场数据连接恢复时调用
    pub fn update_from_market_event(&mut self, exchange: &ExchangeId) {
        if self.global == Health::Healthy {
            return;
        }

        let state = self.connectivity_mut(exchange);
        if state.market_data == Health::Healthy {
            return;
        }

        info!(
            %exchange,
            "EngineState received MarketStream event - setting connection to Healthy"
        );
        state.market_data = Health::Healthy;

        if self.exchange_states().all(ConnectivityState::all_healthy) {
            info!("EngineState setting global connectivity to Healthy");
            self.global = Health::Healthy
        }
    }

    /// Returns a reference to the `ConnectivityState` associated with the
    /// provided `ExchangeIndex`.
    ///
    /// Panics if the `ConnectivityState` associated with the `ExchangeIndex` is not found.
    pub fn connectivity_index(&self, key: &ExchangeIndex) -> &ConnectivityState {
        self.exchanges
            .get_index(key.index())
            .map(|(_key, state)| state)
            .unwrap_or_else(|| panic!("ConnectivityStates does not contain: {key}"))
    }

    /// Returns a mutable reference to the `ConnectivityState` associated with the
    /// provided `ExchangeIndex`.
    ///
    /// Panics if the `ConnectivityState` associated with the `ExchangeIndex` is not found.
    pub fn connectivity_index_mut(&mut self, key: &ExchangeIndex) -> &mut ConnectivityState {
        self.exchanges
            .get_index_mut(key.index())
            .map(|(_key, state)| state)
            .unwrap_or_else(|| panic!("ConnectivityStates does not contain: {key}"))
    }

    /// Returns a reference to the `ConnectivityState` associated with the
    /// provided `ExchangeId`.
    ///
    /// Panics if the `ConnectivityState` associated with the `ExchangeId` is not found.
    pub fn connectivity(&self, key: &ExchangeId) -> &ConnectivityState {
        self.exchanges
            .get(key)
            .unwrap_or_else(|| panic!("ConnectivityStates does not contain: {key}"))
    }

    /// Returns a mutable reference to the `ConnectivityState` associated with the
    /// provided `ExchangeId`.
    ///
    /// Panics if the `ConnectivityState` associated with the `ExchangeId` is not found.
    pub fn connectivity_mut(&mut self, key: &ExchangeId) -> &mut ConnectivityState {
        self.exchanges
            .get_mut(key)
            .unwrap_or_else(|| panic!("ConnectivityStates does not contain: {key}"))
    }

    /// Return an `Iterator` of the `ExchangeId`s being tracked.
    pub fn exchange_ids(&self) -> impl Iterator<Item = &ExchangeId> {
        self.exchanges.keys()
    }

    /// Return an `Iterator` of all `ConnectivityState`s being tracked.
    pub fn exchange_states(&self) -> impl Iterator<Item = &ConnectivityState> {
        self.exchanges.values()
    }
}

/// 表示组件或连接到交易所端点的 `Health`（健康）状态。
///
/// Health 用于在 [`ConnectivityState`] 中跟踪市场数据和账户连接。
///
/// ## 默认值
///
/// 默认实现是 [`Health::Reconnecting`]，表示初始状态为正在重连。
///
/// ## 状态转换
///
/// - **Reconnecting → Healthy**: 当连接建立并正常工作时
/// - **Healthy → Reconnecting**: 当连接断开或失败时
///
/// # 使用示例
///
/// ```rust,ignore
/// let health = Health::Reconnecting;
/// if health == Health::Healthy {
///     // 连接正常
/// }
/// ```
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub enum Health {
    /// 连接已建立并正常工作。
    Healthy,

    /// 连接在断开或失败后正在尝试重新建立。
    Reconnecting,
}

/// 表示交易所的市场数据和账户连接的当前连接状态。
///
/// ConnectivityState 分别监控市场数据和账户连接的健康状态，因为它们通常使用不同的端点，
/// 可能具有不同的健康状态。
///
/// ## 为什么分别监控？
///
/// - **不同端点**: 市场数据和账户连接通常使用不同的 API 端点
/// - **独立故障**: 一个连接可能失败而另一个仍然正常
/// - **灵活控制**: 可以根据不同连接状态采取不同的策略
///
/// # 使用示例
///
/// ```rust,ignore
/// let state = ConnectivityState {
///     market_data: Health::Healthy,
///     account: Health::Reconnecting,
/// };
///
/// if state.all_healthy() {
///     // 所有连接都健康
/// }
/// ```
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default, Deserialize, Serialize)]
pub struct ConnectivityState {
    /// 市场数据连接的状态。
    pub market_data: Health,

    /// 账户和执行连接的状态。
    pub account: Health,
}

impl ConnectivityState {
    /// 如果市场数据和账户连接都是 [`Health::Healthy`]，返回 `true`。
    ///
    /// 此方法用于检查交易所的所有连接是否都健康。
    ///
    /// # 返回值
    ///
    /// - `true`: 如果所有连接都健康
    /// - `false`: 如果至少有一个连接不是健康状态
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// if connectivity_state.all_healthy() {
    ///     // 可以安全地进行交易
    /// }
    /// ```
    pub fn all_healthy(&self) -> bool {
        self.market_data == Health::Healthy && self.account == Health::Healthy
    }
}

impl Default for Health {
    fn default() -> Self {
        Self::Reconnecting
    }
}

/// 生成包含默认连接状态的索引化 [`ConnectivityStates`]。
///
/// 为提供的交易对集合中的每个交易所创建新的连接状态跟踪器，所有连接初始状态都设置为
/// [`Health::Reconnecting`]。
///
/// ## 使用场景
///
/// - 初始化 EngineState 时调用
/// - 创建新的连接状态跟踪器
///
/// # 参数
///
/// - `instruments`: 对 [`IndexedInstruments`] 的引用，包含要跟踪的交易所
///
/// # 返回值
///
/// 返回新创建的 `ConnectivityStates`，所有连接状态初始化为 `Reconnecting`。
///
/// # 使用示例
///
/// ```rust,ignore
/// let connectivity = generate_empty_indexed_connectivity_states(&indexed_instruments);
/// ```
pub fn generate_empty_indexed_connectivity_states(
    instruments: &IndexedInstruments,
) -> ConnectivityStates {
    ConnectivityStates {
        global: Health::Reconnecting,
        exchanges: instruments
            .exchanges()
            .iter()
            .map(|exchange| (exchange.value, ConnectivityState::default()))
            .collect(),
    }
}
