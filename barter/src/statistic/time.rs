//! TimeInterval 时间间隔模块
//!
//! 本模块定义了用于金融计算的时间间隔类型。
//! 时间间隔用于年化收益率、波动率等金融指标的计算。
//!
//! # 核心概念
//!
//! - **TimeInterval**: Trait，定义时间间隔接口
//! - **Annual365**: 365 天年化间隔（适用于加密货币等 24/7 交易）
//! - **Annual252**: 252 天年化间隔（适用于传统市场，每年 252 个交易日）
//! - **Daily**: 日间隔

use chrono::TimeDelta;
use serde::{Deserialize, Serialize};
use smol_str::{SmolStr, format_smolstr};
use std::fmt::Debug;

/// 表示用于金融计算的时间间隔类型的 Trait。
///
/// 实现此 Trait 的类型可以表示不同的时间周期（例如，日、年），
/// 并提供一致的方式来访问其持续时间和人类可读的名称。
///
/// ## 使用场景
///
/// - 年化收益率计算
/// - 年化波动率计算
/// - 时间序列分析
///
/// # 使用示例
///
/// ```rust
/// use barter::statistic::time::{TimeInterval, Daily, Annual252, Annual365};
///
/// // 日时间间隔
/// let daily = Daily;
/// assert_eq!(daily.name().as_str(), "Daily");
/// assert_eq!(daily.interval().num_days(), 1);
///
/// // 传统市场年化时间间隔（每年 252 个交易日）
/// let annual_traditional = Annual252;
/// assert_eq!(annual_traditional.name().as_str(), "Annual(252)");
/// assert_eq!(annual_traditional.interval().num_days(), 252);
///
/// // 加密货币年化时间间隔（24/7 交易）
/// let annual_crypto = Annual365;
/// assert_eq!(annual_crypto.name().as_str(), "Annual(365)");
/// assert_eq!(annual_crypto.interval().num_days(), 365);
/// ```
pub trait TimeInterval: Debug + Copy {
    /// 返回时间间隔的人类可读名称。
    ///
    /// # 返回值
    ///
    /// 返回时间间隔的名称字符串。
    fn name(&self) -> SmolStr;

    /// 返回时间间隔的持续时间。
    ///
    /// # 返回值
    ///
    /// 返回 `TimeDelta`，表示时间间隔的持续时间。
    fn interval(&self) -> TimeDelta;
}

/// 365 天年化时间间隔。
///
/// 适用于加密货币等 24/7 交易的市场，每年有 365 个交易日。
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Default, Deserialize, Serialize)]
pub struct Annual365;

impl TimeInterval for Annual365 {
    /// 返回 "Annual(365)"。
    fn name(&self) -> SmolStr {
        SmolStr::new("Annual(365)")
    }

    /// 返回 365 天的 TimeDelta。
    fn interval(&self) -> TimeDelta {
        TimeDelta::days(365)
    }
}

/// 252 天年化时间间隔。
///
/// 适用于传统市场，每年通常有 252 个交易日（排除周末和节假日）。
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Default, Deserialize, Serialize)]
pub struct Annual252;

impl TimeInterval for Annual252 {
    /// 返回 "Annual(252)"。
    fn name(&self) -> SmolStr {
        SmolStr::new("Annual(252)")
    }

    /// 返回 252 天的 TimeDelta。
    fn interval(&self) -> TimeDelta {
        TimeDelta::days(252)
    }
}

/// 日时间间隔。
///
/// 表示单个交易日的时间间隔。
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Default, Deserialize, Serialize)]
pub struct Daily;

impl TimeInterval for Daily {
    /// 返回 "Daily"。
    fn name(&self) -> SmolStr {
        SmolStr::new("Daily")
    }

    /// 返回 1 天的 TimeDelta。
    fn interval(&self) -> TimeDelta {
        TimeDelta::days(1)
    }
}

impl TimeInterval for TimeDelta {
    /// 返回以分钟为单位的持续时间名称。
    fn name(&self) -> SmolStr {
        format_smolstr!("Duration {} (minutes)", self.num_minutes())
    }

    /// 返回 TimeDelta 本身。
    fn interval(&self) -> TimeDelta {
        *self
    }
}
