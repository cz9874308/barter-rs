//! Algorithm 统计算法模块
//!
//! 本模块提供了用于分析数据集的统计算法。
//! 主要包括 Welford Online 算法，用于单次遍历计算运行中的均值和方差。
//!
//! # 核心概念
//!
//! - **Welford Online 算法**: 单次遍历计算均值和方差的在线算法
//! - **均值计算**: 增量更新均值
//! - **方差计算**: 样本方差和总体方差

/// [Welford Online](https://en.wikipedia.org/wiki/Algorithms_for_calculating_variance#Welford's_online_algorithm)
/// 算法集合，用于单次遍历计算运行中的值，如均值和方差。
///
/// Welford Online 算法是一种在线算法，可以在单次遍历数据时计算均值和方差，
/// 不需要存储所有数据点。这对于处理大量数据或流式数据非常有用。
///
/// ## 算法优势
///
/// - **单次遍历**: 只需遍历数据一次
/// - **内存高效**: 不需要存储所有数据点
/// - **数值稳定**: 减少浮点数误差累积
///
/// # 使用示例
///
/// ```rust,ignore
/// use barter::statistic::algorithm::welford_online;
///
/// let mut mean = Decimal::ZERO;
/// let mut m = Decimal::ZERO;
/// let mut count = Decimal::ZERO;
///
/// for value in data {
///     count += Decimal::ONE;
///     let new_mean = welford_online::calculate_mean(mean, value, count);
///     m = welford_online::calculate_recurrence_relation_m(m, mean, value, new_mean);
///     mean = new_mean;
/// }
///
/// let variance = welford_online::calculate_sample_variance(m, count);
/// ```
pub mod welford_online {
    use rust_decimal::Decimal;

    /// 计算下一个均值。
    ///
    /// 使用 Welford Online 算法增量更新均值。
    ///
    /// ## 公式
    ///
    /// `new_mean = prev_mean + (next_value - prev_mean) / count`
    ///
    /// # 类型参数
    ///
    /// - `T`: 数值类型，必须支持减法、除法和加法赋值
    ///
    /// # 参数
    ///
    /// - `prev_mean`: 之前的均值
    /// - `next_value`: 下一个值
    /// - `count`: 当前计数（包括新值）
    ///
    /// # 返回值
    ///
    /// 返回更新后的均值。
    pub fn calculate_mean<T>(mut prev_mean: T, next_value: T, count: T) -> T
    where
        T: Copy + std::ops::Sub<Output = T> + std::ops::Div<Output = T> + std::ops::AddAssign,
    {
        prev_mean += (next_value - prev_mean) / count;
        prev_mean
    }

    /// 计算下一个 Welford Online 递推关系 M。
    ///
    /// M 是用于计算方差的中间量。
    ///
    /// ## 公式
    ///
    /// `M = prev_m + (new_value - prev_mean) * (new_value - new_mean)`
    ///
    /// # 参数
    ///
    /// - `prev_m`: 之前的 M 值
    /// - `prev_mean`: 之前的均值
    /// - `new_value`: 新值
    /// - `new_mean`: 新的均值
    ///
    /// # 返回值
    ///
    /// 返回更新后的 M 值。
    pub fn calculate_recurrence_relation_m(
        prev_m: Decimal,
        prev_mean: Decimal,
        new_value: Decimal,
        new_mean: Decimal,
    ) -> Decimal {
        prev_m + ((new_value - prev_mean) * (new_value - new_mean))
    }

    /// 使用 Bessel 校正（count - 1）和 Welford Online 递推关系 M 计算下一个无偏"样本"方差。
    ///
    /// 样本方差使用 `n - 1` 作为分母，这是无偏估计。
    ///
    /// ## 公式
    ///
    /// `variance = M / (count - 1)` （当 count >= 2 时）
    ///
    /// # 参数
    ///
    /// - `recurrence_relation_m`: Welford Online 递推关系 M
    /// - `count`: 样本数量
    ///
    /// # 返回值
    ///
    /// 返回样本方差。如果 count < 2，返回 0。
    pub fn calculate_sample_variance(recurrence_relation_m: Decimal, count: Decimal) -> Decimal {
        match count < Decimal::TWO {
            true => Decimal::ZERO,
            false => recurrence_relation_m / (count - Decimal::ONE),
        }
    }

    /// 使用 Welford Online 递推关系 M 计算下一个有偏"总体"方差。
    ///
    /// 总体方差使用 `n` 作为分母。
    ///
    /// ## 公式
    ///
    /// `variance = M / count` （当 count >= 1 时）
    ///
    /// # 参数
    ///
    /// - `recurrence_relation_m`: Welford Online 递推关系 M
    /// - `count`: 总体数量
    ///
    /// # 返回值
    ///
    /// 返回总体方差。如果 count < 1，返回 0。
    pub fn calculate_population_variance(
        recurrence_relation_m: Decimal,
        count: Decimal,
    ) -> Decimal {
        match count < Decimal::ONE {
            true => Decimal::ZERO,
            false => recurrence_relation_m / count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use std::str::FromStr;

    #[test]
    fn calculate_mean() {
        struct Input {
            prev_mean: Decimal,
            next_value: Decimal,
            count: Decimal,
            expected: Decimal,
        }

        // dataset = [0.1, -0.2, -0.05, 0.2, 0.15, -0.17]
        let inputs = vec![
            // TC0
            Input {
                prev_mean: dec!(0.0),
                next_value: dec!(0.1),
                count: dec!(1.0),
                expected: dec!(0.1),
            },
            // TC1
            Input {
                prev_mean: dec!(0.1),
                next_value: dec!(-0.2),
                count: dec!(2.0),
                expected: dec!(-0.05),
            },
            // TC2
            Input {
                prev_mean: dec!(-0.05),
                next_value: dec!(-0.05),
                count: dec!(3.0),
                expected: dec!(-0.05),
            },
            // TC3
            Input {
                prev_mean: dec!(-0.05),
                next_value: dec!(0.2),
                count: dec!(4.0),
                expected: dec!(0.0125),
            },
            // TC4
            Input {
                prev_mean: dec!(0.0125),
                next_value: dec!(0.15),
                count: dec!(5.0),
                expected: dec!(0.04),
            },
            // TC5
            Input {
                prev_mean: dec!(0.04),
                next_value: dec!(-0.17),
                count: dec!(6.0),
                expected: dec!(0.005),
            },
        ];

        for (index, test) in inputs.iter().enumerate() {
            let actual =
                welford_online::calculate_mean(test.prev_mean, test.next_value, test.count);

            assert_eq!(actual, test.expected, "TC{index} failed")
        }
    }

    #[test]
    fn calculate_recurrence_relation_m() {
        struct Input {
            prev_m: Decimal,
            prev_mean: Decimal,
            new_value: Decimal,
            new_mean: Decimal,
        }

        let inputs = vec![
            // dataset_1 = [10, 100, -10]
            Input {
                prev_m: dec!(0.0),
                prev_mean: dec!(0.0),
                new_value: dec!(10.0),
                new_mean: dec!(10.0),
            },
            Input {
                prev_m: dec!(0.0),
                prev_mean: dec!(10.0),
                new_value: dec!(100.0),
                new_mean: dec!(55.0),
            },
            Input {
                prev_m: dec!(4050.0),
                prev_mean: dec!(55.0),
                new_value: dec!(-10.0),
                new_mean: Decimal::from_str("33.333333333333333333").unwrap(),
            },
            // dataset_2 = [-5, -50, -1000]
            Input {
                prev_m: dec!(0.0),
                prev_mean: dec!(0.0),
                new_value: dec!(-5.0),
                new_mean: dec!(-5.0),
            },
            Input {
                prev_m: dec!(0.0),
                prev_mean: dec!(-5.0),
                new_value: dec!(-50.0),
                new_mean: dec!(-27.5),
            },
            Input {
                prev_m: dec!(1012.5),
                prev_mean: dec!(-27.5),
                new_value: dec!(-1000.0),
                new_mean: dec!(-351.666666666666666667),
            },
            // dataset_3 = [90000, -90000, 0]
            Input {
                prev_m: dec!(0.0),
                prev_mean: dec!(0.0),
                new_value: dec!(90000.0),
                new_mean: dec!(90000.0),
            },
            Input {
                prev_m: dec!(0.0),
                prev_mean: dec!(90000.0),
                new_value: dec!(-90000.0),
                new_mean: dec!(0.0),
            },
            Input {
                prev_m: dec!(16200000000.0),
                prev_mean: dec!(0.0),
                new_value: dec!(0.0),
                new_mean: dec!(0.0),
            },
        ];

        let expected = vec![
            dec!(0.0),
            dec!(4050.0),
            dec!(6866.6666666666666666450),
            dec!(0.0),
            dec!(1012.5),
            dec!(631516.6666666666666663425),
            dec!(0.0),
            dec!(16200000000.0),
            dec!(16200000000.0),
        ];

        for (index, (input, expected)) in inputs.iter().zip(expected.into_iter()).enumerate() {
            let actual_m = welford_online::calculate_recurrence_relation_m(
                input.prev_m,
                input.prev_mean,
                input.new_value,
                input.new_mean,
            );

            assert_eq!(actual_m, expected, "TC{index} failed");
        }
    }

    #[test]
    fn calculate_sample_variance() {
        let inputs = vec![
            (dec!(0.0), dec!(1)),
            (dec!(1050.0), dec!(5)),
            (dec!(1012.5), dec!(123223)),
            (dec!(16200000000.0), dec!(3)),
            (dec!(99999.9999), dec!(23232)),
        ];
        let expected = vec![
            dec!(0.0),
            dec!(262.5),
            dec!(0.0082168768564055120027267858),
            dec!(8100000000.0),
            dec!(4.3045929964271878093926219276),
        ];

        for ((input_m, input_count), expected) in inputs.iter().zip(expected.into_iter()) {
            let actual_variance = welford_online::calculate_sample_variance(*input_m, *input_count);
            assert_eq!(actual_variance, expected);
        }
    }

    #[test]
    fn calculate_population_variance() {
        let inputs = vec![
            (dec!(0.0), 1),
            (dec!(1050.0), 5),
            (dec!(1012.5), 123223),
            (dec!(16200000000.0), 3),
            (dec!(99999.9999), 23232),
        ];
        let expected = vec![
            dec!(0.0),
            dec!(210.0),
            dec!(0.0082168101734254157097295148),
            dec!(5400000000.0),
            dec!(4.3044077091942148760330578512),
        ];

        for (index, (input, expected)) in inputs.iter().zip(expected.into_iter()).enumerate() {
            let actual_variance =
                welford_online::calculate_population_variance(input.0, input.1.into());
            assert_eq!(actual_variance, expected, "TC{index} failed");
        }
    }
}
