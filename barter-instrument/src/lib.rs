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

//! # Barter-Instrument
//! Barter-Instrument 包含核心的 Exchange、Instrument 和 Asset 数据结构及相关工具。
//!
//! ## 示例
//! 有关全面的示例集合，请参见 Barter 核心 Engine 的 /examples 目录。

use derive_more::Constructor;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

/// 定义覆盖所有交易所的全局 [`ExchangeId`](exchange::ExchangeId) 枚举。
pub mod exchange;

/// [`Asset`](asset::Asset) 相关的数据结构。
///
/// 例如：`AssetKind`、`AssetNameInternal` 等。
pub mod asset;

/// [`Instrument`](instrument::Instrument) 相关的数据结构。
///
/// 例如：`InstrumentKind`、`OptionContract` 等。
pub mod instrument;

/// 交易所、资产和交易对的索引集合。提供用于索引非索引集合的构建器工具。
pub mod index;

/// 带键的值。
///
/// Keyed 将键和值组合在一起，用于索引集合。
///
/// ## 类型参数
///
/// - `Key`: 键类型
/// - `Value`: 值类型
///
/// ## 使用示例
///
/// `Keyed<InstrumentIndex, Instrument>`
#[derive(
    Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, Constructor,
)]
pub struct Keyed<Key, Value> {
    /// 键。
    pub key: Key,
    /// 值。
    pub value: Value,
}

impl<Key, Value> AsRef<Value> for Keyed<Key, Value> {
    fn as_ref(&self) -> &Value {
        &self.value
    }
}

impl<Key, Value> Display for Keyed<Key, Value>
where
    Key: Display,
    Value: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}, {}", self.key, self.value)
    }
}

/// 交易对底层资产，包含基础资产和报价资产。
///
/// Underlying 表示交易对的基础资产和报价资产。
///
/// ## 类型参数
///
/// - `AssetKey`: 资产键类型
///
/// ## 使用示例
///
/// `Underlying { base: "btc", quote: "usdt" }`
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub struct Underlying<AssetKey> {
    /// 基础资产。
    pub base: AssetKey,
    /// 报价资产。
    pub quote: AssetKey,
}

impl<AssetKey> Underlying<AssetKey> {
    /// 创建新的 Underlying。
    ///
    /// # 参数
    ///
    /// - `base`: 基础资产（可转换为 `AssetKey`）
    /// - `quote`: 报价资产（可转换为 `AssetKey`）
    ///
    /// # 返回值
    ///
    /// 返回新创建的 `Underlying` 实例。
    pub fn new<A>(base: A, quote: A) -> Self
    where
        A: Into<AssetKey>,
    {
        Self {
            base: base.into(),
            quote: quote.into(),
        }
    }
}

/// 交易或持仓的 [`Side`] - 买入或卖出。
///
/// Side 表示交易的方向，可以是买入（Buy）或卖出（Sell）。
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Deserialize, Serialize)]
pub enum Side {
    /// 买入。
    #[serde(alias = "buy", alias = "BUY", alias = "b")]
    Buy,
    /// 卖出。
    #[serde(alias = "sell", alias = "SELL", alias = "s")]
    Sell,
}

impl Display for Side {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Side::Buy => "buy",
                Side::Sell => "sell",
            }
        )
    }
}

pub mod test_utils {
    use crate::{
        Underlying,
        asset::{
            Asset, ExchangeAsset,
            name::{AssetNameExchange, AssetNameInternal},
        },
        exchange::ExchangeId,
        instrument::{
            Instrument,
            kind::InstrumentKind,
            name::{InstrumentNameExchange, InstrumentNameInternal},
            quote::InstrumentQuoteAsset,
        },
    };

    pub fn exchange_asset(exchange: ExchangeId, symbol: &str) -> ExchangeAsset<Asset> {
        ExchangeAsset {
            exchange,
            asset: asset(symbol),
        }
    }

    pub fn asset(symbol: &str) -> Asset {
        Asset {
            name_internal: AssetNameInternal::from(symbol),
            name_exchange: AssetNameExchange::from(symbol),
        }
    }

    pub fn instrument(
        exchange: ExchangeId,
        base: &str,
        quote: &str,
    ) -> Instrument<ExchangeId, Asset> {
        let name_exchange = InstrumentNameExchange::from(format!("{base}_{quote}"));
        let name_internal =
            InstrumentNameInternal::new_from_exchange(exchange, name_exchange.clone());
        let base_asset = asset(base);
        let quote_asset = asset(quote);

        Instrument::new(
            exchange,
            name_internal,
            name_exchange,
            Underlying::new(base_asset, quote_asset),
            InstrumentQuoteAsset::UnderlyingQuote,
            InstrumentKind::Spot,
            None,
        )
    }
}
