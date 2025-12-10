//! RiskManager 检查模块
//!
//! 本模块定义了风险管理器的检查接口和工具函数，用于实现各种风险检查逻辑。
//!
//! # 核心概念
//!
//! - **RiskCheck**: Trait，定义风险检查接口
//! - **CheckHigherThan**: 检查值是否超过上限的简单实现
//! - **工具函数**: 计算名义价值、价格差异等

use derive_more::Constructor;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 辅助 RiskManager 检查的工具函数。
///
/// 例如，计算名义价值、价格差异等。
pub mod util;

/// 实现简单 RiskManager 检查的通用接口。
///
/// RiskCheck 定义了风险检查的标准接口，可以用于实现各种类型的风险检查。
/// 例如价格检查、数量检查、敞口检查等。
///
/// ## 关联类型
///
/// - **Input**: 被验证的数据类型（例如，`Decimal` 用于价格检查）
/// - **Error**: 验证失败时返回的错误类型
///
/// ## 实现示例
///
/// 参见 [`CheckHigherThan`] 作为简单示例。
///
/// # 使用示例
///
/// ```rust,ignore
/// struct MyRiskCheck {
///     limit: Decimal,
/// }
///
/// impl RiskCheck for MyRiskCheck {
///     type Input = Decimal;
///     type Error = MyCheckError;
///
///     fn name() -> &'static str {
///         "MyRiskCheck"
///     }
///
///     fn check(&self, input: &Self::Input) -> Result<(), Self::Error> {
///         // 实现检查逻辑
///     }
/// }
/// ```
pub trait RiskCheck {
    /// 被验证的数据类型。
    type Input;
    /// 验证失败时返回的错误类型。
    type Error;

    /// 返回风险检查的名称。
    ///
    /// 此方法用于日志记录和调试，帮助识别是哪个检查失败了。
    ///
    /// # 返回值
    ///
    /// 返回检查的静态名称字符串。
    fn name() -> &'static str;

    /// 对提供的 `Input` 执行风险检查。
    ///
    /// 此方法执行实际的风险检查逻辑。如果检查通过，返回 `Ok(())`；
    /// 如果检查失败，返回相应的错误。
    ///
    /// # 参数
    ///
    /// - `input`: 要检查的输入值
    ///
    /// # 返回值
    ///
    /// - `Ok(())`: 检查通过
    /// - `Err(Self::Error)`: 检查失败
    fn check(&self, input: &Self::Input) -> Result<(), Self::Error>;
}

/// 验证输入值是否超过上限的通用风险检查。
///
/// CheckHigherThan 是一个简单的风险检查实现，用于验证输入值是否超过预设的上限。
/// 如果输入值小于或等于上限，检查通过；否则检查失败。
///
/// ## 类型参数
///
/// - `T`: 要检查的值类型（必须实现 `Clone` 和 `PartialOrd`）
///
/// ## 使用场景
///
/// - 检查订单数量是否超过最大限制
/// - 检查价格是否超过最大允许价格
/// - 检查敞口是否超过最大允许敞口
/// - 等等
///
/// # 使用示例
///
/// ```rust,ignore
/// let check = CheckHigherThan { limit: Decimal::new(1000, 0) };
///
/// // 检查通过
/// assert!(check.check(&Decimal::new(500, 0)).is_ok());
///
/// // 检查失败
/// assert!(check.check(&Decimal::new(1500, 0)).is_err());
/// ```
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize, Constructor)]
pub struct CheckHigherThan<T> {
    /// 上限值；如果输入 <= limit，则检查通过。
    pub limit: T,
}

impl<T> RiskCheck for CheckHigherThan<T>
where
    T: Clone + PartialOrd,
{
    type Input = T;
    type Error = CheckFailHigherThan<T>;

    /// 返回检查名称。
    fn name() -> &'static str {
        "CheckHigherThan"
    }

    /// 执行检查。
    ///
    /// 如果输入值小于或等于上限，返回 `Ok(())`；否则返回错误。
    fn check(&self, input: &Self::Input) -> Result<(), Self::Error> {
        if *input <= self.limit {
            Ok(())
        } else {
            Err(CheckFailHigherThan {
                limit: self.limit.clone(),
                input: input.clone(),
            })
        }
    }
}

/// 当 [`CheckHigherThan`] 验证失败时返回的错误。
///
/// 此错误包含上限值和导致检查失败的输入值，用于日志记录和调试。
///
/// ## 类型参数
///
/// - `T`: 值类型
#[derive(
    Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize, Constructor, Error,
)]
#[error("CheckHigherThanFailed: input {input} > limit {limit}")]
pub struct CheckFailHigherThan<T> {
    /// 被超过的上限值。
    pub limit: T,
    /// 导致检查失败的输入值。
    pub input: T,
}
