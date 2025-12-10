//! Summary 回测摘要模块
//!
//! 本模块提供了回测结果和指标的数据结构。
//! 包括单个回测摘要和多个回测的汇总结果。

use crate::statistic::summary::TradingSummary;
use rust_decimal::Decimal;
use smol_str::SmolStr;
use std::time::Duration;

/// 包含多个 [`BacktestSummary`] 和相关多回测元数据的容器。
///
/// MultiBacktestSummary 用于存储批量回测的结果。
/// 它包含回测数量、总执行时间和所有回测摘要。
///
/// ## 类型参数
///
/// - `Interval`: 时间间隔类型，用于交易摘要
///
/// ## 字段说明
///
/// - **num_backtests**: 此批次中运行的回测数量
/// - **duration**: 所有回测的总执行时间
/// - **summaries**: `BacktestSummary` 集合
#[derive(Debug)]
pub struct MultiBacktestSummary<Interval> {
    /// 此批次中运行的回测数量。
    pub num_backtests: usize,
    /// 所有回测的总执行时间。
    pub duration: Duration,
    /// `BacktestSummary` 集合。
    pub summaries: Vec<BacktestSummary<Interval>>,
}

impl<Interval> MultiBacktestSummary<Interval> {
    /// 使用提供的数据创建新的 `MultiBacktestSummary`。
    ///
    /// ## 类型参数
    ///
    /// - `SummaryIter`: 回测摘要迭代器类型
    ///
    /// # 参数
    ///
    /// - `duration`: 总执行时间
    /// - `summary_iter`: 回测摘要迭代器
    ///
    /// # 返回值
    ///
    /// 返回新创建的 `MultiBacktestSummary` 实例。
    pub fn new<SummaryIter>(duration: Duration, summary_iter: SummaryIter) -> Self
    where
        SummaryIter: IntoIterator<Item = BacktestSummary<Interval>>,
    {
        let summaries = summary_iter.into_iter().collect::<Vec<_>>();

        Self {
            num_backtests: summaries.len(),
            duration,
            summaries,
        }
    }
}

/// 单个回测的 `TradingSummary` 和相关元数据。
///
/// BacktestSummary 包含单个回测的结果，包括唯一标识符、
/// 无风险收益率和交易摘要。
///
/// ## 类型参数
///
/// - `Interval`: 时间间隔类型，用于交易摘要
///
/// ## 字段说明
///
/// - **id**: 回测的唯一标识符（来自 `BacktestArgsDynamic`）
/// - **risk_free_return**: 用于绩效指标的无风险收益率
/// - **trading_summary**: 回测模拟交易的绩效指标和统计信息
#[derive(Debug, PartialEq)]
pub struct BacktestSummary<Interval> {
    /// [`BacktestArgsDynamic`](super::BacktestArgsDynamic) 的唯一标识符，作为回测的输入。
    pub id: SmolStr,
    /// 用于绩效指标的无风险收益率。
    pub risk_free_return: Decimal,
    /// 回测模拟交易的绩效指标和统计信息。
    pub trading_summary: TradingSummary<Interval>,
}
