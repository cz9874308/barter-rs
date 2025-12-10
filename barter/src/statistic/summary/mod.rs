//! Summary 统计摘要模块
//!
//! 本模块提供了金融数据集的统计摘要。
//! 包括交易摘要、交易对摘要、资产摘要等。
//!
//! # 核心概念
//!
//! - **TradingSummary**: 交易摘要，包含所有交易对和资产的统计信息
//! - **TearSheet**: 交易对摘要，包含单个交易对的详细统计
//! - **TearSheetAsset**: 资产摘要，包含单个资产的统计信息
//! - **PnLReturns**: 盈亏收益率统计

use crate::{
    engine::state::{asset::AssetStates, instrument::InstrumentStates, position::PositionExited},
    statistic::{
        summary::{
            asset::{TearSheetAsset, TearSheetAssetGenerator},
            instrument::{TearSheet, TearSheetGenerator},
        },
        time::TimeInterval,
    },
};
use barter_execution::balance::AssetBalance;
use barter_instrument::{
    asset::{AssetIndex, ExchangeAsset, name::AssetNameInternal},
    instrument::{InstrumentIndex, name::InstrumentNameInternal},
};
use barter_integration::{collection::FnvIndexMap, snapshot::Snapshot};
use chrono::{DateTime, TimeDelta, Utc};
use derive_more::Constructor;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// 资产摘要模块。
pub mod asset;

/// 数据集统计模块。
pub mod dataset;

/// 显示格式化模块。
pub mod display;

/// 交易对摘要模块。
pub mod instrument;

/// 盈亏收益率模块。
pub mod pnl;

/// 交易摘要，包含交易会话的完整统计信息。
///
/// TradingSummary 是交易系统的完整统计摘要，包含所有交易对和资产的统计信息。
/// 它提供了交易会话的整体绩效视图。
///
/// ## 类型参数
///
/// - `Interval`: 时间间隔类型，用于年化指标
///
/// ## 字段说明
///
/// - **time_engine_start**: 交易会话开始时间
/// - **time_engine_end**: 交易会话结束时间
/// - **instruments**: 交易对摘要映射
/// - **assets**: 资产摘要映射
///
/// ## 注意事项
///
/// 交易对是交易所特定的，因此例如 Binance btc_usdt_spot 和 Okx btc_usdt_spot
/// 将由不同的 [`TearSheet`] 汇总。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Constructor)]
pub struct TradingSummary<Interval> {
    /// 由 [`Engine`](crate::engine::Engine) 时钟定义的交易会话开始时间。
    pub time_engine_start: DateTime<Utc>,

    /// 由 [`Engine`](crate::engine::Engine) 时钟定义的交易会话结束时间。
    pub time_engine_end: DateTime<Utc>,

    /// 交易对 [`TearSheet`] 映射。
    ///
    /// 注意：交易对是交易所特定的，因此例如 Binance btc_usdt_spot 和 Okx btc_usdt_spot
    /// 将由不同的 [`TearSheet`] 汇总。
    pub instruments: FnvIndexMap<InstrumentNameInternal, TearSheet<Interval>>,

    /// [`ExchangeAsset`] [`TearSheet`] 映射。
    pub assets: FnvIndexMap<ExchangeAsset<AssetNameInternal>, TearSheetAsset>,
}

impl<Interval> TradingSummary<Interval> {
    /// `TradingSummary` 覆盖的交易持续时间。
    ///
    /// # 返回值
    ///
    /// 返回从交易开始到结束的时间差。
    pub fn trading_duration(&self) -> TimeDelta {
        self.time_engine_end
            .signed_duration_since(self.time_engine_start)
    }
}

/// [`TradingSummary`] 的生成器。
///
/// TradingSummaryGenerator 用于增量构建交易摘要。它跟踪所有交易对和资产的统计信息，
/// 并可以在任何时刻生成完整的 TradingSummary。
///
/// ## 字段说明
///
/// - **risk_free_return**: 无风险收益率（用于计算风险调整指标）
/// - **time_engine_start**: 交易会话开始时间
/// - **time_engine_now**: 交易会话最新更新时间
/// - **instruments**: 交易对摘要生成器映射
/// - **assets**: 资产摘要生成器映射
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Constructor)]
pub struct TradingSummaryGenerator {
    /// 零风险投资的理论收益率。
    ///
    /// 用于计算风险调整指标（如 Sharpe Ratio、Sortino Ratio 等）。
    ///
    /// ## 参考文档
    ///
    /// <https://www.investopedia.com/terms/r/risk-freerate.asp>
    pub risk_free_return: Decimal,

    /// 由 [`Engine`](crate::engine::Engine) 时钟定义的交易会话摘要开始时间。
    pub time_engine_start: DateTime<Utc>,

    /// 由 [`Engine`](crate::engine::Engine) 时钟定义的交易会话摘要最新更新时间。
    pub time_engine_now: DateTime<Utc>,

    /// 交易对 [`TearSheetGenerator`] 映射。
    ///
    /// 注意：交易对是交易所特定的，因此例如 Binance btc_usdt_spot 和 Okx btc_usdt_spot
    /// 将由不同的 [`TearSheet`] 汇总。
    pub instruments: FnvIndexMap<InstrumentNameInternal, TearSheetGenerator>,

    /// [`ExchangeAsset`] [`TearSheetAssetGenerator`] 映射。
    pub assets: FnvIndexMap<ExchangeAsset<AssetNameInternal>, TearSheetAssetGenerator>,
}

impl TradingSummaryGenerator {
    /// Initialise a [`TradingSummaryGenerator`] from a `risk_free_return` value, and initial
    /// indexed state.
    pub fn init<InstrumentData>(
        risk_free_return: Decimal,
        time_engine_start: DateTime<Utc>,
        time_engine_now: DateTime<Utc>,
        instruments: &InstrumentStates<InstrumentData>,
        assets: &AssetStates,
    ) -> Self {
        Self {
            risk_free_return,
            time_engine_start,
            time_engine_now,
            instruments: instruments
                .0
                .values()
                .map(|state| {
                    (
                        state.instrument.name_internal.clone(),
                        state.tear_sheet.clone(),
                    )
                })
                .collect(),
            assets: assets
                .0
                .iter()
                .map(|(asset, state)| (asset.clone(), state.statistics.clone()))
                .collect(),
        }
    }

    /// Update the [`TradingSummaryGenerator`] `time_now`.
    pub fn update_time_now(&mut self, time_now: DateTime<Utc>) {
        self.time_engine_now = time_now;
    }

    /// Update the [`TradingSummaryGenerator`] from the next [`PositionExited`].
    pub fn update_from_position<AssetKey, InstrumentKey>(
        &mut self,
        position: &PositionExited<AssetKey, InstrumentKey>,
    ) where
        Self: InstrumentTearSheetManager<InstrumentKey>,
    {
        if self.time_engine_now < position.time_exit {
            self.time_engine_now = position.time_exit;
        }

        self.instrument_mut(&position.instrument)
            .update_from_position(position)
    }

    /// Update the [`TradingSummaryGenerator`] from the next [`Snapshot`] [`AssetBalance`].
    pub fn update_from_balance<AssetKey>(&mut self, balance: Snapshot<&AssetBalance<AssetKey>>)
    where
        Self: AssetTearSheetManager<AssetKey>,
    {
        if self.time_engine_now < balance.0.time_exchange {
            self.time_engine_now = balance.0.time_exchange;
        }

        self.asset_mut(&balance.0.asset)
            .update_from_balance(balance)
    }

    /// Generate the latest [`TradingSummary`] at the specific [`TimeInterval`].
    ///
    /// For example, pass [`Annual365`](super::time::Annual365) to generate a crypto-centric
    /// (24/7 trading) annualised [`TradingSummary`].
    pub fn generate<Interval>(&mut self, interval: Interval) -> TradingSummary<Interval>
    where
        Interval: TimeInterval,
    {
        let instruments = self
            .instruments
            .iter_mut()
            .map(|(instrument, tear_sheet)| {
                (
                    instrument.clone(),
                    tear_sheet.generate(self.risk_free_return, interval),
                )
            })
            .collect();

        let assets = self
            .assets
            .iter_mut()
            .map(|(asset, tear_sheet)| (asset.clone(), tear_sheet.generate()))
            .collect();

        TradingSummary {
            time_engine_start: self.time_engine_start,
            time_engine_end: self.time_engine_now,
            instruments,
            assets,
        }
    }
}

pub trait InstrumentTearSheetManager<InstrumentKey> {
    fn instrument(&self, key: &InstrumentKey) -> &TearSheetGenerator;
    fn instrument_mut(&mut self, key: &InstrumentKey) -> &mut TearSheetGenerator;
}

impl InstrumentTearSheetManager<InstrumentNameInternal> for TradingSummaryGenerator {
    fn instrument(&self, key: &InstrumentNameInternal) -> &TearSheetGenerator {
        self.instruments
            .get(key)
            .unwrap_or_else(|| panic!("TradingSummaryGenerator does not contain: {key}"))
    }

    fn instrument_mut(&mut self, key: &InstrumentNameInternal) -> &mut TearSheetGenerator {
        self.instruments
            .get_mut(key)
            .unwrap_or_else(|| panic!("TradingSummaryGenerator does not contain: {key}"))
    }
}

impl InstrumentTearSheetManager<InstrumentIndex> for TradingSummaryGenerator {
    fn instrument(&self, key: &InstrumentIndex) -> &TearSheetGenerator {
        self.instruments
            .get_index(key.index())
            .map(|(_key, state)| state)
            .unwrap_or_else(|| panic!("TradingSummaryGenerator does not contain: {key}"))
    }

    fn instrument_mut(&mut self, key: &InstrumentIndex) -> &mut TearSheetGenerator {
        self.instruments
            .get_index_mut(key.index())
            .map(|(_key, state)| state)
            .unwrap_or_else(|| panic!("TradingSummaryGenerator does not contain: {key}"))
    }
}

pub trait AssetTearSheetManager<AssetKey> {
    fn asset(&self, key: &AssetKey) -> &TearSheetAssetGenerator;
    fn asset_mut(&mut self, key: &AssetKey) -> &mut TearSheetAssetGenerator;
}

impl AssetTearSheetManager<AssetIndex> for TradingSummaryGenerator {
    fn asset(&self, key: &AssetIndex) -> &TearSheetAssetGenerator {
        self.assets
            .get_index(key.index())
            .map(|(_key, state)| state)
            .unwrap_or_else(|| panic!("TradingSummaryGenerator does not contain: {key}"))
    }

    fn asset_mut(&mut self, key: &AssetIndex) -> &mut TearSheetAssetGenerator {
        self.assets
            .get_index_mut(key.index())
            .map(|(_key, state)| state)
            .unwrap_or_else(|| panic!("TradingSummaryGenerator does not contain: {key}"))
    }
}

impl AssetTearSheetManager<ExchangeAsset<AssetNameInternal>> for TradingSummaryGenerator {
    fn asset(&self, key: &ExchangeAsset<AssetNameInternal>) -> &TearSheetAssetGenerator {
        self.assets
            .get(key)
            .unwrap_or_else(|| panic!("TradingSummaryGenerator does not contain: {key:?}"))
    }

    fn asset_mut(
        &mut self,
        key: &ExchangeAsset<AssetNameInternal>,
    ) -> &mut TearSheetAssetGenerator {
        self.assets
            .get_mut(key)
            .unwrap_or_else(|| panic!("TradingSummaryGenerator does not contain: {key:?}"))
    }
}
