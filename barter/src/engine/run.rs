//! Engine 运行器模块
//!
//! 本模块提供了 Engine 的四种运行模式，支持同步和异步两种执行方式，以及是否启用审计流两种配置。
//!
//! # 运行模式
//!
//! - **同步运行** (`sync_run`): 使用 Iterator 同步处理事件，适用于回测和单线程场景
//! - **同步运行（带审计）** (`sync_run_with_audit`): 同步运行并发送审计信息到审计流
//! - **异步运行** (`async_run`): 使用 Stream 异步处理事件，适用于实盘交易和多线程场景
//! - **异步运行（带审计）** (`async_run_with_audit`): 异步运行并发送审计信息到审计流
//!
//! # 工作原理
//!
//! 所有运行器都遵循相同的事件处理循环：
//!
//! 1. 从事件源获取下一个事件
//! 2. 使用 Engine 处理事件并生成审计信息
//! 3. 检查是否为终止事件（如 Shutdown）
//! 4. 如果启用审计，将审计信息发送到审计流
//! 5. 重复直到收到终止事件或事件源结束
//! 6. 关闭 Engine 并返回关闭审计

use crate::{
    engine::{
        Processor,
        audit::{AuditTick, Auditor, context::EngineContext},
        process_with_audit,
    },
    shutdown::SyncShutdown,
};
use barter_integration::{
    FeedEnded, Terminal,
    channel::{ChannelTxDroppable, Tx},
};
use futures::{Stream, StreamExt};
use std::fmt::Debug;
use tracing::info;

/// 同步 Engine 运行器，处理输入事件。
///
/// 此函数使用同步方式运行 Engine，从 Iterator 中获取事件并逐个处理。
/// 运行直到收到关闭信号，返回详细说明关闭原因的审计信息
/// （例如：事件源结束 `FeedEnded`、`Command::Shutdown` 等）。
///
/// ## 为什么需要这个函数？
///
/// 同步运行模式适用于：
/// - **回测场景**：使用历史数据，不需要异步 I/O
/// - **单线程环境**：简单的单线程应用
/// - **确定性执行**：需要确定的事件处理顺序
///
/// ## 工作原理
///
/// 就像一条生产线：
///
/// 1. **获取事件**：从 Iterator 中获取下一个事件（同步操作）
/// 2. **处理事件**：使用 Engine 处理事件，生成审计信息
/// 3. **检查终止**：如果事件是终止事件（如 Shutdown），退出循环
/// 4. **重复处理**：继续处理下一个事件
/// 5. **关闭清理**：当事件源结束或收到关闭信号时，关闭 Engine 并返回
///
/// ## 类型参数
///
/// - `Events`: 事件迭代器类型，必须实现 `Iterator`
/// - `Engine`: 事件处理器，必须实现 `Processor`、`Auditor` 和 `SyncShutdown`
///
/// # 参数
///
/// - `feed`: 事件迭代器，提供要处理的事件
/// - `engine`: 要运行的 Engine 实例
///
/// # 返回值
///
/// 返回关闭审计，详细说明关闭原因。
///
/// # 使用示例
///
/// ```rust,ignore
/// let mut events = vec![event1, event2, event3].into_iter();
/// let mut engine = Engine::new(...);
/// let shutdown_audit = sync_run(&mut events, &mut engine);
/// ```
pub fn sync_run<Events, Engine>(feed: &mut Events, engine: &mut Engine) -> Engine::Audit
where
    Events: Iterator,
    Events::Item: Debug + Clone,
    Engine:
        Processor<Events::Item> + Auditor<Engine::Audit, Context = EngineContext> + SyncShutdown,
    Engine::Audit: From<FeedEnded> + Terminal + Debug,
{
    info!(
        feed_mode = "sync",
        audit_mode = "disabled",
        "Engine running"
    );

    // 运行 Engine 处理循环直到关闭
    let shutdown_audit = loop {
        // 从迭代器获取下一个事件，如果迭代器结束则退出
        let Some(event) = feed.next() else {
            break engine.audit(FeedEnded);
        };

        // 处理事件并生成 AuditTick
        let audit = process_with_audit(engine, event);

        // 检查 AuditTick 是否指示需要关闭
        if audit.event.is_terminal() {
            break audit;
        }
    };

    info!(
        shutdown_audit = ?shutdown_audit.event,
        context = ?shutdown_audit.context,
        "Engine shutting down"
    );

    // 关闭 Engine，向所有 ExecutionManager 发送关闭信号
    let _ = engine.shutdown();

    shutdown_audit.event
}

/// 同步 Engine 运行器，处理输入事件并将审计信息转发到提供的 `AuditTx`。
///
/// 此函数与 `sync_run` 类似，但会将所有审计信息发送到审计通道。
/// 运行直到收到关闭信号，返回详细说明关闭原因的审计信息
/// （例如：事件源结束 `FeedEnded`、`Command::Shutdown` 等）。
///
/// ## 为什么需要这个函数？
///
/// 带审计的同步运行模式适用于：
/// - **回测监控**：需要实时查看回测过程中的审计信息
/// - **状态副本**：需要维护 EngineState 的副本（通过 StateReplicaManager）
/// - **日志记录**：需要记录所有 Engine 操作
/// - **UI 更新**：需要将 Engine 状态更新到用户界面
///
/// ## 工作原理
///
/// 与 `sync_run` 类似，但增加了审计流：
///
/// 1. **获取事件**：从 Iterator 中获取下一个事件
/// 2. **处理事件**：使用 Engine 处理事件，生成审计信息
/// 3. **发送审计**：将审计信息发送到审计通道（供其他组件使用）
/// 4. **检查终止**：如果事件是终止事件，退出循环
/// 5. **关闭清理**：发送关闭审计，关闭 Engine 并返回
///
/// ## 类型参数
///
/// - `Events`: 事件迭代器类型，必须实现 `Iterator`
/// - `Engine`: 事件处理器，必须实现 `Processor`、`Auditor` 和 `SyncShutdown`
/// - `AuditTx`: 审计通道类型，用于发送审计信息
///
/// # 参数
///
/// - `feed`: 事件迭代器，提供要处理的事件
/// - `engine`: 要运行的 Engine 实例
/// - `audit_tx`: 审计通道，用于发送审计信息
///
/// # 返回值
///
/// 返回关闭审计，详细说明关闭原因。
///
/// # 使用示例
///
/// ```rust,ignore
/// let mut events = vec![event1, event2].into_iter();
/// let mut engine = Engine::new(...);
/// let (mut audit_tx, audit_rx) = channel();
/// let shutdown_audit = sync_run_with_audit(&mut events, &mut engine, &mut audit_tx);
///
/// // 在另一个线程中接收审计信息
/// while let Some(audit) = audit_rx.recv() {
///     // 处理审计信息
/// }
/// ```
pub fn sync_run_with_audit<Events, Engine, AuditTx>(
    feed: &mut Events,
    engine: &mut Engine,
    audit_tx: &mut ChannelTxDroppable<AuditTx>,
) -> Engine::Audit
where
    Events: Iterator,
    Events::Item: Debug + Clone,
    Engine:
        Processor<Events::Item> + Auditor<Engine::Audit, Context = EngineContext> + SyncShutdown,
    Engine::Audit: From<FeedEnded> + Terminal + Debug + Clone,
    AuditTx: Tx<Item = AuditTick<Engine::Audit, EngineContext>>,
{
    info!(feed_mode = "sync", audit_mode = "enabled", "Engine running");

    // 运行 Engine 处理循环直到关闭
    let shutdown_audit = loop {
        // 从迭代器获取下一个事件，如果迭代器结束则退出
        let Some(event) = feed.next() else {
            break engine.audit(FeedEnded);
        };

        // 处理事件并生成 AuditTick
        let audit = process_with_audit(engine, event);

        // 检查 AuditTick 是否指示需要关闭
        if audit.event.is_terminal() {
            break audit;
        }

        // 将 AuditTick 发送到审计管理器（供其他组件使用，如 StateReplicaManager）
        audit_tx.send(audit);
    };

    // 发送关闭审计，确保审计流收到关闭信号
    audit_tx.send(shutdown_audit.clone());

    info!(
        shutdown_audit = ?shutdown_audit.event,
        context = ?shutdown_audit.context,
        "Engine shutting down"
    );

    // 关闭 Engine，向所有 ExecutionManager 发送关闭信号
    let _ = engine.shutdown();

    shutdown_audit.event
}

/// 异步 Engine 运行器，处理输入事件。
///
/// 此函数使用异步方式运行 Engine，从 Stream 中获取事件并逐个处理。
/// 运行直到收到关闭信号，返回详细说明关闭原因的审计信息
/// （例如：事件源结束 `FeedEnded`、`Command::Shutdown` 等）。
///
/// ## 为什么需要这个函数？
///
/// 异步运行模式适用于：
/// - **实盘交易**：需要处理实时市场数据和网络 I/O
/// - **多线程环境**：需要并发处理多个任务
/// - **非阻塞操作**：需要异步等待事件，不阻塞其他任务
/// - **高并发场景**：需要处理大量并发连接和事件
///
/// ## 工作原理
///
/// 与同步运行类似，但使用异步 Stream：
///
/// 1. **异步获取事件**：从 Stream 中异步获取下一个事件（使用 `.await`）
/// 2. **处理事件**：使用 Engine 处理事件，生成审计信息
/// 3. **检查终止**：如果事件是终止事件，退出循环
/// 4. **重复处理**：继续异步等待下一个事件
/// 5. **关闭清理**：当事件源结束或收到关闭信号时，关闭 Engine 并返回
///
/// ## 类型参数
///
/// - `Events`: 事件流类型，必须实现 `Stream + Unpin`
/// - `Engine`: 事件处理器，必须实现 `Processor`、`Auditor` 和 `SyncShutdown`
///
/// # 参数
///
/// - `feed`: 事件流，提供要处理的事件
/// - `engine`: 要运行的 Engine 实例
///
/// # 返回值
///
/// 返回关闭审计，详细说明关闭原因。
///
/// # 使用示例
///
/// ```rust,ignore
/// let mut market_stream = create_market_stream();
/// let mut engine = Engine::new(...);
/// let shutdown_audit = async_run(&mut market_stream, &mut engine).await;
/// ```
pub async fn async_run<Events, Engine>(feed: &mut Events, engine: &mut Engine) -> Engine::Audit
where
    Events: Stream + Unpin,
    Events::Item: Debug + Clone,
    Engine:
        Processor<Events::Item> + Auditor<Engine::Audit, Context = EngineContext> + SyncShutdown,
    Engine::Audit: From<FeedEnded> + Terminal + Debug,
{
    info!(
        feed_mode = "async",
        audit_mode = "disabled",
        "Engine running"
    );

    // 运行 Engine 处理循环直到关闭
    let shutdown_audit = loop {
        // 从流中异步获取下一个事件，如果流结束则退出
        let Some(event) = feed.next().await else {
            break engine.audit(FeedEnded);
        };

        // 处理事件并生成 AuditTick
        let audit = process_with_audit(engine, event);

        // 检查 AuditTick 是否指示需要关闭
        if audit.event.is_terminal() {
            break audit;
        }
    };

    info!(
        shutdown_audit = ?shutdown_audit.event,
        context = ?shutdown_audit.context,
        "Engine shutting down"
    );

    // 关闭 Engine，向所有 ExecutionManager 发送关闭信号
    let _ = engine.shutdown();

    shutdown_audit.event
}

/// 异步 Engine 运行器，处理输入事件并将审计信息转发到提供的 `AuditTx`。
///
/// 此函数与 `async_run` 类似，但会将所有审计信息发送到审计通道。
/// 运行直到收到关闭信号，返回详细说明关闭原因的审计信息
/// （例如：事件源结束 `FeedEnded`、`Command::Shutdown` 等）。
///
/// ## 为什么需要这个函数？
///
/// 带审计的异步运行模式适用于：
/// - **实盘监控**：需要实时查看实盘交易过程中的审计信息
/// - **状态副本**：需要维护 EngineState 的副本（通过 StateReplicaManager）
/// - **日志记录**：需要异步记录所有 Engine 操作
/// - **UI 更新**：需要将 Engine 状态异步更新到用户界面
/// - **Telegram 机器人**：需要将交易状态发送到 Telegram
///
/// ## 工作原理
///
/// 与 `async_run` 类似，但增加了审计流：
///
/// 1. **异步获取事件**：从 Stream 中异步获取下一个事件
/// 2. **处理事件**：使用 Engine 处理事件，生成审计信息
/// 3. **发送审计**：将审计信息异步发送到审计通道
/// 4. **检查终止**：如果事件是终止事件，退出循环
/// 5. **关闭清理**：发送关闭审计，关闭 Engine 并返回
///
/// ## 类型参数
///
/// - `Events`: 事件流类型，必须实现 `Stream + Unpin`
/// - `Engine`: 事件处理器，必须实现 `Processor`、`Auditor` 和 `SyncShutdown`
/// - `AuditTx`: 审计通道类型，用于发送审计信息
///
/// # 参数
///
/// - `feed`: 事件流，提供要处理的事件
/// - `engine`: 要运行的 Engine 实例
/// - `audit_tx`: 审计通道，用于发送审计信息
///
/// # 返回值
///
/// 返回关闭审计，详细说明关闭原因。
///
/// # 使用示例
///
/// ```rust,ignore
/// let mut market_stream = create_market_stream();
/// let mut engine = Engine::new(...);
/// let (mut audit_tx, mut audit_rx) = channel();
///
/// // 在另一个任务中接收审计信息
/// tokio::spawn(async move {
///     while let Some(audit) = audit_rx.recv().await {
///         // 处理审计信息，如更新 UI、发送到 Telegram 等
///     }
/// });
///
/// let shutdown_audit = async_run_with_audit(
///     &mut market_stream,
///     &mut engine,
///     &mut audit_tx
/// ).await;
/// ```
pub async fn async_run_with_audit<Events, Engine, AuditTx>(
    feed: &mut Events,
    engine: &mut Engine,
    audit_tx: &mut ChannelTxDroppable<AuditTx>,
) -> Engine::Audit
where
    Events: Stream + Unpin,
    Events::Item: Debug + Clone,
    Engine:
        Processor<Events::Item> + Auditor<Engine::Audit, Context = EngineContext> + SyncShutdown,
    Engine::Audit: From<FeedEnded> + Terminal + Debug + Clone,
    AuditTx: Tx<Item = AuditTick<Engine::Audit, EngineContext>>,
{
    info!(
        feed_mode = "async",
        audit_mode = "enabled",
        "Engine running"
    );

    // 运行 Engine 处理循环直到关闭
    let shutdown_audit = loop {
        // 从流中异步获取下一个事件，如果流结束则退出
        let Some(event) = feed.next().await else {
            break engine.audit(FeedEnded);
        };

        // 处理事件并生成 AuditTick
        let audit = process_with_audit(engine, event);

        // 检查 AuditTick 是否指示需要关闭
        if audit.event.is_terminal() {
            break audit;
        }

        // 将 AuditTick 发送到审计管理器（供其他组件使用，如 StateReplicaManager）
        audit_tx.send(audit);
    };

    // 发送关闭审计，确保审计流收到关闭信号
    audit_tx.send(shutdown_audit.clone());

    info!(
        shutdown_audit = ?shutdown_audit.event,
        context = ?shutdown_audit.context,
        "Engine shutting down"
    );

    // 关闭 Engine，向所有 ExecutionManager 发送关闭信号
    let _ = engine.shutdown();

    shutdown_audit.event
}
