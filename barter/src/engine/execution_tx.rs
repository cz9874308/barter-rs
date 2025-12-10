//! Engine 执行请求通道映射模块
//!
//! 本模块定义了如何将执行请求路由到不同交易所的执行管理器。在多交易所交易系统中，
//! Engine 需要能够将执行请求发送到正确的交易所通道。
//!
//! # 核心概念
//!
//! - **ExecutionTxMap**: 执行请求通道映射 Trait，定义如何查找和遍历执行通道
//! - **MultiExchangeTxMap**: 多交易所执行通道映射实现，使用索引映射高效路由
//!
//! # 使用场景
//!
//! - **多交易所交易**: 将执行请求路由到不同交易所
//! - **单交易所交易**: 简化版本，只有一个交易所通道
//! - **动态路由**: 根据交易所索引动态查找对应的执行通道
//!
//! # 工作原理
//!
//! 1. Engine 创建执行请求
//! 2. 根据交易所索引查找对应的执行通道
//! 3. 将执行请求发送到对应的通道
//! 4. 执行管理器接收请求并处理

use crate::{engine::error::UnrecoverableEngineError, execution::request::ExecutionRequest};
use barter_instrument::{
    exchange::{ExchangeId, ExchangeIndex},
    index::error::IndexError,
    instrument::InstrumentIndex,
};
use barter_integration::{
    channel::{Tx, UnboundedTx},
    collection::FnvIndexMap,
};
use std::fmt::Debug;

/// 为每个交易所的 [`ExecutionManager`](crate::execution::manager::ExecutionManager)
/// 收集 [`ExecutionRequest`] [`Tx`] 的集合。
///
/// ExecutionTxMap 是一个抽象接口，用于在多交易所或单交易所交易系统中路由执行请求。
/// 它提供了查找和遍历执行通道的标准方法。
///
/// ## 为什么需要这个 Trait？
///
/// 在多交易所交易系统中，Engine 需要能够：
///
/// - 根据交易所索引查找对应的执行通道
/// - 遍历所有活跃的执行通道
/// - 支持不同的实现（单交易所、多交易所等）
///
/// ## 类型参数
///
/// - `ExchangeKey`: 交易所键类型，默认为 `ExchangeIndex`
/// - `InstrumentKey`: 交易对键类型，默认为 `InstrumentIndex`
///
/// ## 关联类型
///
/// - `ExecutionTx`: 执行请求通道类型，必须实现 `Tx<Item = ExecutionRequest>`
///
/// ## 使用场景
///
/// - 多交易所交易系统
/// - 单交易所交易系统
/// - 动态路由执行请求
///
/// # 使用示例
///
/// ```rust,ignore
/// // 查找特定交易所的执行通道
/// let tx = execution_tx_map.find(&exchange_index)?;
/// tx.send(execution_request).await?;
///
/// // 遍历所有活跃的执行通道
/// for tx in execution_tx_map.iter() {
///     // 发送请求到所有通道
/// }
/// ```
pub trait ExecutionTxMap<ExchangeKey = ExchangeIndex, InstrumentKey = InstrumentIndex> {
    /// 执行请求通道类型。
    ///
    /// 必须实现 `Tx<Item = ExecutionRequest<ExchangeKey, InstrumentKey>>`。
    type ExecutionTx: Tx<Item = ExecutionRequest<ExchangeKey, InstrumentKey>>;

    /// 尝试查找指定 `ExchangeKey` 对应的 [`ExecutionRequest`] [`Tx`]。
    ///
    /// 此方法用于根据交易所索引查找对应的执行通道。如果找不到通道或通道不存在，
    /// 返回不可恢复错误。
    ///
    /// # 参数
    ///
    /// - `exchange`: 交易所键，用于查找对应的执行通道
    ///
    /// # 返回值
    ///
    /// - `Ok(&Self::ExecutionTx)`: 成功找到执行通道
    /// - `Err(UnrecoverableEngineError)`: 找不到执行通道或通道不存在
    ///
    /// # 错误情况
    ///
    /// - 交易所索引不存在
    /// - 交易所对应的执行通道为 `None`（未启用交易）
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// // 查找 Binance 的执行通道
    /// let binance_tx = execution_tx_map.find(&binance_index)?;
    ///
    /// // 发送执行请求
    /// binance_tx.send(request).await?;
    /// ```
    fn find(&self, exchange: &ExchangeKey) -> Result<&Self::ExecutionTx, UnrecoverableEngineError>;

    /// 返回所有活跃的 [`ExecutionRequest`] [`Tx`] 的迭代器。
    ///
    /// 此方法用于遍历所有已启用交易的交易所执行通道。只有非 `None` 的通道会被返回。
    ///
    /// # 返回值
    ///
    /// 返回一个迭代器，遍历所有活跃的执行通道。
    ///
    /// # 使用场景
    ///
    /// - 向所有交易所发送广播请求
    /// - 检查所有交易所的连接状态
    /// - 统计活跃交易所数量
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// // 遍历所有活跃的执行通道
    /// for tx in execution_tx_map.iter() {
    ///     // 发送请求到每个通道
    ///     tx.send(request.clone()).await?;
    /// }
    ///
    /// // 统计活跃交易所数量
    /// let active_count = execution_tx_map.iter().count();
    /// ```
    fn iter<'a>(&'a self) -> impl Iterator<Item = &'a Self::ExecutionTx>
    where
        Self::ExecutionTx: 'a;
}

/// 交易所发送器映射，高效地将执行请求路由到特定交易所的发送器通道。
///
/// MultiExchangeTxMap 是一个使用 `FnvIndexMap` 实现的执行请求通道映射，用于在多交易所
/// 交易系统中路由执行请求。它提供了 O(1) 时间复杂度的查找操作。
///
/// ## 为什么需要可选发送器？
///
/// 发送器对于交易所是可选的（`Option<Tx>`），这处理了以下情况：
///
/// - **仅跟踪不交易**: 交易系统可能跟踪某个交易所的交易对，但不实际在该交易所交易
/// - **动态启用/禁用**: 可以在运行时动态启用或禁用某个交易所的交易
/// - **索引有效性**: 如果没有可选发送器，`ExchangeIndex` 将无法保持有效性
///
/// ## 工作原理
///
/// 1. 使用 `FnvIndexMap` 存储交易所 ID 到执行通道的映射
//// 2. 通道为 `Option<Tx>`，`None` 表示该交易所未启用交易
/// 3. 通过 `ExchangeIndex` 快速查找对应的通道
/// 4. 只返回非 `None` 的通道（活跃通道）
///
/// ## 性能特点
///
/// - **O(1) 查找**: 使用索引映射实现 O(1) 时间复杂度的查找
/// - **高效迭代**: 使用过滤器只返回活跃通道
/// - **内存友好**: 使用 FNV 哈希函数，内存占用小
///
/// ## 使用场景
///
/// - 多交易所交易系统
/// - 需要动态启用/禁用交易所的场景
/// - 需要高效路由执行请求的场景
///
/// # 类型参数
///
/// - `Tx`: 发送器类型，默认为 `UnboundedTx<ExecutionRequest>`
///
/// # 使用示例
///
/// ```rust,ignore
/// // 创建多交易所映射
/// let mut tx_map = MultiExchangeTxMap::new();
/// tx_map.insert(binance_id, Some(binance_tx));
/// tx_map.insert(coinbase_id, None); // 仅跟踪，不交易
///
/// // 查找执行通道
/// let binance_tx = tx_map.find(&binance_index)?;
///
/// // 遍历活跃通道
/// for tx in tx_map.iter() {
///     tx.send(request.clone()).await?;
/// }
/// ```
#[derive(Debug)]
pub struct MultiExchangeTxMap<Tx = UnboundedTx<ExecutionRequest>>(
    /// 使用 FNV 哈希的索引映射，存储交易所 ID 到可选执行通道的映射
    FnvIndexMap<ExchangeId, Option<Tx>>,
);

impl<Tx> FromIterator<(ExchangeId, Option<Tx>)> for MultiExchangeTxMap<Tx> {
    /// 从迭代器创建 `MultiExchangeTxMap`。
    ///
    /// 允许从 `(ExchangeId, Option<Tx>)` 元组的迭代器直接创建映射。
    ///
    /// # 参数
    ///
    /// - `iter`: 包含 `(ExchangeId, Option<Tx>)` 元组的迭代器
    ///
    /// # 返回值
    ///
    /// 返回新创建的 `MultiExchangeTxMap` 实例。
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// // 从向量创建映射
    /// let exchanges = vec![
    ///     (binance_id, Some(binance_tx)),
    ///     (coinbase_id, None),
    /// ];
    /// let tx_map: MultiExchangeTxMap = exchanges.into_iter().collect();
    /// ```
    fn from_iter<Iter>(iter: Iter) -> Self
    where
        Iter: IntoIterator<Item = (ExchangeId, Option<Tx>)>,
    {
        MultiExchangeTxMap(FnvIndexMap::from_iter(iter))
    }
}

impl<'a, Tx> IntoIterator for &'a MultiExchangeTxMap<Tx> {
    type Item = (&'a ExchangeId, &'a Option<Tx>);
    type IntoIter = indexmap::map::Iter<'a, ExchangeId, Option<Tx>>;

    /// 创建不可变迭代器，遍历所有交易所及其执行通道。
    ///
    /// 允许使用 `for` 循环遍历映射中的所有条目。
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// for (exchange_id, tx_option) in &tx_map {
    ///     if let Some(tx) = tx_option {
    ///         // 处理活跃通道
    ///     }
    /// }
    /// ```
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a, Tx> IntoIterator for &'a mut MultiExchangeTxMap<Tx> {
    type Item = (&'a ExchangeId, &'a mut Option<Tx>);
    type IntoIter = indexmap::map::IterMut<'a, ExchangeId, Option<Tx>>;

    /// 创建可变迭代器，遍历所有交易所及其执行通道。
    ///
    /// 允许使用 `for` 循环遍历并修改映射中的所有条目。
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// for (exchange_id, tx_option) in &mut tx_map {
    ///     // 可以修改通道
    ///     *tx_option = Some(new_tx);
    /// }
    /// ```
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl<Transmitter> ExecutionTxMap<ExchangeIndex, InstrumentIndex> for MultiExchangeTxMap<Transmitter>
where
    Transmitter: Tx<Item = ExecutionRequest> + Debug,
{
    type ExecutionTx = Transmitter;

    /// 查找指定交易所索引对应的执行通道。
    ///
    /// 此实现使用索引映射的 `get_index` 方法进行 O(1) 查找。如果找不到通道或通道为 `None`，
    /// 返回不可恢复错误。
    ///
    /// # 参数
    ///
    /// - `exchange`: 交易所索引
    ///
    /// # 返回值
    ///
    /// - `Ok(&Transmitter)`: 成功找到执行通道
    /// - `Err(UnrecoverableEngineError)`: 找不到执行通道或通道为 `None`
    ///
    /// # 错误情况
    ///
    /// - 交易所索引不存在于映射中
    /// - 交易所对应的执行通道为 `None`（未启用交易）
    ///
    /// # 工作原理
    ///
    /// 1. 使用 `ExchangeIndex` 的索引值查找映射
    /// 2. 如果找到，检查通道是否为 `Some`
    /// 3. 如果为 `Some`，返回通道引用
    /// 4. 否则返回错误
    fn find(
        &self,
        exchange: &ExchangeIndex,
    ) -> Result<&Self::ExecutionTx, UnrecoverableEngineError> {
        // 使用索引值进行 O(1) 查找
        self.0
            .get_index(exchange.index())
            .and_then(|(_exchange, tx)| tx.as_ref())
            .ok_or_else(|| {
                UnrecoverableEngineError::IndexError(IndexError::ExchangeIndex(format!(
                    "failed to find ExecutionTx for ExchangeIndex: {exchange}. Available: {self:?}"
                )))
            })
    }

    /// 返回所有活跃执行通道的迭代器。
    ///
    /// 此实现只返回非 `None` 的通道，即已启用交易的交易所通道。
    ///
    /// # 返回值
    ///
    /// 返回一个迭代器，遍历所有活跃的执行通道（非 `None` 的通道）。
    ///
    /// # 工作原理
    ///
    /// 1. 遍历映射中的所有值
    /// 2. 使用 `filter_map` 过滤出非 `None` 的通道
    /// 3. 返回通道引用
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// // 遍历所有活跃通道
    /// for tx in tx_map.iter() {
    ///     tx.send(request.clone()).await?;
    /// }
    ///
    /// // 统计活跃交易所数量
    /// let count = tx_map.iter().count();
    /// ```
    fn iter<'a>(&'a self) -> impl Iterator<Item = &'a Self::ExecutionTx>
    where
        Self::ExecutionTx: 'a,
    {
        // 只返回非 None 的通道（活跃通道）
        self.0.values().filter_map(|tx| tx.as_ref())
    }
}
