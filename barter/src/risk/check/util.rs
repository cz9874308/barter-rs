//! RiskManager 检查工具函数模块
//!
//! 本模块提供了辅助 RiskManager 检查的工具函数，包括计算名义价值、价格差异、Delta 等。
//!
//! # 核心函数
//!
//! - **calculate_quote_notional**: 计算计价资产的名义价值
//! - **calculate_abs_percent_difference**: 计算两个值的绝对百分比差异
//! - **calculate_delta**: 计算 Delta（价格敏感性）

use barter_instrument::Side;
use rust_decimal::Decimal;

/// 根据数量、价格和合约大小计算计价资产的名义价值。
///
/// 名义价值代表仓位的总价值，是风险管理中的重要指标。
///
/// ## 计算公式
///
/// `notional = quantity × price × contract_size`
///
/// ## 参数说明
///
/// - **quantity**: 合约或单位数量
/// - **price**: 每个合约/单位的价格
///   - 对于标准交易对，通常是当前市场价格
///   - 对于期权交易对，应该是执行价格
/// - **contract_size**: 决定每个合约实际敞口的乘数
///
/// ## 返回值
///
/// - `Some(Decimal)`: 如果计算成功，返回名义价值
/// - `None`: 如果发生溢出
///
/// # 使用示例
///
/// ```rust,ignore
/// let notional = calculate_quote_notional(
///     Decimal::new(10, 0),  // 10 个合约
///     Decimal::new(50000, 0),  // 价格 50000
///     Decimal::new(1, 0),  // 合约大小 1
/// );
/// // 返回 Some(500000) - 名义价值为 500000
/// ```
pub fn calculate_quote_notional(
    quantity: Decimal,
    price: Decimal,
    contract_size: Decimal,
) -> Option<Decimal> {
    quantity.checked_mul(price)?.checked_mul(contract_size)
}

/// 计算两个值（例如价格）之间的绝对百分比差异。
///
/// 此函数计算当前值相对于另一个值的百分比差异，返回一个表示百分比的 `Decimal`
/// （例如，0.05 表示 5% 的差异）。
///
/// ## 计算公式
///
/// `percentage = |current - other| / other`
///
/// ## 返回值
///
/// - `Some(Decimal)`: 如果计算成功，返回百分比（例如，0.05 表示 5%）
/// - `None`: 如果发生溢出或除零
///
/// # 使用示例
///
/// ```rust,ignore
/// let diff = calculate_abs_percent_difference(
///     Decimal::new(105, 0),  // 当前价格 105
///     Decimal::new(100, 0),  // 参考价格 100
/// );
/// // 返回 Some(0.05) - 5% 的差异
/// ```
pub fn calculate_abs_percent_difference(current: Decimal, other: Decimal) -> Option<Decimal> {
    // 计算绝对差异
    let price_diff = current.checked_sub(other)?.abs();

    // 计算相对于 other 的百分比差异
    price_diff.checked_div(other)
}

/// 计算某些"实物"单位数量的总 Delta。
///
/// Delta 是衡量交易对价格相对于标的资产变化的指标。它用于计算仓位的风险敞口。
///
/// ## Delta 说明
///
/// - **正返回值**: 表示对标的资产的多头敞口
/// - **负返回值**: 表示对标的资产的空头敞口
///
/// ## 参数说明
///
/// - **instrument_delta**: 交易对的 Delta
///   - 对于现货、永续合约和期货，通常是 1.0
///   - 对于期权，通常在 -1.0 和 1.0 之间
/// - **contract_size**: 决定每个合约实际敞口的乘数
/// - **side**: 数量方向，`Side::Buy`（多头）或 `Side::Sell`（空头）
/// - **quantity_in_kind**: 实物数量
///
/// ## 计算公式
///
/// `delta = instrument_delta × (quantity_in_kind × contract_size) × side_multiplier`
///
/// 其中 `side_multiplier` 为：
/// - `Side::Buy` => 1
/// - `Side::Sell` => -1
///
/// # 返回值
///
/// 返回总 Delta 值。正值表示多头敞口，负值表示空头敞口。
///
/// # 使用示例
///
/// ```rust,ignore
/// let delta = calculate_delta(
///     Decimal::new(1, 0),  // 现货 Delta = 1.0
///     Decimal::new(1, 0),  // 合约大小 = 1
///     Side::Buy,  // 多头
///     Decimal::new(10, 0),  // 10 个单位
/// );
/// // 返回 10.0 - 多头敞口为 10
/// ```
pub fn calculate_delta(
    instrument_delta: Decimal,
    contract_size: Decimal,
    side: Side,
    quantity_in_kind: Decimal,
) -> Decimal {
    let delta = instrument_delta * (quantity_in_kind * contract_size);

    match side {
        Side::Buy => delta,
        Side::Sell => -delta,
    }
}
