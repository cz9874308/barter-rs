//! Max Drawdown 最大回撤模块
//!
//! 本模块提供了 Max Drawdown（最大回撤）的计算逻辑。
//! 最大回撤是 PnL（投资组合、策略、交易对）或资产余额的最大峰值到谷值下降。
//!
//! # 核心概念
//!
//! - **MaxDrawdown**: 最大回撤值，包装了 Drawdown
//! - **MaxDrawdownGenerator**: 最大回撤生成器，跟踪所有回撤并找出最大值
//!
//! # 参考文档
//!
//! <https://www.investopedia.com/terms/m/maximum-drawdown-mdd.asp>

use crate::statistic::metric::drawdown::Drawdown;
use derive_more::Constructor;
use serde::{Deserialize, Serialize};

/// [`MaxDrawdown`] 是 PnL（投资组合、策略、交易对）或资产余额的最大峰值到谷值下降。
///
/// 最大回撤是衡量下行风险的指标，较大的值表示下行波动可能较大。
///
/// ## 解释
///
/// - **较大的 Max Drawdown**: 表示策略可能经历较大的价值下降
/// - **较小的 Max Drawdown**: 表示策略相对稳定
///
/// ## 参考文档
///
/// <https://www.investopedia.com/terms/m/maximum-drawdown-mdd.asp>
#[derive(Debug, Clone, PartialEq, PartialOrd, Default, Deserialize, Serialize, Constructor)]
pub struct MaxDrawdown(pub Drawdown);

/// [`MaxDrawdown`] 生成器。
///
/// MaxDrawdownGenerator 跟踪所有回撤并维护最大回撤值。
/// 当新的回撤大于当前最大回撤时，会替换它。
#[derive(Debug, Clone, PartialEq, PartialOrd, Default, Deserialize, Serialize, Constructor)]
pub struct MaxDrawdownGenerator {
    /// 当前最大回撤（可选）。
    pub max: Option<MaxDrawdown>,
}

impl MaxDrawdownGenerator {
    /// 从初始 [`Drawdown`] 初始化 [`MaxDrawdownGenerator`]。
    ///
    /// # 参数
    ///
    /// - `drawdown`: 初始回撤
    ///
    /// # 返回值
    ///
    /// 返回新创建的 MaxDrawdownGenerator 实例。
    pub fn init(drawdown: Drawdown) -> Self {
        Self {
            max: Some(MaxDrawdown(drawdown)),
        }
    }

    /// 使用最新的下一个 [`Drawdown`] 更新内部 [`MaxDrawdown`]。
    ///
    /// 如果下一个回撤大于当前的 [`MaxDrawdown`]，则替换它。
    ///
    /// # 参数
    ///
    /// - `next_drawdown`: 下一个回撤
    pub fn update(&mut self, next_drawdown: &Drawdown) {
        let max = match self.max.take() {
            Some(current) => {
                // 比较回撤的绝对值
                if next_drawdown.value.abs() > current.0.value.abs() {
                    MaxDrawdown(next_drawdown.clone())
                } else {
                    current
                }
            }
            None => MaxDrawdown(next_drawdown.clone()),
        };

        self.max = Some(max);
    }

    /// 生成当前的 [`MaxDrawdown`]（如果存在）。
    ///
    /// # 返回值
    ///
    /// 返回当前最大回撤，如果不存在则返回 `None`。
    pub fn generate(&self) -> Option<MaxDrawdown> {
        self.max.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::time_plus_days;
    use chrono::{DateTime, Utc};
    use rust_decimal_macros::dec;

    #[test]
    fn test_max_drawdown_generator_update() {
        struct TestCase {
            input: Drawdown,
            expected_state: MaxDrawdownGenerator,
            expected_output: Option<MaxDrawdown>,
        }

        let base_time = DateTime::<Utc>::MIN_UTC;

        let mut generator = MaxDrawdownGenerator::default();

        let cases = vec![
            // TC0: first ever drawdown
            TestCase {
                input: Drawdown {
                    value: dec!(-0.227272727272727273), // -25/110
                    time_start: base_time,
                    time_end: time_plus_days(base_time, 2),
                },
                expected_state: MaxDrawdownGenerator {
                    max: Some(MaxDrawdown::new(Drawdown {
                        value: dec!(-0.227272727272727273),
                        time_start: base_time,
                        time_end: time_plus_days(base_time, 2),
                    })),
                },
                expected_output: Some(MaxDrawdown::new(Drawdown {
                    value: dec!(-0.227272727272727273),
                    time_start: base_time,
                    time_end: time_plus_days(base_time, 2),
                })),
            },
            // TC1: larger drawdown
            TestCase {
                input: Drawdown {
                    value: dec!(-0.55), // -110/200
                    time_start: base_time,
                    time_end: time_plus_days(base_time, 3),
                },
                expected_state: MaxDrawdownGenerator {
                    max: Some(MaxDrawdown::new(Drawdown {
                        value: dec!(-0.55),
                        time_start: base_time,
                        time_end: time_plus_days(base_time, 3),
                    })),
                },
                expected_output: Some(MaxDrawdown::new(Drawdown {
                    value: dec!(-0.55),
                    time_start: base_time,
                    time_end: time_plus_days(base_time, 3),
                })),
            },
            // TC2: smaller drawdown
            TestCase {
                input: Drawdown {
                    value: dec!(-0.033333333333333333), // -10/300
                    time_start: base_time,
                    time_end: time_plus_days(base_time, 3),
                },
                expected_state: MaxDrawdownGenerator {
                    max: Some(MaxDrawdown::new(Drawdown {
                        value: dec!(-0.55),
                        time_start: base_time,
                        time_end: time_plus_days(base_time, 3),
                    })),
                },
                expected_output: Some(MaxDrawdown::new(Drawdown {
                    value: dec!(-0.55),
                    time_start: base_time,
                    time_end: time_plus_days(base_time, 3),
                })),
            },
            // TC3: largest drawdown
            TestCase {
                input: Drawdown {
                    value: dec!(-0.99999), // -9999.9/10000.0
                    time_start: base_time,
                    time_end: time_plus_days(base_time, 3),
                },
                expected_state: MaxDrawdownGenerator {
                    max: Some(MaxDrawdown::new(Drawdown {
                        value: dec!(-0.99999),
                        time_start: base_time,
                        time_end: time_plus_days(base_time, 3),
                    })),
                },
                expected_output: Some(MaxDrawdown::new(Drawdown {
                    value: dec!(-0.99999),
                    time_start: base_time,
                    time_end: time_plus_days(base_time, 3),
                })),
            },
        ];

        for (index, test) in cases.into_iter().enumerate() {
            generator.update(&test.input);

            // Verify both internal state and generated output
            assert_eq!(
                generator, test.expected_state,
                "TC{index} generator state failed"
            );
            assert_eq!(
                generator.generate(),
                test.expected_output,
                "TC{index} generated output failed"
            );
        }
    }
}
