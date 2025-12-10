//! SystemConfig 系统配置模块
//!
//! 本模块提供了用于配置交易系统组件的数据结构。
//! 包括交易对和执行组件的配置。
//!
//! # 核心概念
//!
//! - **SystemConfig**: 完整交易系统的顶级配置
//! - **InstrumentConfig**: 交易对配置
//! - **ExecutionConfig**: 执行组件配置

use barter_execution::client::mock::MockExecutionConfig;
use barter_instrument::{
    Underlying,
    asset::{Asset, name::AssetNameExchange},
    exchange::ExchangeId,
    instrument::{
        Instrument,
        kind::{
            InstrumentKind, future::FutureContract, option::OptionContract,
            perpetual::PerpetualContract,
        },
        name::{InstrumentNameExchange, InstrumentNameInternal},
        quote::InstrumentQuoteAsset,
        spec::{InstrumentSpec, InstrumentSpecQuantity, OrderQuantityUnits},
    },
};
use derive_more::From;
use serde::{Deserialize, Serialize};

/// 完整交易系统的顶级配置。
///
/// SystemConfig 包含系统将跟踪的所有交易对和执行组件的配置。
///
/// ## 字段说明
///
/// - **instruments**: 系统将跟踪的所有交易对的配置
/// - **executions**: 所有执行组件的配置
///
/// # 使用示例
///
/// ```rust,ignore
/// let config = SystemConfig {
///     instruments: vec![
///         InstrumentConfig {
///             exchange: ExchangeId::Binance,
///             name_exchange: "BTCUSDT".into(),
///             // ...
///         },
///     ],
///     executions: vec![
///         ExecutionConfig::Mock(mock_config),
///     ],
/// };
/// ```
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub struct SystemConfig {
    /// 系统将跟踪的所有交易对的配置。
    pub instruments: Vec<InstrumentConfig>,

    /// 所有执行组件的配置。
    pub executions: Vec<ExecutionConfig>,
}

/// 用于在启动时生成 [`Instrument`] 的便捷最小交易对配置。
///
/// InstrumentConfig 提供了配置交易对所需的最小信息集。
/// 它可以在系统启动时自动转换为完整的 `Instrument` 结构。
///
/// ## 字段说明
///
/// - **exchange**: 交易该交易对的交易所标识
/// - **name_exchange**: 交易所特定的交易对名称（例如，"BTCUSDT"）
/// - **underlying**: 交易对的标的资产对
/// - **quote**: 交易对的计价资产
/// - **kind**: 交易对类型（现货、永续、期货、期权）
/// - **spec**: 交易对的可选附加规格
///
/// # 使用示例
///
/// ```rust,ignore
/// let config = InstrumentConfig {
///     exchange: ExchangeId::Binance,
///     name_exchange: "BTCUSDT".into(),
///     underlying: Underlying {
///         base: "BTC".into(),
///         quote: "USDT".into(),
///     },
///     quote: InstrumentQuoteAsset::USDT,
///     kind: InstrumentKind::Spot,
///     spec: Some(InstrumentSpec { /* ... */ }),
/// };
/// ```
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub struct InstrumentConfig {
    /// 交易该交易对的交易所标识。
    pub exchange: ExchangeId,

    /// 交易所特定的交易对名称（例如，"BTCUSDT"）。
    pub name_exchange: InstrumentNameExchange,

    /// 交易对的标的资产对。
    pub underlying: Underlying<AssetNameExchange>,

    /// 交易对的计价资产。
    pub quote: InstrumentQuoteAsset,

    /// 交易对类型（现货、永续、期货、期权）。
    pub kind: InstrumentKind<AssetNameExchange>,

    /// 交易对的可选附加规格。
    pub spec: Option<InstrumentSpec<AssetNameExchange>>,
}

/// 执行链接的配置。
///
/// ExecutionConfig 表示不同类型的执行配置。目前仅支持用于回测的模拟执行。
///
/// ## 变体说明
///
/// - **Mock**: 用于回测的模拟执行配置
///
/// ## 未来扩展
///
/// 未来可能会添加真实交易所的执行配置。
///
/// # 使用示例
///
/// ```rust,ignore
/// let config = ExecutionConfig::Mock(MockExecutionConfig {
///     mocked_exchange: ExchangeId::Binance,
///     // ...
/// });
/// ```
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, From)]
#[serde(untagged)]
pub enum ExecutionConfig {
    /// 用于回测的模拟执行配置。
    Mock(MockExecutionConfig),
}

impl From<InstrumentConfig> for Instrument<ExchangeId, Asset> {
    fn from(value: InstrumentConfig) -> Self {
        Self {
            exchange: value.exchange,
            name_internal: InstrumentNameInternal::new_from_exchange_underlying(
                value.exchange,
                &value.underlying.base,
                &value.underlying.quote,
            ),
            name_exchange: value.name_exchange,
            underlying: Underlying {
                base: Asset::new_from_exchange(value.underlying.base),
                quote: Asset::new_from_exchange(value.underlying.quote),
            },
            quote: value.quote,
            kind: match value.kind {
                InstrumentKind::Spot => InstrumentKind::Spot,
                InstrumentKind::Perpetual(contract) => {
                    InstrumentKind::Perpetual(PerpetualContract {
                        contract_size: contract.contract_size,
                        settlement_asset: Asset::new_from_exchange(contract.settlement_asset),
                    })
                }
                InstrumentKind::Future(contract) => InstrumentKind::Future(FutureContract {
                    contract_size: contract.contract_size,
                    settlement_asset: Asset::new_from_exchange(contract.settlement_asset),
                    expiry: contract.expiry,
                }),
                InstrumentKind::Option(contract) => InstrumentKind::Option(OptionContract {
                    contract_size: contract.contract_size,
                    settlement_asset: Asset::new_from_exchange(contract.settlement_asset),
                    kind: contract.kind,
                    exercise: contract.exercise,
                    expiry: contract.expiry,
                    strike: contract.strike,
                }),
            },
            spec: value.spec.map(|spec| InstrumentSpec {
                price: spec.price,
                quantity: InstrumentSpecQuantity {
                    unit: match spec.quantity.unit {
                        OrderQuantityUnits::Asset(asset) => {
                            OrderQuantityUnits::Asset(Asset::new_from_exchange(asset))
                        }
                        OrderQuantityUnits::Contract => OrderQuantityUnits::Contract,
                        OrderQuantityUnits::Quote => OrderQuantityUnits::Quote,
                    },
                    min: spec.quantity.min,
                    increment: spec.quantity.increment,
                },
                notional: spec.notional,
            }),
        }
    }
}
