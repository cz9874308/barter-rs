//! Sortino Ratio 索提诺比率模块
//!
//! 本模块提供了 Sortino Ratio（索提诺比率）的计算逻辑。
//! 索提诺比率类似于夏普比率，但只考虑下行波动率（负收益的标准差）而不是总波动率。
//! 这使得它对于非正态收益分布的资产组合来说是更好的指标。
//!
//! # 计算公式
//!
//! `Sortino Ratio = (平均收益率 - 无风险收益率) / 下行收益率标准差`
//!
//! # 与 Sharpe Ratio 的区别
//!
//! - Sharpe Ratio 使用总波动率（所有收益的标准差）
//! - Sortino Ratio 只使用下行波动率（负收益的标准差）
//! - Sortino Ratio 更适合评估有偏收益分布的策略

use crate::statistic::time::TimeInterval;
use rust_decimal::{Decimal, MathematicalOps};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// 表示特定 [`TimeInterval`] 上的 Sortino Ratio 值。
///
/// Sortino Ratio 类似于 Sharpe Ratio，但只考虑下行波动率（负收益的标准差）
/// 而不是总波动率。这使得它对于非正态收益分布的资产组合来说是更好的指标。
///
/// ## 解释
///
/// - **高 Sortino Ratio**: 表示在承担相同下行风险的情况下获得了更高的收益
/// - **低 Sortino Ratio**: 表示下行风险调整后的收益较低
/// - **负 Sortino Ratio**: 表示投资表现不如无风险资产
///
/// ## 类型参数
///
/// - `Interval`: 时间间隔类型
#[derive(Debug, Clone, PartialEq, PartialOrd, Default, Deserialize, Serialize)]
pub struct SortinoRatio<Interval> {
    /// Sortino Ratio 值。
    pub value: Decimal,
    /// 时间间隔。
    pub interval: Interval,
}

impl<Interval> SortinoRatio<Interval>
where
    Interval: TimeInterval,
{
    /// 在提供的 [`TimeInterval`] 上计算 [`SortinoRatio`]。
    ///
    /// ## 计算公式
    ///
    /// `Sortino Ratio = (平均收益率 - 无风险收益率) / 下行收益率标准差`
    ///
    /// ## 特殊情况
    ///
    /// 如果下行标准差为零（无下行风险）：
    /// - 超额收益为正：返回 `Decimal::MAX`（表示无限好）
    /// - 超额收益为负：返回 `Decimal::MIN`（表示无限差）
    /// - 超额收益为零：返回 `Decimal::ZERO`（中性）
    ///
    /// # 参数
    ///
    /// - `risk_free_return`: 无风险收益率
    /// - `mean_return`: 平均收益率
    /// - `std_dev_loss_returns`: 下行收益率标准差（只考虑负收益）
    /// - `returns_period`: 收益率的时间间隔
    ///
    /// # 返回值
    ///
    /// 返回计算得到的 SortinoRatio。
    pub fn calculate(
        risk_free_return: Decimal,
        mean_return: Decimal,
        std_dev_loss_returns: Decimal,
        returns_period: Interval,
    ) -> Self {
        if std_dev_loss_returns.is_zero() {
            Self {
                value: match mean_return.cmp(&risk_free_return) {
                    // 特殊情况：正超额收益且无下行风险（非常好）
                    Ordering::Greater => Decimal::MAX,
                    // 特殊情况：负超额收益且无下行风险（非常差）
                    Ordering::Less => Decimal::MIN,
                    // 特殊情况：无超额收益且无下行风险（中性）
                    Ordering::Equal => Decimal::ZERO,
                },
                interval: returns_period,
            }
        } else {
            let excess_returns = mean_return - risk_free_return;
            let ratio = excess_returns.checked_div(std_dev_loss_returns).unwrap();
            Self {
                value: ratio,
                interval: returns_period,
            }
        }
    }

    /// 将 [`SortinoRatio`] 从当前 [`TimeInterval`] 缩放到提供的 [`TimeInterval`]。
    ///
    /// 此缩放假设收益率是独立同分布（IID）的。然而，这个假设对于下行偏差可能不太合适。
    ///
    /// ## 缩放公式
    ///
    /// `scaled_value = value * sqrt(target_interval / current_interval)`
    ///
    /// ## 类型参数
    ///
    /// - `TargetInterval`: 目标时间间隔类型
    ///
    /// # 参数
    ///
    /// - `target`: 目标时间间隔
    ///
    /// # 返回值
    ///
    /// 返回缩放后的 SortinoRatio。
    pub fn scale<TargetInterval>(self, target: TargetInterval) -> SortinoRatio<TargetInterval>
    where
        TargetInterval: TimeInterval,
    {
        // 确定缩放因子：目标间隔与当前间隔比值的平方根
        let target_secs = Decimal::from(target.interval().num_seconds());
        let current_secs = Decimal::from(self.interval.interval().num_seconds());

        let scale = target_secs
            .abs()
            .checked_div(current_secs.abs())
            .unwrap_or(Decimal::MAX)
            .sqrt()
            .expect("ensured seconds are Positive");

        SortinoRatio {
            value: self.value.checked_mul(scale).unwrap_or(Decimal::MAX),
            interval: target,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::statistic::time::{Annual252, Daily};
    use chrono::TimeDelta;
    use rust_decimal_macros::dec;
    use std::str::FromStr;

    #[test]
    fn test_sortino_ratio_normal_case() {
        // Define test case with reasonable values
        let risk_free_return = dec!(0.0015); // 0.15%
        let mean_return = dec!(0.0025); // 0.25%
        let std_dev_loss_returns = dec!(0.02); // 2%
        let time_period = Daily;

        let actual = SortinoRatio::calculate(
            risk_free_return,
            mean_return,
            std_dev_loss_returns,
            time_period,
        );

        let expected = SortinoRatio {
            value: dec!(0.05), // (0.0025 - 0.0015) / 0.02
            interval: time_period,
        };

        assert_eq!(actual.value, expected.value);
        assert_eq!(actual.interval, time_period);
    }

    #[test]
    fn test_sortino_ratio_zero_downside_dev_positive_excess() {
        // Test case: positive excess returns with no downside risk
        let risk_free_return = dec!(0.001); // 0.1%
        let mean_return = dec!(0.002); // 0.2%
        let std_dev_loss_returns = dec!(0.0);
        let time_period = Daily;

        let actual = SortinoRatio::calculate(
            risk_free_return,
            mean_return,
            std_dev_loss_returns,
            time_period,
        );

        assert_eq!(actual.value, Decimal::MAX);
        assert_eq!(actual.interval, time_period);
    }

    #[test]
    fn test_sortino_ratio_zero_downside_dev_negative_excess() {
        // Test case: negative excess returns with no downside risk
        let risk_free_return = dec!(0.002); // 0.2%
        let mean_return = dec!(0.001); // 0.1%
        let std_dev_loss_returns = dec!(0.0);
        let time_period = Daily;

        let actual = SortinoRatio::calculate(
            risk_free_return,
            mean_return,
            std_dev_loss_returns,
            time_period,
        );

        assert_eq!(actual.value, Decimal::MIN);
        assert_eq!(actual.interval, time_period);
    }

    #[test]
    fn test_sortino_ratio_zero_downside_dev_no_excess() {
        // Test case: no excess returns with no downside risk
        let risk_free_return = dec!(0.001); // 0.1%
        let mean_return = dec!(0.001); // 0.1%
        let std_dev_loss_returns = dec!(0.0);
        let time_period = Daily;

        let actual = SortinoRatio::calculate(
            risk_free_return,
            mean_return,
            std_dev_loss_returns,
            time_period,
        );

        assert_eq!(actual.value, dec!(0.0));
        assert_eq!(actual.interval, time_period);
    }

    #[test]
    fn test_sortino_ratio_negative_returns() {
        // Test case: negative mean returns
        let risk_free_return = dec!(0.001); // 0.1%
        let mean_return = dec!(-0.002); // -0.2%
        let std_dev_loss_returns = dec!(0.015); // 1.5%
        let time_period = Daily;

        let actual = SortinoRatio::calculate(
            risk_free_return,
            mean_return,
            std_dev_loss_returns,
            time_period,
        );

        let expected = SortinoRatio {
            value: dec!(-0.2), // (-0.002 - 0.001) / 0.015
            interval: time_period,
        };

        assert_eq!(actual.value, expected.value);
        assert_eq!(actual.interval, expected.interval);
    }

    #[test]
    fn test_sortino_ratio_custom_interval() {
        // Test case with custom time interval
        let risk_free_return = dec!(0.0015); // 0.15%
        let mean_return = dec!(0.0025); // 0.25%
        let std_dev_loss_returns = dec!(0.02); // 2%
        let time_period = TimeDelta::hours(4);

        let actual = SortinoRatio::calculate(
            risk_free_return,
            mean_return,
            std_dev_loss_returns,
            time_period,
        );

        let expected = SortinoRatio {
            value: dec!(0.05),
            interval: time_period,
        };

        assert_eq!(actual.value, expected.value);
        assert_eq!(actual.interval, expected.interval);
    }

    #[test]
    fn test_sortino_ratio_scale_daily_to_annual() {
        // Test scaling from daily to annual
        let daily = SortinoRatio {
            value: dec!(0.05),
            interval: Daily,
        };

        let actual = daily.scale(Annual252);

        // 0.05 * √252 ≈ 0.7937
        let expected = SortinoRatio {
            value: Decimal::from_str("0.7937").unwrap(),
            interval: Annual252,
        };

        let diff = (actual.value - expected.value).abs();
        assert!(diff <= Decimal::from_str("0.0001").unwrap());
        assert_eq!(actual.interval, expected.interval);
    }

    #[test]
    fn test_sortino_ratio_scale_custom_intervals() {
        // Test scaling between custom intervals
        let two_hour = SortinoRatio {
            value: dec!(0.05),
            interval: TimeDelta::hours(2),
        };

        let actual = two_hour.scale(TimeDelta::hours(8));

        // 0.05 * √4 = 0.1
        let expected = SortinoRatio {
            value: dec!(0.1),
            interval: TimeDelta::hours(8),
        };

        assert_eq!(actual.value, expected.value);
        assert_eq!(actual.interval, expected.interval);
    }

    #[test]
    fn test_sortino_ratio_extreme_values() {
        // Test with very small values
        let small = SortinoRatio::calculate(
            Decimal::from_scientific("1e-10").unwrap(),
            Decimal::from_scientific("2e-10").unwrap(),
            Decimal::from_scientific("1e-10").unwrap(),
            Daily,
        );

        let diff = (small.value - dec!(1.0)).abs();
        assert!(diff <= Decimal::from_str("0.0001").unwrap());

        // Test with very large values
        let large = SortinoRatio::calculate(
            Decimal::from_scientific("1e10").unwrap(),
            Decimal::from_scientific("2e10").unwrap(),
            Decimal::from_scientific("1e10").unwrap(),
            Daily,
        );

        let diff = (large.value - dec!(1.0)).abs();
        assert!(diff <= Decimal::from_str("0.0001").unwrap());
    }
}
