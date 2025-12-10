//! Rate Of Return 收益率模块
//!
//! 本模块提供了 Rate Of Return（收益率）的计算逻辑。
//! 收益率衡量在一段时间内价值的百分比变化。
//! 与风险调整指标不同，收益率随时间线性缩放。
//!
//! # 计算公式
//!
//! `Rate Of Return = 平均收益率`
//!
//! # 缩放特性
//!
//! 收益率使用线性缩放，而不是平方根缩放。例如，1% 的日收益率缩放为
//! 约 252% 的年收益率（而不是 √252%）。
//!
//! # 参考文档
//!
//! <https://www.investopedia.com/terms/r/rateofreturn.asp>

use crate::statistic::time::TimeInterval;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// 表示特定 [`TimeInterval`] 上的 Rate Of Return 值。
///
/// Rate Of Return 衡量在一段时间内价值的百分比变化。
/// 与风险调整指标不同，收益率随时间线性缩放。
///
/// ## 缩放特性
///
/// 收益率使用线性缩放，而不是平方根缩放。例如，1% 的日收益率缩放为
/// 约 252% 的年收益率（而不是 √252%）。
///
/// 这假设简单利息而不是复利。
///
/// ## 类型参数
///
/// - `Interval`: 时间间隔类型
///
/// ## 参考文档
///
/// <https://www.investopedia.com/terms/r/rateofreturn.asp>
#[derive(Debug, Clone, PartialEq, PartialOrd, Default, Deserialize, Serialize)]
pub struct RateOfReturn<Interval> {
    /// 收益率值。
    pub value: Decimal,
    /// 时间间隔。
    pub interval: Interval,
}

impl<Interval> RateOfReturn<Interval>
where
    Interval: TimeInterval,
{
    /// 在提供的 [`TimeInterval`] 上计算 [`RateOfReturn`]。
    ///
    /// # 参数
    ///
    /// - `mean_return`: 平均收益率
    /// - `returns_period`: 收益率的时间间隔
    ///
    /// # 返回值
    ///
    /// 返回计算得到的 RateOfReturn。
    pub fn calculate(mean_return: Decimal, returns_period: Interval) -> Self {
        Self {
            value: mean_return,
            interval: returns_period,
        }
    }

    /// 将 [`RateOfReturn`] 从当前 [`TimeInterval`] 缩放到提供的 [`TimeInterval`]。
    ///
    /// 与使用平方根缩放的风险指标不同，[`RateOfReturn`] 随时间线性缩放。
    ///
    /// 例如，1% 的日收益率缩放为约 252% 的年收益率（而不是 √252%）。
    ///
    /// 这假设简单利息而不是复利。
    ///
    /// ## 缩放公式
    ///
    /// `scaled_value = value * (target_interval / current_interval)`
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
    /// 返回缩放后的 RateOfReturn。
    pub fn scale<TargetInterval>(self, target: TargetInterval) -> RateOfReturn<TargetInterval>
    where
        TargetInterval: TimeInterval,
    {
        // 确定缩放因子：目标间隔与当前间隔的线性比例
        let target_secs = Decimal::from(target.interval().num_seconds());
        let current_secs = Decimal::from(self.interval.interval().num_seconds());

        let scale = target_secs
            .abs()
            .checked_div(current_secs.abs())
            .unwrap_or(Decimal::MAX);

        RateOfReturn {
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

    #[test]
    fn test_rate_of_return_normal_case() {
        let mean_return = dec!(0.0025); // 0.25%
        let time_period = Daily;

        let actual = RateOfReturn::calculate(mean_return, time_period);

        let expected = RateOfReturn {
            value: dec!(0.0025),
            interval: time_period,
        };

        assert_eq!(actual.value, expected.value);
        assert_eq!(actual.interval, expected.interval);
    }

    #[test]
    fn test_rate_of_return_zero() {
        let mean_return = dec!(0.0);
        let time_period = Daily;

        let actual = RateOfReturn::calculate(mean_return, time_period);

        assert_eq!(actual.value, dec!(0.0));
        assert_eq!(actual.interval, time_period);
    }

    #[test]
    fn test_rate_of_return_negative() {
        let mean_return = dec!(-0.0025); // -0.25%
        let time_period = Daily;

        let actual = RateOfReturn::calculate(mean_return, time_period);

        let expected = RateOfReturn {
            value: dec!(-0.0025),
            interval: time_period,
        };

        assert_eq!(actual.value, expected.value);
        assert_eq!(actual.interval, expected.interval);
    }

    #[test]
    fn test_rate_of_return_custom_interval() {
        let mean_return = dec!(0.0025); // 0.25%
        let time_period = TimeDelta::hours(4);

        let actual = RateOfReturn::calculate(mean_return, time_period);

        let expected = RateOfReturn {
            value: dec!(0.0025),
            interval: time_period,
        };

        assert_eq!(actual.value, expected.value);
        assert_eq!(actual.interval, expected.interval);
    }

    #[test]
    fn test_rate_of_return_scale_daily_to_annual() {
        // For returns, we use linear scaling (multiply by 252) not square root scaling
        let daily = RateOfReturn {
            value: dec!(0.01), // 1% daily return
            interval: Daily,
        };

        let actual = daily.scale(Annual252);

        let expected = RateOfReturn {
            value: dec!(2.52), // Should be 252% annual return
            interval: Annual252,
        };

        assert_eq!(actual.value, expected.value);
        assert_eq!(actual.interval, expected.interval);
    }

    #[test]
    fn test_rate_of_return_scale_custom_intervals() {
        // Test scaling from 2 hours to 8 hours (linear scaling factor of 4)
        let two_hour = RateOfReturn {
            value: dec!(0.01), // 1% per 2 hours
            interval: TimeDelta::hours(2),
        };

        let actual = two_hour.scale(TimeDelta::hours(8));

        let expected = RateOfReturn {
            value: dec!(0.04), // Should be 4% per 8 hours
            interval: TimeDelta::hours(8),
        };

        assert_eq!(actual.value, expected.value);
        assert_eq!(actual.interval, expected.interval);
    }

    #[test]
    fn test_rate_of_return_scale_zero() {
        // Zero returns should remain zero when scaled
        let daily = RateOfReturn {
            value: dec!(0.0),
            interval: Daily,
        };

        let actual = daily.scale(Annual252);

        assert_eq!(actual.value, dec!(0.0));
        assert_eq!(actual.interval, Annual252);
    }

    #[test]
    fn test_rate_of_return_scale_negative() {
        // Negative returns should scale linearly while maintaining sign
        let daily = RateOfReturn {
            value: dec!(-0.01), // -1% daily return
            interval: Daily,
        };

        let actual = daily.scale(Annual252);

        let expected = RateOfReturn {
            value: dec!(-2.52), // Should be -252% annual return
            interval: Annual252,
        };

        assert_eq!(actual.value, expected.value);
        assert_eq!(actual.interval, expected.interval);
    }

    #[test]
    fn test_rate_of_return_extreme_values() {
        // Test with very small values
        let small = RateOfReturn::calculate(dec!(1e-10), Daily);
        let small_annual = small.scale(Annual252);
        assert_eq!(small_annual.value, dec!(252e-10));

        // Test with very large values
        let large = RateOfReturn::calculate(dec!(1e10), Daily);
        let large_annual = large.scale(Annual252);
        assert_eq!(large_annual.value, dec!(252e10));
    }
}
