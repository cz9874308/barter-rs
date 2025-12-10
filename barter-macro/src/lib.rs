//! Barter-Macro 过程宏模块
//!
//! 本模块提供了用于序列化和反序列化 Exchange 和 SubscriptionKind 类型的过程宏。
//!
//! # 提供的宏
//!
//! - **DeExchange**: 为 Exchange 类型生成反序列化实现
//! - **SerExchange**: 为 Exchange 类型生成序列化实现
//! - **DeSubKind**: 为 SubscriptionKind 类型生成反序列化实现
//! - **SerSubKind**: 为 SubscriptionKind 类型生成序列化实现

extern crate proc_macro;

use convert_case::{Boundary, Case, Casing};
use proc_macro::TokenStream;
use quote::quote;
use syn::DeriveInput;

/// 为 Exchange 类型生成反序列化实现。
///
/// 此宏为 Exchange 类型生成 `serde::Deserialize` 实现。
/// 它期望输入字符串与 Exchange 的 ID 匹配。
///
/// # 使用示例
///
/// ```rust,ignore
/// #[derive(DeExchange)]
/// pub struct BinanceSpot;
/// ```
#[proc_macro_derive(DeExchange)]
pub fn de_exchange_derive(input: TokenStream) -> TokenStream {
    // 使用 Syn 从 TokenStream 解析 Rust 代码抽象语法树 -> DeriveInput
    let ast: DeriveInput =
        syn::parse(input).expect("de_exchange_derive() failed to parse input TokenStream");

    // 确定 Exchange 名称
    let exchange = &ast.ident;

    let generated = quote! {
        impl<'de> serde::Deserialize<'de> for #exchange {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::de::Deserializer<'de>
            {
                let input = <String as serde::Deserialize>::deserialize(deserializer)?;
                let exchange = #exchange::ID;
                let expected = exchange.as_str();

                if input.as_str() == expected {
                    Ok(Self::default())
                } else {
                    Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Str(input.as_str()),
                        &expected
                    ))
                }
            }
        }
    };

    TokenStream::from(generated)
}

/// 为 Exchange 类型生成序列化实现。
///
/// 此宏为 Exchange 类型生成 `serde::Serialize` 实现。
/// 它将 Exchange 序列化为其 ID 字符串。
///
/// # 使用示例
///
/// ```rust,ignore
/// #[derive(SerExchange)]
/// pub struct BinanceSpot;
/// ```
#[proc_macro_derive(SerExchange)]
pub fn ser_exchange_derive(input: TokenStream) -> TokenStream {
    // 使用 Syn 从 TokenStream 解析 Rust 代码抽象语法树 -> DeriveInput
    let ast: DeriveInput =
        syn::parse(input).expect("ser_exchange_derive() failed to parse input TokenStream");

    // 确定 Exchange
    let exchange = &ast.ident;

    let generated = quote! {
        impl serde::Serialize for #exchange {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::ser::Serializer,
            {
                serializer.serialize_str(#exchange::ID.as_str())
            }
        }
    };

    TokenStream::from(generated)
}

/// 为 SubscriptionKind 类型生成反序列化实现。
///
/// 此宏为 SubscriptionKind 类型生成 `serde::Deserialize` 实现。
/// 它将类型名称从 PascalCase 转换为 snake_case 进行匹配。
///
/// # 使用示例
///
/// ```rust,ignore
/// #[derive(DeSubKind)]
/// pub struct PublicTrades;
/// ```
#[proc_macro_derive(DeSubKind)]
pub fn de_sub_kind_derive(input: TokenStream) -> TokenStream {
    // 使用 Syn 从 TokenStream 解析 Rust 代码抽象语法树 -> DeriveInput
    let ast: DeriveInput =
        syn::parse(input).expect("de_sub_kind_derive() failed to parse input TokenStream");

    // 确定 SubKind 名称
    let sub_kind = &ast.ident;

    // 将 PascalCase 转换为 snake_case
    let expected_sub_kind = sub_kind
        .to_string()
        .from_case(Case::Pascal)
        .without_boundaries(&Boundary::letter_digit())
        .to_case(Case::Snake);

    let generated = quote! {
        impl<'de> serde::Deserialize<'de> for #sub_kind {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::de::Deserializer<'de>
            {
                let input = <String as serde::Deserialize>::deserialize(deserializer)?;

                if input == #expected_sub_kind {
                    Ok(Self)
                } else {
                    Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Str(input.as_str()),
                        &#expected_sub_kind
                    ))
                }
            }
        }
    };

    TokenStream::from(generated)
}

/// 为 SubscriptionKind 类型生成序列化实现。
///
/// 此宏为 SubscriptionKind 类型生成 `serde::Serialize` 实现。
/// 它将类型名称转换为 snake_case 字符串。
///
/// # 使用示例
///
/// ```rust,ignore
/// #[derive(SerSubKind)]
/// pub struct PublicTrades;
/// ```
#[proc_macro_derive(SerSubKind)]
pub fn ser_sub_kind_derive(input: TokenStream) -> TokenStream {
    // 使用 Syn 从 TokenStream 解析 Rust 代码抽象语法树 -> DeriveInput
    let ast: DeriveInput =
        syn::parse(input).expect("ser_sub_kind_derive() failed to parse input TokenStream");

    // 确定 SubKind 名称
    let sub_kind = &ast.ident;
    // 将类型名称转换为 snake_case
    let sub_kind_string = sub_kind.to_string().to_case(Case::Snake);
    let sub_kind_str = sub_kind_string.as_str();

    let generated = quote! {
        impl serde::Serialize for #sub_kind {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::ser::Serializer,
            {
                serializer.serialize_str(#sub_kind_str)
            }
        }
    };

    TokenStream::from(generated)
}
