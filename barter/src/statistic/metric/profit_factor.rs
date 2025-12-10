//! Profit Factor 盈利因子模块
//!
//! 本模块提供了 Profit Factor（盈利因子）的计算逻辑。
//! 盈利因子是衡量策略绩效的指标，将总利润的绝对值除以总亏损的绝对值。
//!
//! # 计算公式
//!
//! `Profit Factor = |总利润| / |总亏损|`
//!
//! # 解释
//!
//! - **Profit Factor > 1**: 表示策略盈利
//! - **Profit Factor = 1**: 表示盈亏平衡
//! - **Profit Factor < 1**: 表示策略亏损
//!
//! # 参考文档
//!
//! <https://www.investopedia.com/articles/fundamental-analysis/10/strategy-performance-reports.asp#toc-profit-factor>

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// ProfitFactor 是一种绩效指标，将总利润的绝对值除以总亏损的绝对值。
///
/// Profit Factor 大于 1 表示盈利策略。
///
/// ## 特殊情况
///
/// - 如果利润和亏损都为零：返回 `None`（无法计算）
/// - 如果有利润但没有亏损：返回 `Decimal::MAX`（完美表现）
/// - 如果有亏损但没有利润：返回 `Decimal::MIN`（最差表现）
///
/// ## 解释
///
/// - **Profit Factor > 1**: 策略盈利
/// - **Profit Factor = 1**: 盈亏平衡
/// - **Profit Factor < 1**: 策略亏损
///
/// ## 参考文档
///
/// <https://www.investopedia.com/articles/fundamental-analysis/10/strategy-performance-reports.asp#toc-profit-factor>
#[derive(Debug, Clone, PartialEq, PartialOrd, Default, Deserialize, Serialize)]
pub struct ProfitFactor {
    /// Profit Factor 值。
    pub value: Decimal,
}

impl ProfitFactor {
    /// 根据提供的总利润和总亏损计算 [`ProfitFactor`]。
    ///
    /// ## 计算公式
    ///
    /// `Profit Factor = |总利润| / |总亏损|`
    ///
    /// ## 特殊情况处理
    ///
    /// - 如果利润和亏损都为零：返回 `None`
    /// - 如果亏损为零（有利润但无亏损）：返回 `Decimal::MAX`
    /// - 如果利润为零（有亏损但无利润）：返回 `Decimal::MIN`
    ///
    /// # 参数
    ///
    /// - `profits_gross_abs`: 总利润（绝对值会被使用）
    /// - `losses_gross_abs`: 总亏损（绝对值会被使用）
    ///
    /// # 返回值
    ///
    /// 返回计算得到的 ProfitFactor，如果无法计算则返回 `None`。
    pub fn calculate(profits_gross_abs: Decimal, losses_gross_abs: Decimal) -> Option<Self> {
        if profits_gross_abs.is_zero() && losses_gross_abs.is_zero() {
            return None;
        }

        let value = if losses_gross_abs.is_zero() {
            // 有利润但无亏损（完美表现）
            Decimal::MAX
        } else if profits_gross_abs.is_zero() {
            // 有亏损但无利润（最差表现）
            Decimal::MIN
        } else {
            // 正常情况：利润 / 亏损
            profits_gross_abs
                .abs()
                .checked_div(losses_gross_abs.abs())?
        };

        Some(Self { value })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::str::FromStr;

    #[test]
    fn test_profit_factor_calculate() {
        // both profits & losses are very small
        assert_eq!(
            ProfitFactor::calculate(
                Decimal::from_scientific("1e-20").unwrap(),
                Decimal::from_scientific("1e-20").unwrap()
            )
            .unwrap()
            .value,
            Decimal::ONE
        );

        // both profits & losses are very large
        assert_eq!(
            ProfitFactor::calculate(Decimal::MAX / dec!(2), Decimal::MAX / dec!(2))
                .unwrap()
                .value,
            Decimal::ONE
        );

        // both profits & losses are zero
        assert_eq!(ProfitFactor::calculate(dec!(0.0), dec!(0.0)), None);

        // profits are zero
        assert_eq!(
            ProfitFactor::calculate(dec!(0.0), dec!(1.0)).unwrap().value,
            Decimal::MIN
        );

        // losses are zero
        assert_eq!(
            ProfitFactor::calculate(dec!(1.0), dec!(0.0)).unwrap().value,
            Decimal::MAX
        );

        // both profits & losses are non-zero
        assert_eq!(
            ProfitFactor::calculate(dec!(10.0), dec!(5.0))
                .unwrap()
                .value,
            dec!(2.0)
        );

        // both profits & losses are non-zero, but input losses are not abs
        assert_eq!(
            ProfitFactor::calculate(dec!(10.0), dec!(-5.0))
                .unwrap()
                .value,
            dec!(2.0)
        );

        // test with precise decimal values
        assert_eq!(
            ProfitFactor::calculate(dec!(10.5555), dec!(5.2345))
                .unwrap()
                .value,
            Decimal::from_str("2.016524978507975928933040405").unwrap()
        );
    }
}
