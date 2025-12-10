//! Mean Drawdown 平均回撤模块
//!
//! 本模块提供了 Mean Drawdown（平均回撤）的计算逻辑。
//! 平均回撤是从一组 [`Drawdown`] 中计算的平均回撤值和毫秒持续时间。
//!
//! # 核心概念
//!
//! - **MeanDrawdown**: 平均回撤值，包含平均回撤幅度和平均持续时间
//! - **MeanDrawdownGenerator**: 平均回撤生成器，使用 Welford Online 算法增量计算平均值

use crate::statistic::{algorithm::welford_online, metric::drawdown::Drawdown};
use derive_more::Constructor;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// [`MeanDrawdown`] 定义为从一组 [`Drawdown`] 中计算的平均回撤值和毫秒持续时间。
///
/// MeanDrawdown 包含两个平均值：
/// - 平均回撤幅度（百分比）
/// - 平均回撤持续时间（毫秒）
///
/// ## 字段说明
///
/// - **mean_drawdown**: 平均回撤值（百分比）
/// - **mean_drawdown_ms**: 平均回撤持续时间（毫秒）
#[derive(Debug, Clone, PartialEq, PartialOrd, Default, Deserialize, Serialize, Constructor)]
pub struct MeanDrawdown {
    /// 平均回撤值（百分比）。
    pub mean_drawdown: Decimal,
    /// 平均回撤持续时间（毫秒）。
    pub mean_drawdown_ms: i64,
}

/// [`MeanDrawdown`] 生成器。
///
/// MeanDrawdownGenerator 使用 Welford Online 算法增量计算平均回撤值。
/// 它可以在单次遍历数据时计算平均值，不需要存储所有回撤值。
#[derive(Debug, Clone, PartialEq, PartialOrd, Default, Deserialize, Serialize, Constructor)]
pub struct MeanDrawdownGenerator {
    /// 回撤计数。
    pub count: u64,
    /// 当前平均回撤（可选）。
    pub mean_drawdown: Option<MeanDrawdown>,
}

impl MeanDrawdownGenerator {
    /// 从初始 [`Drawdown`] 初始化 [`MeanDrawdownGenerator`]。
    ///
    /// # 参数
    ///
    /// - `drawdown`: 初始回撤
    ///
    /// # 返回值
    ///
    /// 返回新创建的 MeanDrawdownGenerator 实例。
    pub fn init(drawdown: Drawdown) -> Self {
        Self {
            count: 1,
            mean_drawdown: Some(MeanDrawdown {
                mean_drawdown: drawdown.value,
                mean_drawdown_ms: drawdown.duration().num_milliseconds(),
            }),
        }
    }

    /// 使用提供的下一个 [`Drawdown`] 更新平均回撤和平均回撤持续时间。
    ///
    /// 此方法使用 Welford Online 算法增量更新平均值，不需要存储所有回撤值。
    ///
    /// # 参数
    ///
    /// - `next_drawdown`: 下一个回撤
    pub fn update(&mut self, next_drawdown: &Drawdown) {
        self.count += 1;

        let mean_drawdown = match self.mean_drawdown.take() {
            Some(MeanDrawdown {
                mean_drawdown,
                mean_drawdown_ms,
            }) => MeanDrawdown {
                // 使用 Welford Online 算法更新平均回撤值
                mean_drawdown: welford_online::calculate_mean(
                    mean_drawdown,
                    next_drawdown.value,
                    Decimal::from(self.count),
                ),
                // 使用 Welford Online 算法更新平均持续时间
                mean_drawdown_ms: welford_online::calculate_mean(
                    mean_drawdown_ms,
                    next_drawdown.duration().num_milliseconds(),
                    self.count as i64,
                ),
            },
            None => MeanDrawdown {
                mean_drawdown: next_drawdown.value,
                mean_drawdown_ms: next_drawdown.duration().num_milliseconds(),
            },
        };

        self.mean_drawdown = Some(mean_drawdown)
    }

    /// 生成当前的 [`MeanDrawdown`]（如果存在）。
    ///
    /// # 返回值
    ///
    /// 返回当前平均回撤，如果不存在则返回 `None`。
    pub fn generate(&self) -> Option<MeanDrawdown> {
        self.mean_drawdown.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::time_plus_days;
    use chrono::{DateTime, TimeDelta, Utc};
    use rust_decimal_macros::dec;

    #[test]
    fn test_mean_drawdown_generator_update() {
        struct TestCase {
            input: Drawdown,
            expected_state: MeanDrawdownGenerator,
            expected_output: Option<MeanDrawdown>,
        }

        let base_time = DateTime::<Utc>::MIN_UTC;

        let mut generator = MeanDrawdownGenerator::default();

        let cases = vec![
            // TC0: first ever drawdown
            TestCase {
                input: Drawdown {
                    value: dec!(-0.5), // -50/100
                    time_start: base_time,
                    time_end: time_plus_days(base_time, 2),
                },
                expected_state: MeanDrawdownGenerator {
                    count: 1,
                    mean_drawdown: Some(MeanDrawdown {
                        mean_drawdown: dec!(-0.5),
                        mean_drawdown_ms: TimeDelta::days(2).num_milliseconds(),
                    }),
                },
                expected_output: Some(MeanDrawdown {
                    mean_drawdown: dec!(-0.5),
                    mean_drawdown_ms: TimeDelta::days(2).num_milliseconds(),
                }),
            },
            // TC1: second drawdown updates mean
            TestCase {
                input: Drawdown {
                    value: dec!(-0.5), // -100/200
                    time_start: base_time,
                    time_end: time_plus_days(base_time, 2),
                },
                expected_state: MeanDrawdownGenerator {
                    count: 2,
                    mean_drawdown: Some(MeanDrawdown {
                        mean_drawdown: dec!(-0.5),
                        mean_drawdown_ms: TimeDelta::days(2).num_milliseconds(),
                    }),
                },
                expected_output: Some(MeanDrawdown {
                    mean_drawdown: dec!(-0.5),
                    mean_drawdown_ms: TimeDelta::days(2).num_milliseconds(),
                }),
            },
            // TC2: third drawdown with different duration
            TestCase {
                input: Drawdown {
                    value: dec!(-0.18), // -180/1000
                    time_start: base_time,
                    time_end: time_plus_days(base_time, 5),
                },
                expected_state: MeanDrawdownGenerator {
                    count: 3,
                    mean_drawdown: Some(MeanDrawdown {
                        mean_drawdown: dec!(-0.3933333333333333333333333333), // -59/150
                        mean_drawdown_ms: TimeDelta::days(3).num_milliseconds(),
                    }),
                },
                expected_output: Some(MeanDrawdown {
                    mean_drawdown: dec!(-0.3933333333333333333333333333), // -59/150
                    mean_drawdown_ms: TimeDelta::days(3).num_milliseconds(),
                }),
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
