//! Engine 交易状态模块
//!
//! 本模块定义了 Engine 的交易状态，用于控制 Engine 是否生成算法订单。
//!
//! # 核心概念
//!
//! - **TradingState**: 交易状态枚举，表示 Engine 是否启用交易
//! - **Enabled**: 交易启用，Engine 会生成算法订单
//! - **Disabled**: 交易禁用，Engine 不会生成算法订单，但仍会处理命令和更新状态
//!
//! # 使用场景
//!
//! - 控制 Engine 的交易行为
//! - 暂停/恢复算法交易
//! - 系统维护时禁用交易

use serde::{Deserialize, Serialize};
use tracing::info;

/// 表示 `Engine` 的当前 `TradingState`（交易状态）。
///
/// TradingState 控制 Engine 是否生成算法订单。它有两个状态：
///
/// - **Enabled**: Engine 会使用 `AlgoStrategy` 实现生成算法订单
/// - **Disabled**: Engine 会继续基于输入事件更新状态，但不会生成算法订单。
///   在此状态下，`Commands` 仍然会被执行（例如"开仓"、"取消订单"、"平仓"等）
///
/// ## 状态转换
///
/// 可以通过 `update()` 方法更新交易状态，该方法会返回审计记录，记录状态转换。
///
/// # 使用示例
///
/// ```rust,ignore
/// let mut trading_state = TradingState::Disabled;
///
/// // 启用交易
/// let audit = trading_state.update(TradingState::Enabled);
/// assert_eq!(audit.prev, TradingState::Disabled);
/// assert_eq!(audit.current, TradingState::Enabled);
///
/// // 禁用交易
/// let audit = trading_state.update(TradingState::Disabled);
/// assert_eq!(audit.prev, TradingState::Enabled);
/// assert_eq!(audit.current, TradingState::Disabled);
/// ```
#[derive(
    Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Default, Deserialize, Serialize,
)]
pub enum TradingState {
    /// 交易启用：Engine 会生成算法订单。
    Enabled,
    /// 交易禁用：Engine 不会生成算法订单，但仍会处理命令和更新状态（默认状态）。
    #[default]
    Disabled,
}

impl TradingState {
    /// 更新 Engine 的 `TradingState`。
    ///
    /// 此方法更新交易状态并返回审计记录，记录状态转换。如果新状态与当前状态相同，
    /// 仍然会记录日志（用于调试）。
    ///
    /// # 参数
    ///
    /// - `update`: 新的交易状态
    ///
    /// # 返回值
    ///
    /// 返回 [`TradingStateUpdateAudit`]，包含之前和新的状态记录。
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let mut state = TradingState::Disabled;
    /// let audit = state.update(TradingState::Enabled);
    ///
    /// // 检查是否转换到禁用状态
    /// if audit.transitioned_to_disabled() {
    ///     // 处理交易禁用逻辑
    /// }
    /// ```
    pub fn update(&mut self, update: TradingState) -> TradingStateUpdateAudit {
        let prev = *self;
        let next = match (*self, update) {
            (TradingState::Enabled, TradingState::Disabled) => {
                info!("EngineState setting TradingState::Disabled");
                TradingState::Disabled
            }
            (TradingState::Disabled, TradingState::Enabled) => {
                info!("EngineState setting TradingState::Enabled");
                TradingState::Enabled
            }
            (TradingState::Enabled, TradingState::Enabled) => {
                info!("EngineState set TradingState::Enabled, although it was already enabled");
                TradingState::Enabled
            }
            (TradingState::Disabled, TradingState::Disabled) => {
                info!("EngineState set TradingState::Disabled, although it was already disabled");
                TradingState::Disabled
            }
        };

        *self = next;

        TradingStateUpdateAudit {
            prev,
            current: next,
        }
    }
}

/// [`TradingState`] 更新的审计记录，包含之前和当前的状态。
///
/// TradingStateUpdateAudit 使上游组件能够确定 [`TradingState`] 是否以及如何发生了变化。
/// 这对于需要响应状态转换的组件（如策略、风险管理系统）非常有用。
///
/// # 使用场景
///
/// - 检测交易状态的转换
/// - 在状态转换时执行特定逻辑
/// - 记录状态变更历史
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TradingStateUpdateAudit {
    /// 之前的状态
    pub prev: TradingState,
    /// 当前的状态
    pub current: TradingState,
}

impl TradingStateUpdateAudit {
    /// 仅当之前的状态不是 `Disabled`，且新状态是 `Disabled` 时返回 `true`。
    ///
    /// 此方法用于检测是否转换到禁用状态，这对于需要响应交易禁用的组件非常有用。
    ///
    /// # 返回值
    ///
    /// - `true`: 如果从启用状态转换到禁用状态
    /// - `false`: 其他情况
    ///
    /// # 使用示例
    ///
    /// ```rust,ignore
    /// let audit = trading_state.update(TradingState::Disabled);
    /// if audit.transitioned_to_disabled() {
    ///     // 处理交易禁用逻辑，例如取消所有未成交订单
    ///     cancel_all_orders();
    /// }
    /// ```
    pub fn transitioned_to_disabled(&self) -> bool {
        self.current == TradingState::Disabled && self.prev != TradingState::Disabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestCase {
        name: &'static str,
        initial: TradingState,
        update: TradingState,
        expected_state: TradingState,
        expected_audit: TradingStateUpdateAudit,
    }

    #[test]
    fn test_trading_state_update() {
        let test_cases = vec![
            TestCase {
                name: "Enable when disabled",
                initial: TradingState::Disabled,
                update: TradingState::Enabled,
                expected_state: TradingState::Enabled,
                expected_audit: TradingStateUpdateAudit {
                    prev: TradingState::Disabled,
                    current: TradingState::Enabled,
                },
            },
            TestCase {
                name: "Disable when enabled",
                initial: TradingState::Enabled,
                update: TradingState::Disabled,
                expected_state: TradingState::Disabled,
                expected_audit: TradingStateUpdateAudit {
                    prev: TradingState::Enabled,
                    current: TradingState::Disabled,
                },
            },
            TestCase {
                name: "Enable when already enabled",
                initial: TradingState::Enabled,
                update: TradingState::Enabled,
                expected_state: TradingState::Enabled,
                expected_audit: TradingStateUpdateAudit {
                    prev: TradingState::Enabled,
                    current: TradingState::Enabled,
                },
            },
            TestCase {
                name: "Disable when already disabled",
                initial: TradingState::Disabled,
                update: TradingState::Disabled,
                expected_state: TradingState::Disabled,
                expected_audit: TradingStateUpdateAudit {
                    prev: TradingState::Disabled,
                    current: TradingState::Disabled,
                },
            },
        ];

        for test in test_cases {
            let mut state = test.initial;
            let audit = state.update(test.update);

            assert_eq!(
                state, test.expected_state,
                "Failed test '{}': state mismatch",
                test.name
            );

            assert_eq!(
                audit.prev, test.expected_audit.prev,
                "Failed test '{}': audit prev state mismatch",
                test.name
            );

            assert_eq!(
                audit.current, test.expected_audit.current,
                "Failed test '{}': audit current state mismatch",
                test.name
            );
        }
    }

    #[test]
    fn test_trading_state_update_audit_transition_to_disabled() {
        let test_cases = vec![
            TestCase {
                name: "Detect transition to disabled from enabled",
                initial: TradingState::Enabled,
                update: TradingState::Disabled,
                expected_state: TradingState::Disabled,
                expected_audit: TradingStateUpdateAudit {
                    prev: TradingState::Enabled,
                    current: TradingState::Disabled,
                },
            },
            TestCase {
                name: "No transition detected when already disabled",
                initial: TradingState::Disabled,
                update: TradingState::Disabled,
                expected_state: TradingState::Disabled,
                expected_audit: TradingStateUpdateAudit {
                    prev: TradingState::Disabled,
                    current: TradingState::Disabled,
                },
            },
            TestCase {
                name: "No transition detected when enabling",
                initial: TradingState::Disabled,
                update: TradingState::Enabled,
                expected_state: TradingState::Enabled,
                expected_audit: TradingStateUpdateAudit {
                    prev: TradingState::Disabled,
                    current: TradingState::Enabled,
                },
            },
        ];

        for test in test_cases {
            let mut state = test.initial;
            let audit = state.update(test.update);

            let expected_transition =
                audit.prev != TradingState::Disabled && audit.current == TradingState::Disabled;

            assert_eq!(
                audit.transitioned_to_disabled(),
                expected_transition,
                "Failed test '{}': transition detection incorrect",
                test.name
            );
        }
    }
}
