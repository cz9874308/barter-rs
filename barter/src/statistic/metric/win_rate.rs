//! Win Rate 胜率模块
//!
//! 本模块提供了 Win Rate（胜率）的计算逻辑。
//! 胜率是盈利交易数量与总交易数量的比率，范围在 0 到 1 之间。
//!
//! # 计算公式
//!
//! `Win Rate = 盈利交易数 / 总交易数`
//!
//! # 解释
//!
//! - **Win Rate = 1.0**: 所有交易都盈利（100% 胜率）
//! - **Win Rate = 0.5**: 一半交易盈利（50% 胜率）
//! - **Win Rate = 0.0**: 没有交易盈利（0% 胜率）
//!
//! # 参考文档
//!
//! <https://www.investopedia.com/terms/w/win-loss-ratio.asp>

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// 表示 0 到 1 之间的胜率比率，计算公式为 `wins/total`。
///
/// 胜率计算为盈利交易数量与总交易数量的绝对比率。
///
/// 如果没有交易（total = 0）或除法运算溢出，返回 `None`。
///
/// ## 解释
///
/// - **Win Rate = 1.0**: 所有交易都盈利（100% 胜率）
/// - **Win Rate = 0.5**: 一半交易盈利（50% 胜率）
/// - **Win Rate = 0.0**: 没有交易盈利（0% 胜率）
///
/// ## 参考文档
///
/// <https://www.investopedia.com/terms/w/win-loss-ratio.asp>
#[derive(Debug, Clone, PartialEq, PartialOrd, Default, Deserialize, Serialize)]
pub struct WinRate {
    /// 胜率值（0 到 1 之间）。
    pub value: Decimal,
}

impl WinRate {
    /// 根据提供的盈利交易数和总交易数计算 [`WinRate`]。
    ///
    /// ## 计算公式
    ///
    /// `Win Rate = |盈利交易数| / |总交易数|`
    ///
    /// ## 特殊情况
    ///
    /// 如果总交易数为零（没有交易），返回 `None`。
    ///
    /// # 参数
    ///
    /// - `wins`: 盈利交易数量（绝对值会被使用）
    /// - `total`: 总交易数量（绝对值会被使用）
    ///
    /// # 返回值
    ///
    /// 返回计算得到的 WinRate，如果没有交易或除法溢出则返回 `None`。
    pub fn calculate(wins: Decimal, total: Decimal) -> Option<Self> {
        if total == Decimal::ZERO {
            None
        } else {
            let value = wins.abs().checked_div(total.abs())?;
            Some(Self { value })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_win_rate_calculate() {
        // no trades
        assert_eq!(WinRate::calculate(Decimal::ZERO, Decimal::ZERO), None);

        // all winning trades
        assert_eq!(
            WinRate::calculate(Decimal::TEN, Decimal::TEN)
                .unwrap()
                .value,
            Decimal::ONE
        );

        // no winning trades
        assert_eq!(
            WinRate::calculate(Decimal::ZERO, Decimal::TEN)
                .unwrap()
                .value,
            Decimal::ZERO
        );

        // mixed winning and losing trades
        assert_eq!(
            WinRate::calculate(dec!(6), Decimal::TEN).unwrap().value,
            dec!(0.6)
        );
    }
}
