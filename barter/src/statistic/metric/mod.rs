//! Metric 金融指标模块
//!
//! 本模块提供了各种金融指标的计算逻辑，用于评估交易策略的绩效。
//! 所有指标都支持在不同时间间隔上计算，并可以相互转换。
//!
//! # 核心指标
//!
//! - **Sharpe Ratio**: 夏普比率，衡量风险调整后的收益
//! - **Sortino Ratio**: 索提诺比率，只考虑下行波动率
//! - **Calmar Ratio**: 卡尔玛比率，使用最大回撤作为风险度量
//! - **Profit Factor**: 盈利因子，衡量盈利与亏损的比率
//! - **Rate Of Return**: 收益率
//! - **Win Rate**: 胜率
//! - **Drawdown**: 回撤

/// Calmar Ratio 计算逻辑。
pub mod calmar;

/// Drawdown 回撤计算逻辑。
pub mod drawdown;

/// Profit Factor 盈利因子计算逻辑。
pub mod profit_factor;

/// Rate Of Return 收益率计算逻辑。
pub mod rate_of_return;

/// Sharpe Ratio 夏普比率计算逻辑。
pub mod sharpe;

/// Sortino Ratio 索提诺比率计算逻辑。
pub mod sortino;

/// Win Rate 胜率计算逻辑。
pub mod win_rate;
