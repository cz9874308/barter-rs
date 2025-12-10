//! Backtest 回测模块
//!
//! 本模块提供了用于算法交易策略的回测工具。
//! 它提供了使用市场数据运行交易策略的历史模拟，并分析这些模拟的绩效。
//!
//! # 核心概念
//!
//! - **Backtest**: 单个回测，使用历史数据模拟策略
//! - **BacktestMarketData**: 回测市场数据接口
//! - **BacktestSummary**: 回测结果摘要
//! - **MultiBacktestSummary**: 多个回测的汇总结果
//!
//! # 使用场景
//!
//! - 策略参数优化
//! - 策略绩效评估
//! - 批量回测多个策略变体
//! - 历史数据验证

use crate::{
    backtest::{
        market_data::BacktestMarketData,
        summary::{BacktestSummary, MultiBacktestSummary},
    },
    engine::{
        Processor,
        clock::HistoricalClock,
        execution_tx::MultiExchangeTxMap,
        state::{EngineState, instrument::data::InstrumentDataState},
    },
    error::BarterError,
    risk::RiskManager,
    statistic::time::TimeInterval,
    strategy::{
        algo::AlgoStrategy, close_positions::ClosePositionsStrategy,
        on_disconnect::OnDisconnectStrategy, on_trading_disabled::OnTradingDisabled,
    },
    system::{builder::EngineFeedMode, config::ExecutionConfig},
};
use crate::{
    engine::Engine,
    execution::builder::{ExecutionBuild, ExecutionBuilder},
    system::builder::{AuditMode, SystemBuild},
};
use barter_data::event::MarketEvent;
use barter_execution::AccountEvent;
use barter_instrument::{index::IndexedInstruments, instrument::InstrumentIndex};
use futures::future::try_join_all;
use rust_decimal::Decimal;
use smol_str::SmolStr;
use std::{fmt::Debug, sync::Arc};

/// 定义可用于回测的不同类型市场数据源的接口和实现。
pub mod market_data;

/// 包含用于表示回测结果和指标的数据结构。
pub mod summary;

/// 批次中所有回测使用的常量配置。
///
/// 包含共享输入，如交易对、执行配置、市场数据和摘要时间间隔。
///
/// ## 类型参数
///
/// - `MarketData`: 市场数据类型
/// - `SummaryInterval`: 摘要时间间隔类型
/// - `State`: EngineState 类型
#[derive(Debug, Clone)]
pub struct BacktestArgsConstant<MarketData, SummaryInterval, State> {
    /// 由唯一标识符索引的交易对集合。
    pub instruments: IndexedInstruments,
    /// 交易所执行配置。
    pub executions: Vec<ExecutionConfig>,
    /// 用于模拟的历史市场数据。
    pub market_data: MarketData,
    /// 用于聚合和报告摘要统计的时间间隔。
    pub summary_interval: SummaryInterval,
    /// EngineState。
    pub engine_state: State,
}

/// 可在各个回测之间变化的变量配置。
///
/// 包含定义要测试的特定策略变体的参数。
///
/// ## 类型参数
///
/// - `Strategy`: 策略类型
/// - `Risk`: 风险管理类型
#[derive(Debug, Clone)]
pub struct BacktestArgsDynamic<Strategy, Risk> {
    /// 此回测的唯一标识符。
    pub id: SmolStr,
    /// 用于绩效指标的无风险收益率。
    pub risk_free_return: Decimal,
    /// 要回测的交易策略。
    pub strategy: Strategy,
    /// 风险管理规则。
    pub risk: Risk,
}
/// 并发运行多个回测，每个回测使用不同的策略参数。
///
/// 接受共享常量和不同策略配置的迭代器，然后并行执行所有回测，收集结果。
///
/// ## 工作流程
///
/// 1. 为每个策略配置创建回测任务
/// 2. 并发执行所有回测
/// 3. 收集所有回测结果
/// 4. 返回汇总结果
///
/// ## 类型参数
///
/// - `MarketData`: 市场数据类型，必须实现 `BacktestMarketData`
/// - `SummaryInterval`: 摘要时间间隔类型
/// - `Strategy`: 策略类型，必须实现多个策略 Trait
/// - `Risk`: 风险管理类型
/// - `GlobalData`: 全局数据类型
/// - `InstrumentData`: 交易对数据类型
///
/// # 参数
///
/// - `args_constant`: 共享的常量配置
/// - `args_dynamic_iter`: 动态配置的迭代器
///
/// # 返回值
///
/// 返回包含所有回测结果的 `MultiBacktestSummary`。
pub async fn run_backtests<
    MarketData,
    SummaryInterval,
    Strategy,
    Risk,
    GlobalData,
    InstrumentData,
>(
    args_constant: Arc<
        BacktestArgsConstant<MarketData, SummaryInterval, EngineState<GlobalData, InstrumentData>>,
    >,
    args_dynamic_iter: impl IntoIterator<Item = BacktestArgsDynamic<Strategy, Risk>>,
) -> Result<MultiBacktestSummary<SummaryInterval>, BarterError>
where
    MarketData: BacktestMarketData<Kind = InstrumentData::MarketEventKind>,
    SummaryInterval: TimeInterval,
    Strategy: AlgoStrategy<State = EngineState<GlobalData, InstrumentData>>
        + ClosePositionsStrategy<State = EngineState<GlobalData, InstrumentData>>
        + OnTradingDisabled<
            HistoricalClock,
            EngineState<GlobalData, InstrumentData>,
            MultiExchangeTxMap,
            Risk,
        > + OnDisconnectStrategy<
            HistoricalClock,
            EngineState<GlobalData, InstrumentData>,
            MultiExchangeTxMap,
            Risk,
        > + Send
        + 'static,
    <Strategy as OnTradingDisabled<
        HistoricalClock,
        EngineState<GlobalData, InstrumentData>,
        MultiExchangeTxMap,
        Risk,
    >>::OnTradingDisabled: Debug + Clone + Send,
    <Strategy as OnDisconnectStrategy<
        HistoricalClock,
        EngineState<GlobalData, InstrumentData>,
        MultiExchangeTxMap,
        Risk,
    >>::OnDisconnect: Debug + Clone + Send,
    Risk: RiskManager<State = EngineState<GlobalData, InstrumentData>> + Send + 'static,
    GlobalData: for<'a> Processor<&'a MarketEvent<InstrumentIndex, InstrumentData::MarketEventKind>>
        + for<'a> Processor<&'a AccountEvent>
        + Debug
        + Clone
        + Default
        + Send
        + 'static,
    InstrumentData: InstrumentDataState + Default + Send + 'static,
{
    let time_start = std::time::Instant::now();

    // 为每个动态配置创建回测 Future
    let backtest_futures = args_dynamic_iter
        .into_iter()
        .map(|args_dynamic| backtest(Arc::clone(&args_constant), args_dynamic));

    // 并发运行所有回测
    let summaries = try_join_all(backtest_futures).await?;

    Ok(MultiBacktestSummary::new(
        std::time::Instant::now().duration_since(time_start),
        summaries,
    ))
}

/// 使用给定参数运行单个回测。
///
/// 使用历史市场数据模拟交易策略并生成绩效指标。
///
/// ## 工作流程
///
/// 1. 从市场数据创建历史时钟
/// 2. 创建市场数据流
/// 3. 构建执行基础设施
/// 4. 创建 Engine 和 System
/// 5. 运行回测直到结束
/// 6. 生成交易摘要
///
/// ## 类型参数
///
/// - `MarketData`: 市场数据类型，必须实现 `BacktestMarketData`
/// - `SummaryInterval`: 摘要时间间隔类型
/// - `Strategy`: 策略类型，必须实现多个策略 Trait
/// - `Risk`: 风险管理类型
/// - `GlobalData`: 全局数据类型
/// - `InstrumentData`: 交易对数据类型
///
/// # 参数
///
/// - `args_constant`: 共享的常量配置
/// - `args_dynamic`: 动态配置
///
/// # 返回值
///
/// 返回包含回测结果的 `BacktestSummary`。
pub async fn backtest<MarketData, SummaryInterval, Strategy, Risk, GlobalData, InstrumentData>(
    args_constant: Arc<
        BacktestArgsConstant<MarketData, SummaryInterval, EngineState<GlobalData, InstrumentData>>,
    >,
    args_dynamic: BacktestArgsDynamic<Strategy, Risk>,
) -> Result<BacktestSummary<SummaryInterval>, BarterError>
where
    MarketData: BacktestMarketData<Kind = InstrumentData::MarketEventKind>,
    SummaryInterval: TimeInterval,
    Strategy: AlgoStrategy<State = EngineState<GlobalData, InstrumentData>>
        + ClosePositionsStrategy<State = EngineState<GlobalData, InstrumentData>>
        + OnTradingDisabled<
            HistoricalClock,
            EngineState<GlobalData, InstrumentData>,
            MultiExchangeTxMap,
            Risk,
        > + OnDisconnectStrategy<
            HistoricalClock,
            EngineState<GlobalData, InstrumentData>,
            MultiExchangeTxMap,
            Risk,
        > + Send
        + 'static,
    <Strategy as OnTradingDisabled<
        HistoricalClock,
        EngineState<GlobalData, InstrumentData>,
        MultiExchangeTxMap,
        Risk,
    >>::OnTradingDisabled: Debug + Clone + Send,
    <Strategy as OnDisconnectStrategy<
        HistoricalClock,
        EngineState<GlobalData, InstrumentData>,
        MultiExchangeTxMap,
        Risk,
    >>::OnDisconnect: Debug + Clone + Send,
    Risk: RiskManager<State = EngineState<GlobalData, InstrumentData>> + Send + 'static,
    GlobalData: for<'a> Processor<&'a MarketEvent<InstrumentIndex, InstrumentData::MarketEventKind>>
        + for<'a> Processor<&'a AccountEvent>
        + Debug
        + Clone
        + Default
        + Send
        + 'static,
    InstrumentData: InstrumentDataState + Send + 'static,
{
    // 从市场数据获取第一个事件时间并创建历史时钟
    let clock = args_constant
        .market_data
        .time_first_event()
        .await
        .map(HistoricalClock::new)?;
    // 创建市场数据流
    let market_stream = args_constant.market_data.stream().await?;

    // 构建执行基础设施
    let ExecutionBuild {
        execution_tx_map,
        account_channel,
        futures,
    } = args_constant
        .executions
        .clone()
        .into_iter()
        .try_fold(
            ExecutionBuilder::new(&args_constant.instruments),
            |builder, config| match config {
                ExecutionConfig::Mock(mock_config) => builder.add_mock(mock_config, clock.clone()),
            },
        )?
        .build();

    // 创建 Engine
    let engine = Engine::new(
        clock,
        args_constant.engine_state.clone(),
        execution_tx_map,
        args_dynamic.strategy,
        args_dynamic.risk,
    );

    // 创建并初始化 System
    let system = SystemBuild::new(
        engine,
        EngineFeedMode::Stream,
        AuditMode::Disabled,
        market_stream,
        account_channel,
        futures,
    )
    .init()
    .await?;

    // 运行回测直到结束
    let (engine, _shutdown_audit) = system.shutdown_after_backtest().await?;

    // 生成交易摘要
    let trading_summary = engine
        .trading_summary_generator(args_dynamic.risk_free_return)
        .generate(args_constant.summary_interval);

    Ok(BacktestSummary {
        id: args_dynamic.id,
        risk_free_return: args_dynamic.risk_free_return,
        trading_summary,
    })
}
