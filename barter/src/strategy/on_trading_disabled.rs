//! 交易禁用处理策略模块
//!
//! 本模块定义了当 `TradingState` 设置为 `TradingState::Disabled` 时，Engine 应该
//! 执行的操作策略接口。策略可以实现自定义的交易禁用处理逻辑，如取消所有订单、
//! 平仓所有仓位等。
//!
//! # 核心概念
//!
//! - **OnTradingDisabled**: Trait，定义交易禁用处理策略接口
//! - **使用场景**: 交易被禁用时的清理和状态维护
//!
//! # 常见处理方式
//!
//! 不同的策略可以实现不同的交易禁用处理：
//! - 取消所有订单
//! - 平仓所有仓位
//! - 记录日志和通知
//! - 等等

use crate::engine::Engine;

/// 定义 [`Engine`] 在 `TradingState` 设置为 `TradingState::Disabled` 后应执行的操作的策略接口。
///
/// OnTradingDisabled 允许策略自定义在交易被禁用时的处理逻辑。
/// 这对于清理订单、平仓和系统状态维护非常重要。
///
/// ## 常见处理方式
///
/// 不同的策略可以实现：
/// - 取消所有订单
/// - 平仓所有仓位
/// - 记录交易禁用事件
/// - 发送通知
/// - 等等
///
/// ## 类型参数
///
/// - `Clock`: Engine 时钟类型
/// - `State`: Engine 状态类型
/// - `ExecutionTxs`: 执行通道映射类型
/// - `Risk`: 风险管理器类型
///
/// ## 关联类型
///
/// - **OnTradingDisabled**: 策略的输出类型，会被转发到 AuditStream
///
/// ## 输出类型
///
/// 输出可以包含生成的订单请求、状态更新等信息，这些信息会被记录到审计流中。
///
/// # 使用示例
///
/// ```rust,ignore
/// struct MyTradingDisabledStrategy {
///     // 策略配置
/// }
///
/// impl<Clock, State, ExecutionTxs, Risk> OnTradingDisabled<Clock, State, ExecutionTxs, Risk>
///     for MyTradingDisabledStrategy
/// {
///     type OnTradingDisabled = SomeOutput;
///
///     fn on_trading_disabled(
///         engine: &mut Engine<Clock, State, ExecutionTxs, Self, Risk>,
///     ) -> Self::OnTradingDisabled {
///         // 实现交易禁用处理逻辑
///         // 例如：取消所有订单、平仓等
///     }
/// }
/// ```
pub trait OnTradingDisabled<Clock, State, ExecutionTxs, Risk>
where
    Self: Sized,
{
    /// `OnTradingDisabled` 的输出，会被转发到 `AuditStream`。
    ///
    /// 例如，这可以包括生成的任何订单请求。
    type OnTradingDisabled;

    /// 在 `TradingState` 设置为 `TradingState::Disabled` 后执行 [`Engine`] 操作。
    ///
    /// 此方法在交易状态被设置为禁用时被调用。策略可以执行任何必要的操作，
    /// 如取消订单、平仓、更新状态等。
    ///
    /// ## 注意事项
    ///
    /// - 此方法在交易被禁用时立即调用
    /// - 策略应该确保系统处于安全状态
    /// - 生成的输出会被记录到审计流中
    ///
    /// # 参数
    ///
    /// - `engine`: Engine 实例（可变引用）
    ///
    /// # 返回值
    ///
    /// 返回策略的输出，会被转发到 AuditStream。
    fn on_trading_disabled(
        engine: &mut Engine<Clock, State, ExecutionTxs, Self, Risk>,
    ) -> Self::OnTradingDisabled;
}
