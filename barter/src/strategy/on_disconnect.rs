//! 断开连接处理策略模块
//!
//! 本模块定义了当交易所连接断开时，Engine 应该执行的操作策略接口。
//! 策略可以实现自定义的断开连接处理逻辑，如取消所有订单、平仓所有仓位、
//! 设置交易状态为禁用等。
//!
//! # 核心概念
//!
//! - **OnDisconnectStrategy**: Trait，定义断开连接处理策略接口
//! - **使用场景**: 交易所连接断开时的应急处理
//!
//! # 常见处理方式
//!
//! 不同的策略可以实现不同的断开连接处理：
//! - 取消所有订单
//! - 平仓所有仓位
//! - 设置 `TradingState::Disabled`
//! - 记录日志和通知
//! - 等等

use crate::engine::Engine;
use barter_instrument::exchange::ExchangeId;

/// 定义 [`Engine`] 在 [`ExchangeId`] 连接断开后应执行的操作的策略接口。
///
/// OnDisconnectStrategy 允许策略自定义在交易所连接断开时的处理逻辑。
/// 这对于风险管理、订单清理和系统状态维护非常重要。
///
/// ## 常见处理方式
///
/// 不同的策略可以实现：
/// - 取消所有订单
/// - 平仓所有仓位
/// - 设置 `TradingState::Disabled`
/// - 记录断开连接事件
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
/// - **OnDisconnect**: 策略的输出类型，会被转发到 AuditStream
///
/// ## 输出类型
///
/// 输出可以包含生成的订单请求、状态更新等信息，这些信息会被记录到审计流中。
///
/// # 使用示例
///
/// ```rust,ignore
/// struct MyDisconnectStrategy {
///     // 策略配置
/// }
///
/// impl<Clock, State, ExecutionTxs, Risk> OnDisconnectStrategy<Clock, State, ExecutionTxs, Risk>
///     for MyDisconnectStrategy
/// {
///     type OnDisconnect = SomeOutput;
///
///     fn on_disconnect(
///         engine: &mut Engine<Clock, State, ExecutionTxs, Self, Risk>,
///         exchange: ExchangeId,
///     ) -> Self::OnDisconnect {
///         // 实现断开连接处理逻辑
///         // 例如：取消所有订单、平仓等
///     }
/// }
/// ```
pub trait OnDisconnectStrategy<Clock, State, ExecutionTxs, Risk>
where
    Self: Sized,
{
    /// `OnDisconnectStrategy` 的输出，会被转发到 `AuditStream`。
    ///
    /// 例如，这可以包括生成的任何订单请求。
    type OnDisconnect;

    /// 在接收到 [`ExchangeId`] 断开连接事件后执行 [`Engine`] 操作。
    ///
    /// 此方法在检测到交易所连接断开时被调用。策略可以执行任何必要的操作，
    /// 如取消订单、平仓、更新状态等。
    ///
    /// ## 注意事项
    ///
    /// - 此方法在断开连接时立即调用
    /// - 策略应该确保系统处于安全状态
    /// - 生成的输出会被记录到审计流中
    ///
    /// # 参数
    ///
    /// - `engine`: Engine 实例（可变引用）
    /// - `exchange`: 断开连接的交易所 ID
    ///
    /// # 返回值
    ///
    /// 返回策略的输出，会被转发到 AuditStream。
    fn on_disconnect(
        engine: &mut Engine<Clock, State, ExecutionTxs, Self, Risk>,
        exchange: ExchangeId,
    ) -> Self::OnDisconnect;
}
