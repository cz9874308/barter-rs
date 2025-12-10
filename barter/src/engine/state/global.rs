//! EngineState 默认全局数据模块
//!
//! 本模块提供了 `DefaultGlobalData`，一个空的全局数据实现，可用于不需要特定全局数据状态的
//! Strategy 和 RiskManager 组合。
//!
//! # 核心概念
//!
//! - **DefaultGlobalData**: 默认全局数据，空实现，不存储任何状态
//! - **零成本抽象**: 当不需要全局数据时，使用此类型可以避免不必要的开销
//!
//! # 使用场景
//!
//! - 简单的策略和风险管理器组合
//! - 不需要全局状态的交易系统
//! - 作为全局数据的默认实现

use crate::engine::Processor;
use barter_data::event::MarketEvent;
use barter_execution::AccountEvent;
use serde::{Deserialize, Serialize};

/// 空的 `GlobalData`，可用于不需要特定全局数据状态的 `Strategy` 和 `RiskManager` 组合。
///
/// DefaultGlobalData 是一个零大小的类型（ZST），不存储任何数据。它实现了 `Processor` trait
/// 来处理账户事件和市场事件，但所有处理都是空操作。
///
/// ## 为什么需要这个类型？
///
/// 当 Strategy 和 RiskManager 不需要全局状态时，使用 `DefaultGlobalData` 可以：
///
/// - **零成本**: 不占用任何内存
/// - **类型安全**: 满足 EngineState 的类型约束
/// - **简化实现**: 不需要实现自定义的全局数据类型
///
/// ## 使用场景
///
/// - 简单的策略实现
/// - 不需要全局状态的交易系统
/// - 作为全局数据的默认占位符
///
/// # 使用示例
///
/// ```rust,ignore
/// // 使用默认全局数据创建 EngineState
/// let engine_state = EngineState::builder(
///     &indexed_instruments,
///     DefaultGlobalData, // 使用默认全局数据
///     |instrument| InstrumentData::new(instrument),
/// )
/// .build();
/// ```
#[derive(
    Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default, Deserialize, Serialize,
)]
pub struct DefaultGlobalData;

impl<ExchangeKey, AssetKey, InstrumentKey>
    Processor<&AccountEvent<ExchangeKey, AssetKey, InstrumentKey>> for DefaultGlobalData
{
    type Audit = ();

    /// 处理账户事件（空操作）。
    ///
    /// DefaultGlobalData 不存储任何状态，因此此方法为空实现。
    ///
    /// # 参数
    ///
    /// - `_`: 账户事件（未使用）
    ///
    /// # 返回值
    ///
    /// 返回空审计 `()`。
    fn process(&mut self, _: &AccountEvent<ExchangeKey, AssetKey, InstrumentKey>) -> Self::Audit {}
}

impl<InstrumentKey, Kind> Processor<&MarketEvent<InstrumentKey, Kind>> for DefaultGlobalData {
    type Audit = ();

    /// 处理市场事件（空操作）。
    ///
    /// DefaultGlobalData 不存储任何状态，因此此方法为空实现。
    ///
    /// # 参数
    ///
    /// - `_`: 市场事件（未使用）
    ///
    /// # 返回值
    ///
    /// 返回空审计 `()`。
    fn process(&mut self, _: &MarketEvent<InstrumentKey, Kind>) -> Self::Audit {}
}
