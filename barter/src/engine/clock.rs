//! Engine 时钟模块
//!
//! 本模块定义了 Engine 如何确定当前时间的接口。通过抽象时钟接口，Engine 可以在实盘交易和回测
//! 之间无缝切换，使用不同的时间源。
//!
//! # 核心概念
//!
//! - **EngineClock**: 定义如何获取当前时间的 Trait
//! - **LiveClock**: 实盘交易使用的实时时钟（使用系统当前时间）
//! - **HistoricalClock**: 回测使用的历史时钟（使用事件中的历史时间戳）
//! - **TimeExchange**: 从事件中提取交易所时间戳的 Trait
//!
//! # 工作原理
//!
//! Engine 通过 EngineClock 获取当前时间，这个时间用于：
//!
//! - 记录事件处理时间
//! - 计算时间相关的统计指标
//! - 生成交易摘要
//!
//! 在实盘交易中，使用 LiveClock 返回系统当前时间；在回测中，使用 HistoricalClock
//! 返回历史事件的时间戳，确保回测结果的时间准确性。

use crate::{EngineEvent, engine::Processor, execution::AccountStreamEvent};
use barter_data::streams::consumer::MarketStreamEvent;
use barter_execution::AccountEventKind;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{fmt::Debug, ops::Add, sync::Arc};
use tracing::{debug, error, warn};

/// 定义 [`Engine`](super::Engine) 如何确定当前时间。
///
/// EngineClock 是一个抽象接口，允许 Engine 使用不同的时间源。
/// 这种设计使得 Engine 可以在实盘交易和回测之间无缝切换。
///
/// ## 为什么需要这个 Trait？
///
/// 在实盘交易和回测中，时间处理方式不同：
///
/// - **实盘交易**：需要使用系统当前时间（`Utc::now()`）
/// - **回测**：需要使用历史事件的时间戳，确保回测结果的时间准确性
///
/// 通过抽象时钟接口，Engine 不需要关心具体使用哪种时间源。
///
/// ## 实现要求
///
/// 实现者必须：
/// - 实现 `time()` 方法，返回当前时间（UTC）
/// - 时间应该是单调递增的（或至少不会大幅倒退）
///
/// ## 使用示例
///
/// ```rust,ignore
/// // 实盘交易
/// let clock = LiveClock;
/// let current_time = clock.time();
///
/// // 回测
/// let clock = HistoricalClock::new(initial_time);
/// let current_time = clock.time();
/// ```
pub trait EngineClock {
    /// 获取当前时间。
    ///
    /// # 返回值
    ///
    /// 返回当前时间（UTC 格式）。
    ///
    /// # 注意事项
    ///
    /// - 在实盘交易中，返回系统当前时间
    /// - 在回测中，返回基于历史事件的时间戳
    /// - 时间应该是单调递增的
    fn time(&self) -> DateTime<Utc>;
}

/// 定义如何从事件中提取"交易所时间戳"。
///
/// TimeExchange 用于从事件中提取交易所时间戳，这对于 HistoricalClock 确定当前历史时间至关重要。
///
/// ## 为什么需要这个 Trait？
///
/// 在回测中，HistoricalClock 需要从事件中提取时间戳来确定当前历史时间。
/// 不同的事件类型（市场事件、账户事件等）可能有不同的时间戳字段，这个 Trait
/// 提供了统一的接口来提取时间戳。
///
/// ## 实现要求
///
/// 实现者必须：
/// - 实现 `time_exchange()` 方法
/// - 如果事件包含时间戳，返回 `Some(DateTime<Utc>)`
/// - 如果事件不包含时间戳（如连接断开事件），返回 `None`
///
/// ## 使用场景
///
/// - HistoricalClock 处理事件时，使用此 Trait 提取时间戳
/// - 用于更新 HistoricalClock 的内部时间状态
///
/// # 返回值
///
/// 如果事件包含交易所时间戳，返回 `Some(DateTime<Utc>)`；否则返回 `None`。
pub trait TimeExchange {
    /// 从事件中提取交易所时间戳。
    ///
    /// # 返回值
    ///
    /// - `Some(DateTime<Utc>)`: 如果事件包含时间戳
    /// - `None`: 如果事件不包含时间戳（如连接断开事件）
    fn time_exchange(&self) -> Option<DateTime<Utc>>;
}

/// 实盘交易使用的实时时钟，使用 `Utc::now()` 获取系统当前时间。
///
/// LiveClock 是实盘交易场景中的时钟实现，直接返回系统当前时间。
/// 它不处理事件，因为实盘交易中时间由系统时钟决定，不需要从事件中提取。
///
/// ## 使用场景
///
/// - 实盘交易：使用系统当前时间
/// - 模拟交易：使用系统当前时间
/// - 任何需要实时时间的场景
///
/// ## 工作原理
///
/// LiveClock 非常简单：每次调用 `time()` 时，直接返回 `Utc::now()`。
/// 它实现了 `Processor` Trait，但处理事件时不做任何操作（因为不需要）。
///
/// # 使用示例
///
/// ```rust,ignore
/// let clock = LiveClock;
/// let current_time = clock.time(); // 返回系统当前时间
///
/// // 在 Engine 中使用
/// let engine = Engine::new(
///     LiveClock,
///     engine_state,
///     execution_txs,
///     strategy,
///     risk_manager,
/// );
/// ```
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub struct LiveClock;

impl EngineClock for LiveClock {
    fn time(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

impl<Event> Processor<&Event> for LiveClock {
    type Audit = ();

    /// 处理事件（实盘时钟不需要处理事件，因为时间由系统决定）。
    ///
    /// # 参数
    ///
    /// - `_`: 事件（未使用）
    ///
    /// # 返回值
    ///
    /// 返回空审计 `()`。
    fn process(&mut self, _: &Event) -> Self::Audit {}
}

/// 回测使用的历史时钟，使用处理过的事件时间戳来估算当前历史时间。
///
/// HistoricalClock 是回测场景中的时钟实现。它通过处理事件中的时间戳来确定当前历史时间，
/// 确保回测结果的时间准确性。
///
/// ## 为什么需要这个时钟？
///
/// 在回测中，我们需要使用历史事件的时间戳，而不是系统当前时间。这样可以：
///
/// - 确保回测结果的时间准确性
/// - 支持快速回测（不等待真实时间流逝）
// - 支持时间相关的统计指标计算
///
/// ## 工作原理
///
/// HistoricalClock 维护两个时间：
///
/// 1. **`time_exchange_last`**: 最后一个事件的交易所时间戳
/// 2. **`time_live_last_event`**: 处理最后一个事件时的系统时间
///
/// 当调用 `time()` 时：
/// - 计算自处理最后一个事件以来经过的系统时间
/// - 将这个时间差加到最后一个事件的交易所时间戳上
/// - 返回估算的当前历史时间
///
/// 这样可以在事件之间平滑地推进时间，即使事件之间有间隔。
///
/// ## 注意事项
///
/// - 此时钟不能在没有起始 `last_exchange_timestamp` 的情况下初始化
/// - 使用线程安全的内部状态（Arc + RwLock），支持多线程访问
/// - 处理乱序事件时会记录警告，但不会更新时间（保持时间单调性）
///
/// # 使用示例
///
/// ```rust,ignore
/// // 使用第一个历史事件的时间戳初始化
/// let clock = HistoricalClock::new(first_event_time);
///
/// // 处理事件（会更新内部时间状态）
/// clock.process(&event);
///
/// // 获取当前历史时间
/// let current_time = clock.time();
///
/// // 在 Engine 中使用
/// let engine = Engine::new(
///     clock,
///     engine_state,
///     execution_txs,
///     strategy,
///     risk_manager,
/// );
/// ```
#[derive(Debug, Clone)]
pub struct HistoricalClock {
    /// 线程安全的内部状态（使用 Arc + RwLock 支持多线程访问）
    inner: Arc<parking_lot::RwLock<HistoricalClockInner>>,
}

/// HistoricalClock 的内部状态。
///
/// # 字段
///
/// - `time_exchange_last`: 最后一个事件的交易所时间戳
/// - `time_live_last_event`: 处理最后一个事件时的系统时间（用于计算时间差）
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
struct HistoricalClockInner {
    /// 最后一个事件的交易所时间戳
    time_exchange_last: DateTime<Utc>,
    /// 处理最后一个事件时的系统时间
    time_live_last_event: DateTime<Utc>,
}

impl HistoricalClock {
    /// 使用提供的 `last_exchange_time` 作为种子构造一个新的 `HistoricalClock`。
    ///
    /// # 参数
    ///
    /// - `last_exchange_time`: 最后一个事件的交易所时间戳，用作初始时间
    ///
    /// # 返回值
    ///
    /// 返回新创建的 HistoricalClock 实例。
    ///
    /// # 使用场景
    ///
    /// 通常在回测开始时，使用第一个历史事件的时间戳初始化时钟：
    ///
    /// ```rust,ignore
    /// // 从历史数据中获取第一个事件的时间戳
    /// let first_event_time = historical_events.first().unwrap().time_exchange();
    /// let clock = HistoricalClock::new(first_event_time);
    /// ```
    pub fn new(last_exchange_time: DateTime<Utc>) -> Self {
        Self {
            inner: Arc::new(parking_lot::RwLock::new(HistoricalClockInner {
                time_exchange_last: last_exchange_time,
                time_live_last_event: Utc::now(),
            })),
        }
    }
}

impl EngineClock for HistoricalClock {
    /// 获取当前历史时间。
    ///
    /// 此方法通过以下方式计算当前历史时间：
    ///
    /// 1. 获取最后一个事件的交易所时间戳
    /// 2. 计算自处理最后一个事件以来经过的系统时间
    /// 3. 将时间差加到最后一个事件的交易所时间戳上
    ///
    /// 这样可以确保在事件之间时间也能平滑推进，即使事件之间有间隔。
    ///
    /// # 返回值
    ///
    /// 返回估算的当前历史时间（UTC 格式）。
    ///
    /// # 工作原理
    ///
    /// 就像"时间机器"：
    ///
    /// - 最后一个事件的时间戳是"历史时间基准点"
    /// - 系统时间流逝是"时间推进速度"
    /// - 当前历史时间 = 历史时间基准点 + 系统时间流逝
    ///
    /// # 边界情况处理
    ///
    /// 如果系统时间倒退（不应该发生，但为了健壮性），只返回最后一个事件的交易所时间戳，
    /// 不添加负的时间差。这确保了时间的单调性。
    fn time(&self) -> DateTime<Utc> {
        // 读取内部状态（使用读锁，允许多个并发读取）
        let lock = self.inner.read();
        let time_live_last_event = lock.time_live_last_event;
        let time_exchange_last = lock.time_exchange_last;
        drop(lock);

        // 计算自处理最后一个事件以来经过的系统时间
        let delta_since_last_event_live_time =
            Utc::now().signed_duration_since(time_live_last_event);

        // 边界情况：只有当时间差为正数时才添加，以处理乱序更新
        // 这确保了时间的单调性（时间不会倒退）
        match delta_since_last_event_live_time {
            delta if delta.num_milliseconds() >= 0 => time_exchange_last.add(delta),
            _ => time_exchange_last,
        }
    }
}

impl<Event> Processor<&Event> for HistoricalClock
where
    Event: Debug + TimeExchange,
{
    type Audit = ();

    /// 处理事件，更新历史时钟的内部状态。
    ///
    /// 当事件包含交易所时间戳时，HistoricalClock 会更新其内部状态：
    /// - 如果事件时间戳更新（大于等于当前时间），更新最后事件时间
    /// - 如果事件时间戳更旧（乱序事件），记录警告但不更新时间
    ///
    /// # 参数
    ///
    /// - `event`: 要处理的事件，必须实现 `TimeExchange` trait
    ///
    /// # 工作原理
    ///
    /// 1. 从事件中提取交易所时间戳
    /// 2. 如果时间戳更新，更新内部状态
    /// 3. 如果时间戳更旧（乱序），根据时间差记录不同级别的日志
    ///
    /// # 乱序事件处理
    ///
    /// 根据时间差的大小记录不同级别的日志：
    /// - < 1 秒：debug 级别（常见的小幅乱序）
    /// - 1-30 秒：warn 级别（中等乱序）
    /// - > 30 秒：error 级别（严重乱序）
    fn process(&mut self, event: &Event) -> Self::Audit {
        // 从事件中提取交易所时间戳
        let Some(time_event_exchange) = event.time_exchange() else {
            debug!(?event, "HistoricalClock found no timestamp in event");
            return;
        };

        // 获取写锁以更新内部状态
        let mut lock = self.inner.write();

        // 输入事件的时间戳更新（大于等于当前时间）
        if time_event_exchange >= lock.time_exchange_last {
            debug!(
                ?event,
                time_exchange_last_current = ?lock.time_exchange_last,
                time_update = ?time_event_exchange,
                "HistoricalClock updating based on input event time_exchange"
            );
            // 更新最后事件的交易所时间戳和对应的实时时间
            lock.time_exchange_last = time_event_exchange;
            lock.time_live_last_event = Utc::now();
            return;
        };

        // 输入事件的时间戳更旧（乱序事件），根据时间差记录不同级别的日志
        let time_diff_secs = time_event_exchange
            .signed_duration_since(lock.time_exchange_last)
            .num_seconds()
            .abs();

        if time_diff_secs < 1 {
            // 小于 1 秒的乱序，常见情况，记录 debug
            debug!(
                ?event,
                time_exchange_last_current = ?lock.time_exchange_last,
                time_update = ?time_event_exchange,
                time_diff_secs,
                "HistoricalClock received out-of-order events"
            );
        } else if time_diff_secs < 30 {
            // 1-30 秒的乱序，中等严重，记录 warn
            warn!(
                ?event,
                time_exchange_last_current = ?lock.time_exchange_last,
                time_update = ?time_event_exchange,
                time_diff_secs,
                "HistoricalClock received out-of-order events"
            );
        } else {
            // 大于 30 秒的乱序，严重问题，记录 error
            error!(
                ?event,
                time_exchange_last_current = ?lock.time_exchange_last,
                time_update = ?time_event_exchange,
                time_diff_secs,
                "HistoricalClock received out-of-order events"
            );
        }
    }
}

impl<MarketEventKind: Debug> TimeExchange for EngineEvent<MarketEventKind> {
    /// 从 EngineEvent 中提取交易所时间戳。
    ///
    /// 根据事件类型从不同的事件数据中提取时间戳：
    ///
    /// - **市场事件**: 从 MarketEvent 中提取 `time_exchange`
    /// - **账户事件**: 根据账户事件类型从不同字段提取时间戳
    ///   - Snapshot: 使用 `time_most_recent()`
    ///   - BalanceSnapshot: 从余额时间戳中提取
    ///   - OrderSnapshot: 从订单状态中提取
    ///   - OrderCancelled: 从取消响应状态中提取（如果存在）
    ///   - Trade: 从交易时间戳中提取
    /// - **其他事件**: 不包含时间戳，返回 `None`
    ///
    /// # 返回值
    ///
    /// 如果事件包含交易所时间戳，返回 `Some(DateTime<Utc>)`；否则返回 `None`。
    fn time_exchange(&self) -> Option<DateTime<Utc>> {
        match self {
            // 市场事件：直接提取 time_exchange
            Self::Market(MarketStreamEvent::Item(event)) => Some(event.time_exchange),
            // 账户事件：根据事件类型提取时间戳
            Self::Account(AccountStreamEvent::Item(event)) => match &event.kind {
                AccountEventKind::Snapshot(snapshot) => snapshot.time_most_recent(),
                AccountEventKind::BalanceSnapshot(balance) => Some(balance.0.time_exchange),
                AccountEventKind::OrderSnapshot(order) => order.0.state.time_exchange(),
                AccountEventKind::OrderCancelled(response) => response
                    .state
                    .as_ref()
                    .map(|cancelled| cancelled.time_exchange)
                    .ok(),
                AccountEventKind::Trade(trade) => Some(trade.time_exchange),
            },
            // 其他事件类型（Shutdown、Command、TradingStateUpdate）不包含时间戳
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use barter_data::event::MarketEvent;
    use barter_instrument::{exchange::ExchangeId, instrument::InstrumentIndex};
    use chrono::TimeDelta;

    fn market_event(time_exchange: DateTime<Utc>) -> EngineEvent<()> {
        EngineEvent::Market(MarketStreamEvent::Item(MarketEvent {
            time_exchange,
            time_received: Default::default(),
            exchange: ExchangeId::BinanceSpot,
            instrument: InstrumentIndex::new(0),
            kind: (),
        }))
    }

    #[test]
    fn test_historical_clock_process() {
        #[derive(Debug)]
        struct TestCase {
            name: &'static str,
            time_initial: DateTime<Utc>,
            input_events: Vec<EngineEvent<()>>,
            expected_time_exchange_last: DateTime<Utc>,
            delay_ms: Option<u64>,
        }

        // Create a fixed initial time to use as a base
        let time_base = DateTime::<Utc>::MIN_UTC;

        // Util for adding time
        let plus_ms = |ms: i64| {
            time_base
                .checked_add_signed(TimeDelta::milliseconds(ms))
                .unwrap()
        };

        let cases = vec![
            // TC0: Basic case - single event in order
            TestCase {
                name: "single event in order",
                time_initial: time_base,
                input_events: vec![market_event(plus_ms(1000))],
                expected_time_exchange_last: plus_ms(1000),
                delay_ms: None,
            },
            // TC1: Out of order event - earlier than current
            TestCase {
                name: "out of order event - earlier than current",
                time_initial: plus_ms(1000),
                input_events: vec![market_event(plus_ms(500))],
                expected_time_exchange_last: plus_ms(1000), // Should not update
                delay_ms: None,
            },
            // TC2: Equal timestamp event
            TestCase {
                name: "equal timestamp event",
                time_initial: plus_ms(1000),
                input_events: vec![market_event(plus_ms(1000))],
                expected_time_exchange_last: plus_ms(1000), // Should maintain current time
                delay_ms: None,
            },
            // TC3: Multiple events in order
            TestCase {
                name: "multiple events in order",
                time_initial: time_base,
                input_events: vec![
                    market_event(plus_ms(1000)),
                    market_event(plus_ms(2000)),
                    market_event(plus_ms(3000)),
                ],
                expected_time_exchange_last: plus_ms(3000),
                delay_ms: Some(10), // Small delay between events
            },
            // TC4: Multiple events out of order
            TestCase {
                name: "multiple events out of order",
                time_initial: time_base,
                input_events: vec![
                    market_event(plus_ms(3000)),
                    market_event(plus_ms(1000)),
                    market_event(plus_ms(2000)),
                ],
                expected_time_exchange_last: plus_ms(3000),
                delay_ms: Some(10),
            },
            // TC5: Event with no timestamp
            TestCase {
                name: "event with no timestamp",
                time_initial: plus_ms(1000),
                input_events: vec![EngineEvent::Market(MarketStreamEvent::Reconnecting(
                    ExchangeId::BinanceSpot,
                ))],
                expected_time_exchange_last: plus_ms(1000), // Should not update
                delay_ms: None,
            },
            // TC6: Mixed events with and without timestamps
            TestCase {
                name: "mixed events with and without timestamps",
                time_initial: time_base,
                input_events: vec![
                    market_event(plus_ms(1000)),
                    EngineEvent::Market(MarketStreamEvent::Reconnecting(ExchangeId::BinanceSpot)),
                    market_event(plus_ms(2000)),
                ],
                expected_time_exchange_last: plus_ms(2000),
                delay_ms: Some(10),
            },
        ];

        for (index, test) in cases.iter().enumerate() {
            // Setup clock with initial time
            let mut clock = HistoricalClock::new(test.time_initial);

            // Process all events
            for event in test.input_events.iter() {
                clock.process(event);

                // Add delay if specified
                if let Some(delay) = test.delay_ms {
                    spin_sleep::sleep(std::time::Duration::from_millis(delay));
                }
            }

            assert_eq!(
                clock.inner.read().time_exchange_last,
                test.expected_time_exchange_last,
                "TC{} ({}) failed - incorrect time_exchange_last",
                index,
                test.name
            );
        }
    }

    #[test]
    fn test_historical_clock_time_delta_calculation() {
        let time_base = DateTime::<Utc>::MIN_UTC;
        let clock = HistoricalClock::new(time_base);

        // Get initial time
        let time_1 = clock.time();

        // Sleep to simulate time passing
        spin_sleep::sleep(std::time::Duration::from_millis(100));

        // Get time after delay
        let time_2 = clock.time();

        // Verify time has increased
        assert!(
            time_2 > time_1,
            "Historical clock time should increase with wall clock"
        );

        // Verify increase is reasonable (eg/ close to our sleep duration)
        let delta_ms = time_2.signed_duration_since(time_1).num_milliseconds();

        assert!(
            delta_ms >= 95 && delta_ms <= 105,
            "Historical clock time delta outside expected range"
        );
    }
}
