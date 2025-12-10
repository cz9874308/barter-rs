//! Statistic 统计模块
//!
//! 本模块提供了用于分析金融数据集的统计算法和指标。
//! 包括各种金融指标的计算、统计摘要的生成、时间间隔的定义等。
//!
//! # 核心概念
//!
//! - **algorithm**: 用于分析数据集的统计算法
//! - **metric**: 金融指标及其在不同时间间隔上的计算方法
//! - **summary**: 金融数据集的统计摘要
//! - **time**: 用于金融计算的时间间隔定义
//!
//! # 使用场景
//!
//! - 计算交易策略的绩效指标
//! - 生成交易摘要和报告
//! - 分析回测结果
//! - 评估风险指标

/// 用于分析数据集的统计算法。
pub mod algorithm;

/// 金融指标及其在不同 [`TimeIntervals`](time::TimeInterval) 上的计算方法。
pub mod metric;

/// 金融数据集的统计摘要。
///
/// 例如，`TradingSummary`、`TearSheet`、`TearSheetAsset`、`PnLReturns` 等。
pub mod summary;

/// 用于金融计算的时间间隔定义。
///
/// 例如，`Annual365`、`Annual252`、`Daily` 等。
pub mod time;
