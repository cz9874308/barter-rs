//! Market Data 市场数据模块
//!
//! 本模块提供了回测市场数据的接口和实现。
//! 定义了如何为回测提供市场数据流和历史时钟。

use crate::error::BarterError;
use barter_data::streams::consumer::MarketStreamEvent;
use barter_instrument::instrument::InstrumentIndex;
use chrono::{DateTime, Utc};
use futures::Stream;
use std::sync::Arc;

/// 提供回测 MarketStream 和相关 [`HistoricalClock`](crate::engine::clock::HistoricalClock) 的接口。
///
/// BacktestMarketData Trait 定义了回测所需的市场数据源接口。
/// 实现此 Trait 的类型可以提供历史市场数据用于回测。
///
/// ## 关联类型
///
/// - `Kind`: 此数据源提供的市场事件类型
///
/// ## 方法说明
///
/// - `time_first_event`: 返回市场数据流中第一个事件的时间
/// - `stream`: 返回市场事件流
pub trait BacktestMarketData {
    /// 此数据源提供的市场事件类型。
    type Kind;

    /// 返回市场数据 `Stream` 中第一个事件的 `DateTime<Utc>`。
    ///
    /// 此时间用于初始化历史时钟。
    ///
    /// # 返回值
    ///
    /// 返回第一个事件的时间，如果出错则返回错误。
    fn time_first_event(&self) -> impl Future<Output = Result<DateTime<Utc>, BarterError>>;

    /// 返回 `MarketStreamEvent` 的 `Stream`。
    ///
    /// 此流提供回测所需的所有市场事件。
    ///
    /// # 返回值
    ///
    /// 返回市场事件流，如果出错则返回错误。
    fn stream(
        &self,
    ) -> impl Future<
        Output = Result<
            impl Stream<Item = MarketStreamEvent<InstrumentIndex, Self::Kind>> + Send + 'static,
            BarterError,
        >,
    >;
}

/// 内存中的市场数据。
///
/// 将所有市场事件存储在内存中，并在需要时通过延迟克隆数据来生成 [`MarketStreamEvent`] 的 `Stream`。
///
/// ## 特点
///
/// - **内存存储**: 所有事件存储在内存中
/// - **延迟克隆**: 仅在需要时克隆数据，节省内存
/// - **高效访问**: 使用 `Arc` 共享数据，避免重复存储
///
/// ## 类型参数
///
/// - `Kind`: 市场事件类型
#[derive(Debug, Clone)]
pub struct MarketDataInMemory<Kind> {
    /// 第一个事件的时间。
    time_first_event: DateTime<Utc>,
    /// 市场事件列表（使用 Arc 共享）。
    events: Arc<Vec<MarketStreamEvent<InstrumentIndex, Kind>>>,
}

impl<Kind> BacktestMarketData for MarketDataInMemory<Kind>
where
    Kind: Clone + Sync + Send + 'static,
{
    type Kind = Kind;

    /// 返回第一个事件的时间。
    async fn time_first_event(&self) -> Result<DateTime<Utc>, BarterError> {
        Ok(self.time_first_event)
    }

    /// 返回市场事件流。
    ///
    /// 通过延迟克隆事件来创建流，避免一次性克隆所有数据。
    async fn stream(
        &self,
    ) -> Result<
        impl Stream<Item = MarketStreamEvent<InstrumentIndex, Self::Kind>> + Send + 'static,
        BarterError,
    > {
        let events = Arc::clone(&self.events);
        // 创建延迟克隆迭代器，只在需要时克隆事件
        let lazy_clone_iter = (0..events.len()).map(move |index| events[index].clone());
        let stream = futures::stream::iter(lazy_clone_iter);
        Ok(stream)
    }
}

impl<Kind> MarketDataInMemory<Kind> {
    /// 从市场事件向量创建新的内存市场数据源。
    ///
    /// ## 参数
    ///
    /// - `events`: 市场事件列表（使用 Arc 共享）
    ///
    /// ## Panics
    ///
    /// 如果事件列表为空，此函数会 panic。
    ///
    /// # 返回值
    ///
    /// 返回新创建的 `MarketDataInMemory` 实例。
    pub fn new(events: Arc<Vec<MarketStreamEvent<InstrumentIndex, Kind>>>) -> Self {
        // 查找第一个实际市场事件（非控制事件）的时间
        let time_first_event = events
            .iter()
            .find_map(|event| match event {
                MarketStreamEvent::Item(event) => Some(event.time_exchange),
                _ => None,
            })
            .expect("cannot construct MarketDataInMemory using an empty Vec<MarketStreamEvent>");

        Self {
            time_first_event,
            events,
        }
    }
}
