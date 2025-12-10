#![forbid(unsafe_code)]
#![warn(
    unused,
    clippy::cognitive_complexity,
    unused_crate_dependencies,
    unused_extern_crates,
    clippy::unused_self,
    clippy::useless_let_if_seq,
    missing_debug_implementations,
    rust_2018_idioms,
    rust_2024_compatibility
)]
#![allow(clippy::type_complexity, clippy::too_many_arguments, type_alias_bounds)]

//! # Barter-Execution
//! 从金融场所流式传输私有账户数据，并执行（实时或模拟）订单。
//! 还提供了功能丰富的 MockExchange 和 MockExecutionClient 以协助回测和模拟交易。
//!
//! **它的特点是：**
//! * **简单**: ExecutionClient Trait 提供了与交易所交互的统一且简单的语言。
//! * **标准化**: 允许您的策略使用相同的接口与每个真实或模拟交易所通信。
//! * **可扩展**: Barter-Execution 高度可扩展，通过添加新的交易所集成可以轻松贡献！
//!
//! 更多信息和示例请参见 `README.md`。

use crate::{
    balance::AssetBalance,
    order::{Order, OrderSnapshot, request::OrderResponseCancel},
    trade::Trade,
};
use barter_instrument::{
    asset::{AssetIndex, QuoteAsset, name::AssetNameExchange},
    exchange::{ExchangeId, ExchangeIndex},
    instrument::{InstrumentIndex, name::InstrumentNameExchange},
};
use barter_integration::snapshot::Snapshot;
use chrono::{DateTime, Utc};
use derive_more::{Constructor, From};
use order::state::OrderState;
use serde::{Deserialize, Serialize};

pub mod balance;
pub mod client;
pub mod error;
pub mod exchange;
pub mod indexer;
pub mod map;
pub mod order;
pub mod trade;

/// 使用 [`ExchangeId`]、[`AssetNameExchange`] 和 [`InstrumentNameExchange`]
/// 作为键的 [`AccountEvent`] 的便捷类型别名。
pub type UnindexedAccountEvent =
    AccountEvent<ExchangeId, AssetNameExchange, InstrumentNameExchange>;

/// 使用 [`ExchangeId`]、[`AssetNameExchange`] 和 [`InstrumentNameExchange`]
/// 作为键的 [`AccountSnapshot`] 的便捷类型别名。
pub type UnindexedAccountSnapshot =
    AccountSnapshot<ExchangeId, AssetNameExchange, InstrumentNameExchange>;

/// 账户事件，表示账户状态的变更。
///
/// AccountEvent 是账户数据流中的基本事件类型，包含交易所标识和事件类型。
///
/// ## 类型参数
///
/// - `ExchangeKey`: 交易所键类型（默认：`ExchangeIndex`）
/// - `AssetKey`: 资产键类型（默认：`AssetIndex`）
/// - `InstrumentKey`: 交易对键类型（默认：`InstrumentIndex`）
///
/// ## 字段说明
///
/// - **exchange**: 交易所标识
/// - **kind**: 账户事件类型（快照、余额、订单、交易等）
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct AccountEvent<
    ExchangeKey = ExchangeIndex,
    AssetKey = AssetIndex,
    InstrumentKey = InstrumentIndex,
> {
    /// 交易所标识。
    pub exchange: ExchangeKey,
    /// 账户事件类型。
    pub kind: AccountEventKind<ExchangeKey, AssetKey, InstrumentKey>,
}

impl<ExchangeKey, AssetKey, InstrumentKey> AccountEvent<ExchangeKey, AssetKey, InstrumentKey> {
    /// 创建新的账户事件。
    ///
    /// # 参数
    ///
    /// - `exchange`: 交易所标识
    /// - `kind`: 账户事件类型（可转换为 `AccountEventKind`）
    ///
    /// # 返回值
    ///
    /// 返回新创建的 `AccountEvent` 实例。
    pub fn new<K>(exchange: ExchangeKey, kind: K) -> Self
    where
        K: Into<AccountEventKind<ExchangeKey, AssetKey, InstrumentKey>>,
    {
        Self {
            exchange,
            kind: kind.into(),
        }
    }
}

/// 账户事件类型枚举。
///
/// AccountEventKind 定义了账户数据流中可能发生的各种事件类型。
///
/// ## 类型参数
///
/// - `ExchangeKey`: 交易所键类型
/// - `AssetKey`: 资产键类型
/// - `InstrumentKey`: 交易对键类型
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, From)]
pub enum AccountEventKind<ExchangeKey, AssetKey, InstrumentKey> {
    /// 完整的 [`AccountSnapshot`] - 替换所有现有状态。
    Snapshot(AccountSnapshot<ExchangeKey, AssetKey, InstrumentKey>),

    /// 单个 [`AssetBalance`] 快照 - 替换现有余额状态。
    BalanceSnapshot(Snapshot<AssetBalance<AssetKey>>),

    /// 单个 [`Order`] 快照 - 如果更新则用于更新现有订单状态。
    ///
    /// 此变体涵盖一般订单更新和开仓订单响应。
    OrderSnapshot(Snapshot<Order<ExchangeKey, InstrumentKey, OrderState<AssetKey, InstrumentKey>>>),

    /// 对 [`OrderRequestCancel<ExchangeKey, InstrumentKey>`](order::request::OrderRequestOpen) 的响应。
    OrderCancelled(OrderResponseCancel<ExchangeKey, AssetKey, InstrumentKey>),

    /// [`Order<ExchangeKey, InstrumentKey, Open>`] 部分或完全成交。
    Trade(Trade<QuoteAsset, InstrumentKey>),
}

impl<ExchangeKey, AssetKey, InstrumentKey> AccountEvent<ExchangeKey, AssetKey, InstrumentKey>
where
    AssetKey: Eq,
    InstrumentKey: Eq,
{
    /// 如果此事件是快照，则提取账户快照。
    ///
    /// # 返回值
    ///
    /// 如果事件是 `Snapshot` 类型，返回 `Some(AccountSnapshot)`；否则返回 `None`。
    pub fn snapshot(self) -> Option<AccountSnapshot<ExchangeKey, AssetKey, InstrumentKey>> {
        match self.kind {
            AccountEventKind::Snapshot(snapshot) => Some(snapshot),
            _ => None,
        }
    }
}

/// 账户快照，包含完整的账户状态。
///
/// AccountSnapshot 表示账户在某个时间点的完整状态，包括所有余额和订单。
///
/// ## 类型参数
///
/// - `ExchangeKey`: 交易所键类型（默认：`ExchangeIndex`）
/// - `AssetKey`: 资产键类型（默认：`AssetIndex`）
/// - `InstrumentKey`: 交易对键类型（默认：`InstrumentIndex`）
///
/// ## 字段说明
///
/// - **exchange**: 交易所标识
/// - **balances**: 资产余额列表
/// - **instruments**: 交易对账户快照列表
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize, Constructor,
)]
pub struct AccountSnapshot<
    ExchangeKey = ExchangeIndex,
    AssetKey = AssetIndex,
    InstrumentKey = InstrumentIndex,
> {
    /// 交易所标识。
    pub exchange: ExchangeKey,
    /// 资产余额列表。
    pub balances: Vec<AssetBalance<AssetKey>>,
    /// 交易对账户快照列表。
    pub instruments: Vec<InstrumentAccountSnapshot<ExchangeKey, AssetKey, InstrumentKey>>,
}

/// 交易对账户快照，包含特定交易对的订单状态。
///
/// InstrumentAccountSnapshot 表示特定交易对在某个时间点的订单状态。
///
/// ## 类型参数
///
/// - `ExchangeKey`: 交易所键类型（默认：`ExchangeIndex`）
/// - `AssetKey`: 资产键类型（默认：`AssetIndex`）
/// - `InstrumentKey`: 交易对键类型（默认：`InstrumentIndex`）
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize, Constructor,
)]
pub struct InstrumentAccountSnapshot<
    ExchangeKey = ExchangeIndex,
    AssetKey = AssetIndex,
    InstrumentKey = InstrumentIndex,
> {
    /// 交易对标识。
    pub instrument: InstrumentKey,
    /// 订单快照列表（默认：空向量）。
    #[serde(default = "Vec::new")]
    pub orders: Vec<OrderSnapshot<ExchangeKey, AssetKey, InstrumentKey>>,
}

impl<ExchangeKey, AssetKey, InstrumentKey> AccountSnapshot<ExchangeKey, AssetKey, InstrumentKey> {
    /// 获取快照中最新的时间戳。
    ///
    /// 从所有订单和余额中查找最新的时间戳。
    ///
    /// # 返回值
    ///
    /// 返回最新的时间戳，如果没有时间戳则返回 `None`。
    pub fn time_most_recent(&self) -> Option<DateTime<Utc>> {
        let order_times = self.instruments.iter().flat_map(|instrument| {
            instrument
                .orders
                .iter()
                .filter_map(|order| order.state.time_exchange())
        });
        let balance_times = self.balances.iter().map(|balance| balance.time_exchange);

        order_times.chain(balance_times).max()
    }

    /// 返回所有资产的迭代器。
    ///
    /// # 返回值
    ///
    /// 返回资产键的迭代器。
    pub fn assets(&self) -> impl Iterator<Item = &AssetKey> {
        self.balances.iter().map(|balance| &balance.asset)
    }

    /// 返回所有交易对的迭代器。
    ///
    /// # 返回值
    ///
    /// 返回交易对键的迭代器。
    pub fn instruments(&self) -> impl Iterator<Item = &InstrumentKey> {
        self.instruments.iter().map(|snapshot| &snapshot.instrument)
    }
}
