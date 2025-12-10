//! Calmar Ratio 卡尔玛比率模块
//!
//! 本模块提供了 Calmar Ratio（卡尔玛比率）的计算逻辑。
//! 卡尔玛比率是一种风险调整后的收益指标，将超额收益（超过无风险利率）除以最大回撤风险。
//! 它类似于 Sharpe 和 Sortino 比率，但使用最大回撤作为风险度量而不是标准差。
//!
//! # 计算公式
//!
//! `Calmar Ratio = (平均收益率 - 无风险收益率) / 最大回撤`
//!
//! # 参考文档
//!
//! <https://corporatefinanceinstitute.com/resources/career-map/sell-side/capital-markets/calmar-ratio/>

use crate::statistic::time::TimeInterval;
use rust_decimal::{Decimal, MathematicalOps};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// 表示特定 [`TimeInterval`] 上的 Calmar Ratio 值。
///
/// Calmar Ratio 是一种风险调整后的收益指标，将超额收益（超过无风险利率）
/// 除以最大回撤风险。它类似于 Sharpe 和 Sortino 比率，但使用最大回撤作为风险度量
/// 而不是标准差。
///
/// ## 解释
///
/// - **高 Calmar Ratio**: 表示在承担相同最大回撤风险的情况下获得了更高的收益
/// - **低 Calmar Ratio**: 表示最大回撤风险调整后的收益较低
/// - **负 Calmar Ratio**: 表示投资表现不如无风险资产
///
/// ## 类型参数
///
/// - `Interval`: 时间间隔类型
///
/// ## 参考文档
///
/// <https://corporatefinanceinstitute.com/resources/career-map/sell-side/capital-markets/calmar-ratio/>
#[derive(Debug, Clone, PartialEq, PartialOrd, Default, Deserialize, Serialize)]
pub struct CalmarRatio<Interval> {
    /// Calmar Ratio 值。
    pub value: Decimal,
    /// 时间间隔。
    pub interval: Interval,
}

impl<Interval> CalmarRatio<Interval>
where
    Interval: TimeInterval,
{
    /// 在提供的 [`TimeInterval`] 上计算 [`CalmarRatio`]。
    ///
    /// ## 计算公式
    ///
    /// `Calmar Ratio = (平均收益率 - 无风险收益率) / |最大回撤|`
    ///
    /// ## 特殊情况
    ///
    /// 如果最大回撤为零（无回撤风险）：
    /// - 超额收益为正：返回 `Decimal::MAX`（表示无限好）
    /// - 超额收益为负：返回 `Decimal::MIN`（表示无限差）
    /// - 超额收益为零：返回 `Decimal::ZERO`（中性）
    ///
    /// # 参数
    ///
    /// - `risk_free_return`: 无风险收益率
    /// - `mean_return`: 平均收益率
    /// - `max_drawdown`: 最大回撤（绝对值会被使用）
    /// - `returns_period`: 收益率的时间间隔
    ///
    /// # 返回值
    ///
    /// 返回计算得到的 CalmarRatio。
    pub fn calculate(
        risk_free_return: Decimal,
        mean_return: Decimal,
        max_drawdown: Decimal,
        returns_period: Interval,
    ) -> Self {
        if max_drawdown.is_zero() {
            Self {
                value: match mean_return.cmp(&risk_free_return) {
                    // 特殊情况：正超额收益且无回撤风险（非常好）
                    Ordering::Greater => Decimal::MAX,
                    // 特殊情况：负超额收益且无回撤风险（非常差）
                    Ordering::Less => Decimal::MIN,
                    // 特殊情况：无超额收益且无回撤风险（中性）
                    Ordering::Equal => Decimal::ZERO,
                },
                interval: returns_period,
            }
        } else {
            let excess_returns = mean_return - risk_free_return;
            // 使用最大回撤的绝对值
            let ratio = excess_returns.checked_div(max_drawdown.abs()).unwrap();
            Self {
                value: ratio,
                interval: returns_period,
            }
        }
    }

    /// 将 [`CalmarRatio`] 从当前 [`TimeInterval`] 缩放到提供的 [`TimeInterval`]。
    ///
    /// 此缩放假设收益率是独立同分布（IID）的。然而，这个假设是有争议的，
    /// 因为最大回撤可能不会像波动率那样随时间的平方根缩放。
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
    /// 返回缩放后的 CalmarRatio。
    pub fn scale<TargetInterval>(self, target: TargetInterval) -> CalmarRatio<TargetInterval>
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

        CalmarRatio {
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
    fn test_calmar_ratio_normal_case() {
        let risk_free_return = dec!(0.0015); // 0.15%
        let mean_return = dec!(0.0025); // 0.25%
        let max_drawdown = dec!(0.02); // 2%
        let time_period = Daily;

        let actual =
            CalmarRatio::calculate(risk_free_return, mean_return, max_drawdown, time_period);

        let expected = CalmarRatio {
            value: dec!(0.05), // (0.0025 - 0.0015) / 0.02
            interval: time_period,
        };

        assert_eq!(actual.value, expected.value);
        assert_eq!(actual.interval, expected.interval);
    }

    #[test]
    fn test_calmar_ratio_zero_drawdown_positive_excess() {
        let risk_free_return = dec!(0.001); // 0.1%
        let mean_return = dec!(0.002); // 0.2%
        let max_drawdown = dec!(0.0); // 0%
        let time_period = Daily;

        let actual =
            CalmarRatio::calculate(risk_free_return, mean_return, max_drawdown, time_period);

        assert_eq!(actual.value, Decimal::MAX);
        assert_eq!(actual.interval, time_period);
    }

    #[test]
    fn test_calmar_ratio_zero_drawdown_negative_excess_returns() {
        let risk_free_return = dec!(0.002); // 0.2%
        let mean_return = dec!(0.001); // 0.1%
        let max_drawdown = dec!(0.0); // 0%
        let time_period = Daily;

        let actual =
            CalmarRatio::calculate(risk_free_return, mean_return, max_drawdown, time_period);

        assert_eq!(actual.value, Decimal::MIN);
        assert_eq!(actual.interval, time_period);
    }

    #[test]
    fn test_calmar_ratio_zero_drawdown_negative_excess_via_negative_returns() {
        let risk_free_return = dec!(0.002); // 0.2%
        let mean_return = dec!(-0.001); // -0.1%
        let max_drawdown = dec!(0.0); // 0%
        let time_period = Daily;

        let actual =
            CalmarRatio::calculate(risk_free_return, mean_return, max_drawdown, time_period);

        assert_eq!(actual.value, Decimal::MIN);
        assert_eq!(actual.interval, time_period);
    }

    #[test]
    fn test_calmar_ratio_zero_drawdown_no_excess_returns() {
        let risk_free_return = dec!(0.001); // 0.1%
        let mean_return = dec!(0.001); // 0.1%
        let max_drawdown = dec!(0.0); // 0%
        let time_period = Daily;

        let actual =
            CalmarRatio::calculate(risk_free_return, mean_return, max_drawdown, time_period);

        assert_eq!(actual.value, dec!(0.0));
        assert_eq!(actual.interval, time_period);
    }

    #[test]
    fn test_calmar_ratio_negative_returns() {
        let risk_free_return = dec!(0.001); // 0.1%
        let mean_return = dec!(-0.002); // -0.2%
        let max_drawdown = dec!(0.015); // 1.5%
        let time_period = Daily;

        let actual =
            CalmarRatio::calculate(risk_free_return, mean_return, max_drawdown, time_period);

        let expected = CalmarRatio {
            value: Decimal::from_str("-0.2").unwrap(), // (-0.002 - 0.001) / 0.015
            interval: time_period,
        };

        assert_eq!(actual.value, expected.value);
        assert_eq!(actual.interval, expected.interval);
    }

    #[test]
    fn test_calmar_ratio_custom_interval() {
        let risk_free_return = dec!(0.0015); // 0.15%
        let mean_return = dec!(0.0025); // 0.25%
        let max_drawdown = dec!(0.02); // 2%
        let time_period = TimeDelta::hours(4);

        let actual =
            CalmarRatio::calculate(risk_free_return, mean_return, max_drawdown, time_period);

        let expected = CalmarRatio {
            value: dec!(0.05),
            interval: time_period,
        };

        assert_eq!(actual.value, expected.value);
        assert_eq!(actual.interval, expected.interval);
    }

    #[test]
    fn test_calmar_ratio_scale_daily_to_annual() {
        let daily = CalmarRatio {
            value: dec!(0.05),
            interval: Daily,
        };

        let actual = daily.scale(Annual252);

        // 0.05 * sqrt(252) ≈ 0.7937
        let expected = CalmarRatio {
            value: Decimal::from_str("0.7937").unwrap(),
            interval: Annual252,
        };

        let diff = (actual.value - expected.value).abs();
        assert!(diff <= Decimal::from_str("0.0001").unwrap());
        assert_eq!(actual.interval, expected.interval);
    }

    #[test]
    fn test_calmar_ratio_scale_custom_intervals() {
        let two_hour = CalmarRatio {
            value: dec!(0.05),
            interval: TimeDelta::hours(2),
        };

        let actual = two_hour.scale(TimeDelta::hours(8));

        // 0.05 * sqrt(4) = 0.05 * 2 = 0.1
        let expected = CalmarRatio {
            value: dec!(0.1),
            interval: TimeDelta::hours(8),
        };

        assert_eq!(actual.value, expected.value);
        assert_eq!(actual.interval, expected.interval);
    }

    #[test]
    fn test_calmar_ratio_extreme_values() {
        // Test with very small values
        let small = CalmarRatio::calculate(
            Decimal::from_scientific("1e-10").unwrap(),
            Decimal::from_scientific("2e-10").unwrap(),
            Decimal::from_scientific("1e-10").unwrap(),
            Daily,
        );

        assert_eq!(small.value, dec!(1.0));

        // Test with very large values
        let large = CalmarRatio::calculate(
            Decimal::from_scientific("1e10").unwrap(),
            Decimal::from_scientific("2e10").unwrap(),
            Decimal::from_scientific("1e10").unwrap(),
            Daily,
        );

        assert_eq!(large.value, dec!(1.0));
    }

    #[test]
    fn test_calmar_ratio_absolute_drawdown() {
        // Test that negative drawdown values are handled correctly (absolute value is used)
        let risk_free_return = dec!(0.001);
        let mean_return = dec!(0.002);
        let negative_drawdown = dec!(-0.015); // Should be treated same as positive 0.015
        let time_period = Daily;

        let actual = CalmarRatio::calculate(
            risk_free_return,
            mean_return,
            negative_drawdown,
            time_period,
        );

        let expected = CalmarRatio {
            value: Decimal::from_str("0.0666666666666666666666666667").unwrap(), // (0.002 - 0.001) / 0.015
            interval: time_period,
        };

        assert_eq!(actual.value, expected.value);
        assert_eq!(actual.interval, expected.interval);
    }
}
