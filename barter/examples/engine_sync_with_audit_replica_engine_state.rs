use barter::{
    EngineEvent,
    engine::{
        audit::state_replica::StateReplicaManager,
        clock::LiveClock,
        state::{
            global::DefaultGlobalData,
            instrument::{data::DefaultInstrumentMarketData, filter::InstrumentFilter},
            trading::TradingState,
        },
    },
    logging::init_logging,
    risk::DefaultRiskManager,
    statistic::time::Daily,
    strategy::DefaultStrategy,
    system::{
        builder::{AuditMode, EngineFeedMode, SystemArgs, SystemBuilder},
        config::SystemConfig,
    },
};
use barter_data::{
    streams::builder::dynamic::indexed::init_indexed_multi_exchange_market_stream,
    subscription::SubKind,
};
use barter_instrument::index::IndexedInstruments;
use barter_integration::snapshot::SnapUpdates;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::{fs::File, io::BufReader, time::Duration};

/// 系统配置文件路径。
const FILE_PATH_SYSTEM_CONFIG: &str = "barter/examples/config/system_config.json";

/// 无风险收益率（5%，可根据需要配置）。
const RISK_FREE_RETURN: Decimal = dec!(0.05);

/// 示例：使用审计副本 EngineState 的同步 Engine。
///
/// 此示例演示了如何使用 StateReplicaManager 来维护 EngineState 的副本，
/// 通过审计流来同步状态。这对于需要独立跟踪 Engine 状态的场景很有用。
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志追踪
    init_logging();

    // 加载系统配置
    let SystemConfig {
        instruments,
        executions,
    } = load_config()?;

    // 构建索引交易对集合
    let instruments = IndexedInstruments::new(instruments);

    // 初始化市场数据流
    let market_stream = init_indexed_multi_exchange_market_stream(
        &instruments,
        &[SubKind::PublicTrades, SubKind::OrderBooksL1],
    )
    .await?;

    // 构建系统参数
    let args = SystemArgs::new(
        &instruments,
        executions,
        LiveClock,
        DefaultStrategy::default(),
        DefaultRiskManager::default(),
        market_stream,
        DefaultGlobalData::default(),
        |_| DefaultInstrumentMarketData::default(),
    );

    // 构建 SystemBuild：
    // 参见 SystemBuilder 了解所有配置选项
    let mut system = SystemBuilder::new(args)
        // Engine 以同步模式运行（迭代器输入）
        .engine_feed_mode(EngineFeedMode::Iterator)
        // 启用审计流（Engine 发送审计信息）
        .audit_mode(AuditMode::Enabled)
        // Engine 以 TradingState::Disabled 状态启动
        .trading_state(TradingState::Disabled)
        // 构建 System，但尚未启动任务
        .build::<EngineEvent, _>()?
        // 初始化 System，在当前运行时上生成组件任务
        .init_with_runtime(tokio::runtime::Handle::current())
        .await?;

    // 获取 Engine 审计快照和更新
    let SnapUpdates {
        snapshot: audit_snapshot,
        updates: audit_updates,
    } = system.audit.take().unwrap();

    // 使用初始 EngineState 构建 StateReplicaManager
    let mut state_replica_manager = StateReplicaManager::new(audit_snapshot, audit_updates);

    // 在阻塞任务上运行同步 AuditReplicaStateManager
    let state_replica_task = tokio::task::spawn_blocking(move || {
        state_replica_manager.run().unwrap();
        state_replica_manager
    });

    // 启用交易
    system.trading_state(TradingState::Enabled);

    // 让示例运行 5 秒...
    tokio::time::sleep(Duration::from_secs(5)).await;

    // 在关闭之前，先取消订单，然后平仓
    system.cancel_orders(InstrumentFilter::None);
    system.close_positions(InstrumentFilter::None);

    // 关闭系统
    let (engine, _shutdown_audit) = system.shutdown().await?;
    state_replica_task.await?;

    // 生成 TradingSummary<Daily>
    let trading_summary = engine
        .trading_summary_generator(RISK_FREE_RETURN)
        .generate(Daily);

    // 将 TradingSummary<Daily> 打印到终端（可以保存到文件、发送到某处等）
    trading_summary.print_summary();

    Ok(())
}

/// 从文件加载系统配置。
///
/// # 返回值
///
/// 返回加载的系统配置，如果出错则返回错误。
fn load_config() -> Result<SystemConfig, Box<dyn std::error::Error>> {
    let file = File::open(FILE_PATH_SYSTEM_CONFIG)?;
    let reader = BufReader::new(file);
    let config = serde_json::from_reader(reader)?;
    Ok(config)
}
