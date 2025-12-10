//! Drawdown 回撤模块
//!
//! 本模块提供了 Drawdown（回撤）的计算逻辑。
//! 回撤是在特定时期内从峰值到谷值的价值下降，是衡量下行波动率的指标。
//!
//! # 核心概念
//!
//! - **Drawdown**: 回撤值，包含回撤幅度和时间范围
//! - **DrawdownGenerator**: 回撤生成器，用于跟踪和计算回撤
//! - **Max Drawdown**: 最大回撤
//! - **Mean Drawdown**: 平均回撤
//!
//! # 使用场景
//!
//! - 投资组合 PnL
//! - 策略 PnL
//! - 交易对 PnL
//! - 资产权益
//!
//! # 参考文档
//!
//! <https://www.investopedia.com/terms/d/drawdown.asp>

use crate::Timed;
use chrono::{DateTime, TimeDelta, Utc};
use derive_more::Constructor;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// 最大回撤计算逻辑。
pub mod max;

/// 平均回撤计算逻辑。
pub mod mean;

/// [`Drawdown`] 是在特定时期内从峰值到谷值的价值下降。回撤是衡量下行波动率的指标。
///
/// Drawdown 表示从峰值到谷值的百分比下降。它用于评估投资或策略的风险。
///
/// ## 使用场景示例
///
/// - 投资组合 PnL
/// - 策略 PnL
/// - 交易对 PnL
/// - 资产权益
///
/// ## 字段说明
///
/// - **value**: 回撤值（百分比，例如 0.2 表示 20% 的回撤）
/// - **time_start**: 回撤开始时间（峰值时间）
/// - **time_end**: 回撤结束时间（恢复到峰值的时间）
///
/// ## 参考文档
///
/// <https://www.investopedia.com/terms/d/drawdown.asp>
#[derive(Debug, Clone, PartialEq, PartialOrd, Default, Deserialize, Serialize, Constructor)]
pub struct Drawdown {
    /// 回撤值（百分比，例如 0.2 表示 20% 的回撤）。
    pub value: Decimal,
    /// 回撤开始时间（峰值时间）。
    pub time_start: DateTime<Utc>,
    /// 回撤结束时间（恢复到峰值的时间）。
    pub time_end: DateTime<Utc>,
}

impl Drawdown {
    /// 回撤的时间周期。
    ///
    /// # 返回值
    ///
    /// 返回从回撤开始到结束的时间差。
    pub fn duration(&self) -> TimeDelta {
        self.time_end.signed_duration_since(self.time_start)
    }
}

/// [`Drawdown`] 生成器。
///
/// DrawdownGenerator 用于跟踪价值序列并计算回撤。它维护峰值、当前最大回撤
/// 和时间信息，并在回撤期结束时生成 Drawdown 实例。
///
/// ## 工作原理
///
/// 1. 跟踪当前峰值
/// 2. 当价值低于峰值时，计算当前回撤
/// 3. 更新最大回撤
/// 4. 当价值恢复到峰值以上时，生成回撤并重置
///
/// ## 字段说明
///
/// - **peak**: 当前峰值（可选）
/// - **drawdown_max**: 当前回撤期内的最大回撤
/// - **time_peak**: 峰值时间（可选）
/// - **time_now**: 当前时间
///
/// ## 参考文档
///
/// <https://www.investopedia.com/terms/d/drawdown.asp>
#[derive(Debug, Clone, PartialEq, PartialOrd, Default, Deserialize, Serialize, Constructor)]
pub struct DrawdownGenerator {
    /// 当前峰值。
    pub peak: Option<Decimal>,
    /// 当前回撤期内的最大回撤。
    pub drawdown_max: Decimal,
    /// 峰值时间。
    pub time_peak: Option<DateTime<Utc>>,
    /// 当前时间。
    pub time_now: DateTime<Utc>,
}

impl DrawdownGenerator {
    /// 从初始 [`Timed`] 值初始化 [`DrawdownGenerator`]。
    ///
    /// # 参数
    ///
    /// - `point`: 初始时间戳值
    ///
    /// # 返回值
    ///
    /// 返回新创建的 DrawdownGenerator 实例。
    pub fn init(point: Timed<Decimal>) -> Self {
        Self {
            peak: Some(point.value),
            drawdown_max: Decimal::ZERO,
            time_peak: Some(point.time),
            time_now: point.time,
        }
    }

    /// 使用最新的 [`Timed`] 值更新内部 [`DrawdownGenerator`] 状态。
    ///
    /// 如果回撤期已结束（即投资从谷值恢复到之前的峰值以上），
    /// 函数返回 `Some(Drawdown)`，否则返回 `None`。
    ///
    /// ## 工作流程
    ///
    /// 1. 更新当前时间
    /// 2. 如果价值超过峰值：生成回撤（如果有）并重置
    /// 3. 如果价值低于峰值：计算当前回撤并更新最大回撤
    ///
    /// # 参数
    ///
    /// - `point`: 最新的时间戳值
    ///
    /// # 返回值
    ///
    /// 如果回撤期结束，返回 `Some(Drawdown)`；否则返回 `None`。
    pub fn update(&mut self, point: Timed<Decimal>) -> Option<Drawdown> {
        self.time_now = point.time;

        // 处理第一个值的情况
        let Some(peak) = self.peak else {
            self.peak = Some(point.value);
            self.time_peak = Some(point.time);
            return None;
        };

        if point.value > peak {
            // 只有当实际发生回撤时才生成 Drawdown
            // 例如，如果我们一直在增加峰值，则不应该生成回撤
            let ended_drawdown = self.generate();

            // 重置参数（即使没有生成回撤，因为我们有了新的峰值）
            self.peak = Some(point.value);
            self.time_peak = Some(point.time);
            self.drawdown_max = Decimal::ZERO;

            ended_drawdown
        } else {
            // 计算当前时刻的回撤
            let drawdown_current = (peak - point.value).checked_div(peak);

            if let Some(drawdown_current) = drawdown_current {
                // 如果当前回撤更大，则替换"期间最大回撤"
                if drawdown_current > self.drawdown_max {
                    self.drawdown_max = drawdown_current;
                }
            }

            None
        }
    }

    /// 在当前时刻生成 [`Drawdown`]（如果非零）。
    ///
    /// 此方法生成当前回撤期的 Drawdown 实例，包含最大回撤值和时间范围。
    ///
    /// # 返回值
    ///
    /// 如果存在非零回撤，返回 `Some(Drawdown)`；否则返回 `None`。
    pub fn generate(&mut self) -> Option<Drawdown> {
        let time_peak = self.time_peak?;

        (self.drawdown_max != Decimal::ZERO).then_some(Drawdown {
            value: self.drawdown_max,
            time_start: time_peak,
            time_end: self.time_now,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::time_plus_days;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use std::str::FromStr;

    #[test]
    fn test_drawdown_generate_update() {
        struct TestCase {
            input: Timed<Decimal>,
            expected_state: DrawdownGenerator,
            expected_output: Option<Drawdown>,
        }

        let time_base = DateTime::<Utc>::MIN_UTC;

        let mut generator = DrawdownGenerator::default();

        let cases = vec![
            // TC0: first ever balance update
            TestCase {
                input: Timed::new(dec!(100.0), time_base),
                expected_state: DrawdownGenerator {
                    peak: Some(dec!(100.0)),
                    drawdown_max: dec!(0.0),
                    time_peak: Some(time_base),
                    time_now: time_base,
                },
                expected_output: None,
            },
            // TC1: peak increases from initial value w/ no drawdown
            TestCase {
                input: Timed::new(dec!(110.0), time_plus_days(time_base, 1)),
                expected_state: DrawdownGenerator {
                    peak: Some(dec!(110.0)),
                    drawdown_max: dec!(0.0),
                    time_peak: Some(time_plus_days(time_base, 1)),
                    time_now: time_plus_days(time_base, 1),
                },
                expected_output: None,
            },
            // TC2: first drawdown occurs
            TestCase {
                input: Timed::new(dec!(99.0), time_plus_days(time_base, 2)),
                expected_state: DrawdownGenerator {
                    peak: Some(dec!(110.0)),
                    drawdown_max: dec!(0.1), // (110-99)/110
                    time_peak: Some(time_plus_days(time_base, 1)),
                    time_now: time_plus_days(time_base, 2),
                },
                expected_output: None,
            },
            // TC3: drawdown increases
            TestCase {
                input: Timed::new(dec!(88.0), time_plus_days(time_base, 3)),
                expected_state: DrawdownGenerator {
                    peak: Some(dec!(110.0)),
                    drawdown_max: dec!(0.2), // (110-88)/110
                    time_peak: Some(time_plus_days(time_base, 1)),
                    time_now: time_plus_days(time_base, 3),
                },
                expected_output: None,
            },
            // TC4: partial recovery (still in drawdown)
            TestCase {
                input: Timed::new(dec!(95.0), time_plus_days(time_base, 4)),
                expected_state: DrawdownGenerator {
                    peak: Some(dec!(110.0)),
                    drawdown_max: dec!(0.2), // max drawdown unchanged
                    time_peak: Some(time_plus_days(time_base, 1)),
                    time_now: time_plus_days(time_base, 4),
                },
                expected_output: None,
            },
            // TC5: full recovery above previous peak - should emit drawdown
            TestCase {
                input: Timed::new(dec!(115.0), time_plus_days(time_base, 5)),
                expected_state: DrawdownGenerator {
                    peak: Some(dec!(115.0)),
                    drawdown_max: dec!(0.0), // reset for new period
                    time_peak: Some(time_plus_days(time_base, 5)),
                    time_now: time_plus_days(time_base, 5),
                },
                expected_output: Some(Drawdown {
                    value: dec!(0.2), // maximum drawdown from previous period
                    time_start: time_plus_days(time_base, 1),
                    time_end: time_plus_days(time_base, 5),
                }),
            },
            // TC6: equal to previous peak (shouldn't trigger new period)
            TestCase {
                input: Timed::new(dec!(115.0), time_plus_days(time_base, 6)),
                expected_state: DrawdownGenerator {
                    peak: Some(dec!(115.0)),
                    drawdown_max: dec!(0.0),
                    time_peak: Some(time_plus_days(time_base, 5)),
                    time_now: time_plus_days(time_base, 6),
                },
                expected_output: None,
            },
            // TC7: tiny drawdown (testing decimal precision)
            TestCase {
                input: Timed::new(
                    Decimal::from_str("114.99999").unwrap(),
                    time_plus_days(time_base, 7),
                ),
                expected_state: DrawdownGenerator {
                    peak: Some(dec!(115.0)),
                    drawdown_max: Decimal::from_str("0.0000000869565217391304347826").unwrap(), // (115-114.99999)/115
                    time_peak: Some(time_plus_days(time_base, 5)),
                    time_now: time_plus_days(time_base, 7),
                },
                expected_output: None,
            },
            // TC8: large peak jump after drawdown
            TestCase {
                input: Timed::new(dec!(200.0), time_plus_days(time_base, 8)),
                expected_state: DrawdownGenerator {
                    peak: Some(dec!(200.0)),
                    drawdown_max: dec!(0.0),
                    time_peak: Some(time_plus_days(time_base, 8)),
                    time_now: time_plus_days(time_base, 8),
                },
                expected_output: Some(Drawdown {
                    value: Decimal::from_str("0.0000000869565217391304347826").unwrap(), // maximum drawdown from previous period
                    time_start: time_plus_days(time_base, 5),
                    time_end: time_plus_days(time_base, 8),
                }),
            },
        ];

        for (index, test) in cases.into_iter().enumerate() {
            let output = generator.update(test.input);
            assert_eq!(generator, test.expected_state, "TC{index} failed");
            assert_eq!(output, test.expected_output, "TC{index} failed");
        }
    }
}
