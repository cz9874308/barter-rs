// 允许 dev-dependencies 中的未使用 extern crate 警告
// 这些依赖仅在示例/测试/基准测试中使用，不在库代码中使用
#![allow(unused_extern_crates)]
#![forbid(unsafe_code)]
#![warn(
    unused,
    clippy::cognitive_complexity,
    unused_crate_dependencies,
    clippy::unused_self,
    clippy::useless_let_if_seq,
    missing_debug_implementations,
    rust_2018_idioms,
    rust_2024_compatibility
)]
#![allow(clippy::type_complexity, clippy::too_many_arguments, type_alias_bounds)]

//! # Barter-Integration
//! 高性能、低级别的框架，用于构建灵活的 Web 集成。
//!
//! 被其他 Barter 交易生态系统 crate 使用，用于构建稳健的金融执行集成，
//! 主要用于公共数据收集和交易执行。它的特点是：
//! * **低级别**: 使用任意数据转换将通过网络通信的原始数据流转换为任何所需的数据模型。
//! * **灵活**: 兼容任何协议（WebSocket、FIX、Http 等）、任何输入/输出模型和任何用户定义的转换。
//!
//! ## 核心抽象：
//! - **RestClient**: 提供客户端和服务器之间可配置的签名 Http 通信。
//! - **ExchangeStream**: 提供通过任何异步流协议（WebSocket、FIX 等）的可配置通信。
//!
//! 这两个核心抽象提供了在服务器和客户端数据模型之间方便转换所需的稳健粘合剂。

use crate::error::SocketError;
use serde::{Deserialize, Serialize};

/// Barter-Integration 中生成的所有 [`Error`](std::error::Error)。
pub mod error;

/// 包含用于将通信协议特定消息转换为通用输出数据结构的 `StreamParser` 实现。
pub mod protocol;

/// 包含用于通用表示实时指标的灵活 `Metric` 类型。
pub mod metric;

/// 用于辅助反序列化的工具。
pub mod de;

/// 定义 [`SubscriptionId`](subscription::SubscriptionId) 新类型，表示已订阅的
/// 数据流（市场数据、账户数据）的唯一 `SmolStr` 标识符。
pub mod subscription;

/// 定义不同通道类型的 Trait [`Tx`](channel::Tx) 抽象，以及其他通道工具。
///
/// 例如：`UnboundedTx`、`ChannelTxDroppable` 等。
pub mod channel;

/// 集合工具。
pub mod collection;

/// 流工具。
pub mod stream;

/// 快照工具。
pub mod snapshot;

/// [`Validator`] 能够确定其内部状态是否足以满足实现者定义的某些用例。
///
/// Validator Trait 用于验证对象的状态是否满足特定要求。
pub trait Validator {
    /// 检查 `Self` 是否对某些用例有效。
    ///
    /// # 返回值
    ///
    /// 如果有效，返回 `Ok(Self)`；否则返回错误。
    fn validate(self) -> Result<Self, SocketError>
    where
        Self: Sized;
}

/// [`Transformer`] 能够将任何 `Input` 转换为 `Result<Self::Output, Self::Error>` 的迭代器。
///
/// Transformer Trait 用于将一种数据格式转换为另一种格式。
/// 它支持状态转换和错误处理。
///
/// ## 关联类型
///
/// - `Error`: 转换过程中可能发生的错误类型
/// - `Input`: 输入数据类型
/// - `Output`: 输出数据类型
/// - `OutputIter`: 输出迭代器类型
pub trait Transformer {
    /// 转换错误类型。
    type Error;
    /// 输入数据类型。
    type Input;
    /// 输出数据类型。
    type Output;
    /// 输出迭代器类型。
    type OutputIter: IntoIterator<Item = Result<Self::Output, Self::Error>>;

    /// 将输入转换为输出迭代器。
    ///
    /// # 参数
    ///
    /// - `input`: 要转换的输入数据
    ///
    /// # 返回值
    ///
    /// 返回包含转换结果的迭代器。
    fn transform(&mut self, input: Self::Input) -> Self::OutputIter;
}

/// 确定某物是否被认为是"不可恢复的"，例如不可恢复的错误。
///
/// 注意，[`Unrecoverable`] 的含义可能因上下文而异。
pub trait Unrecoverable {
    /// 检查是否不可恢复。
    ///
    /// # 返回值
    ///
    /// 如果不可恢复，返回 `true`；否则返回 `false`。
    fn is_unrecoverable(&self) -> bool;
}

/// Trait，用于表示某物是否是终端的（例如，需要关闭或重启）。
///
/// Terminal Trait 用于标记需要终止操作的状态或事件。
pub trait Terminal {
    /// 检查是否是终端状态。
    ///
    /// # 返回值
    ///
    /// 如果是终端状态，返回 `true`；否则返回 `false`。
    fn is_terminal(&self) -> bool;
}

/// 表示 `Iterator` 或 `Stream` 已结束。
///
/// FeedEnded 用于标记数据流或迭代器已结束。
#[derive(
    Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Deserialize, Serialize,
)]
pub struct FeedEnded;
