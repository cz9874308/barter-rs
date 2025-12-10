//! Sharpe Ratio 夏普比率模块
//!
//! 本模块提供了 Sharpe Ratio（夏普比率）的计算逻辑。
//! 夏普比率是衡量投资风险调整后收益的指标，通过比较超额收益（超过无风险利率）
//! 与标准差来计算。
//!
//! # 计算公式
//!
//! `Sharpe Ratio = (平均收益率 - 无风险收益率) / 收益率标准差`
//!
//! # 参考文档
//!
//! <https://www.investopedia.com/articles/07/sharpe_ratio.asp>

use crate::statistic::time::TimeInterval;
use rust_decimal::{Decimal, MathematicalOps};
use serde::{Deserialize, Serialize};

/// 表示特定 [`TimeInterval`] 上的 Sharpe Ratio 值。
///
/// Sharpe Ratio 通过比较投资的超额收益（超过无风险利率）与其标准差来衡量
/// 投资的风险调整后收益。
///
/// ## 解释
///
/// - **高 Sharpe Ratio**: 表示在承担相同风险的情况下获得了更高的收益
/// - **低 Sharpe Ratio**: 表示风险调整后的收益较低
/// - **负 Sharpe Ratio**: 表示投资表现不如无风险资产
///
/// ## 类型参数
///
/// - `Interval`: 时间间隔类型
///
/// ## 参考文档
///
/// <https://www.investopedia.com/articles/07/sharpe_ratio.asp>
#[derive(Debug, Clone, PartialEq, PartialOrd, Default, Deserialize, Serialize)]
pub struct SharpeRatio<Interval> {
    /// Sharpe Ratio 值。
    pub value: Decimal,
    /// 时间间隔。
    pub interval: Interval,
}

impl<Interval> SharpeRatio<Interval>
where
    Interval: TimeInterval,
{
    /// 在提供的 [`TimeInterval`] 上计算 [`SharpeRatio`]。
    ///
    /// ## 计算公式
    ///
    /// `Sharpe Ratio = (平均收益率 - 无风险收益率) / 收益率标准差`
    ///
    /// ## 特殊情况
    ///
    /// 如果标准差为零（无波动），返回 `Decimal::MAX`（表示无限好的风险调整收益）。
    ///
    /// # 参数
    ///
    /// - `risk_free_return`: 无风险收益率
    /// - `mean_return`: 平均收益率
    /// - `std_dev_returns`: 收益率标准差
    /// - `returns_period`: 收益率的时间间隔
    ///
    /// # 返回值
    ///
    /// 返回计算得到的 SharpeRatio。
    pub fn calculate(
        risk_free_return: Decimal,
        mean_return: Decimal,
        std_dev_returns: Decimal,
        returns_period: Interval,
    ) -> Self {
        if std_dev_returns.is_zero() {
            // 特殊情况：无波动，返回最大值
            Self {
                value: Decimal::MAX,
                interval: returns_period,
            }
        } else {
            let excess_returns = mean_return - risk_free_return;
            let ratio = excess_returns.checked_div(std_dev_returns).unwrap();
            Self {
                value: ratio,
                interval: returns_period,
            }
        }
    }

    /// 将 [`SharpeRatio`] 从当前 [`TimeInterval`] 缩放到提供的 [`TimeInterval`]。
    ///
    /// 此缩放假设收益率是独立同分布（IID）的。
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
    /// 返回缩放后的 SharpeRatio。
    pub fn scale<TargetInterval>(self, target: TargetInterval) -> SharpeRatio<TargetInterval>
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

        SharpeRatio {
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
    fn test_sharpe_ratio_with_zero_std_dev() {
        let risk_free_return = dec!(0.001);
        let mean_return = dec!(0.002);
        let std_dev_returns = dec!(0.0);
        let time_period = TimeDelta::hours(2);

        let result =
            SharpeRatio::calculate(risk_free_return, mean_return, std_dev_returns, time_period);
        assert_eq!(result.value, Decimal::MAX);
    }

    #[test]
    fn test_sharpe_ratio_calculate_with_custom_interval() {
        // Define custom interval returns statistics
        let risk_free_return = dec!(0.0015); // 0.15%
        let mean_return = dec!(0.0025); // 0.25%
        let std_dev_returns = dec!(0.02); // 2%
        let time_period = TimeDelta::hours(2);

        let actual =
            SharpeRatio::calculate(risk_free_return, mean_return, std_dev_returns, time_period);

        let expected = SharpeRatio {
            value: dec!(0.05),
            interval: time_period,
        };

        assert_eq!(actual.value, expected.value);
        assert_eq!(actual.interval, expected.interval);
    }

    #[test]
    fn test_sharpe_ratio_calculate_with_daily_interval() {
        // Define daily returns statistics
        let risk_free_return = dec!(0.0015); // 0.15%
        let mean_return = dec!(0.0025); // 0.25%
        let std_dev_returns = dec!(0.02); // 2%
        let time_period = Daily;

        let actual =
            SharpeRatio::calculate(risk_free_return, mean_return, std_dev_returns, time_period);

        let expected = SharpeRatio {
            value: dec!(0.05),
            interval: time_period,
        };

        assert_eq!(actual.value, expected.value);
        assert_eq!(actual.interval, expected.interval);
    }

    #[test]
    fn test_sharpe_ratio_scale_from_daily_to_annual_252() {
        let input = SharpeRatio {
            value: dec!(0.05),
            interval: Daily,
        };

        let actual = input.scale(Annual252);

        let expected = SharpeRatio {
            value: dec!(0.7937253933193771771504847261),
            interval: Annual252,
        };

        assert_eq!(actual.value, expected.value);
        assert_eq!(actual.interval, expected.interval);
    }
}
