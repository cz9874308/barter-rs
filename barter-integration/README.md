# Barter-Integration

用于构建灵活 Web 集成的高性能、底层框架。

被其他 [`Barter`] 交易生态系统 crate 用于构建强大的金融交易所集成，主要用于公共数据收集和交易执行。特点：

-   **底层**：使用任意数据转换将通过网络通信的原始数据流转换为任何所需的数据模型。
-   **灵活**：兼容任何协议（WebSocket、FIX、Http 等）、任何输入/输出模型以及任何用户定义的转换。

核心抽象包括：

-   **RestClient** 提供客户端和服务器之间可配置的签名 Http 通信。
-   **ExchangeStream** 提供通过任何异步流协议（WebSocket、FIX 等）的可配置通信。

这两个核心抽象提供了您所需的强大粘合剂，可以方便地在服务器和客户端数据模型之间进行转换。

**请参阅：[`Barter`]、[`Barter-Data`] 和 [`Barter-Execution`]**

[![Crates.io][crates-badge]][crates-url]
[![MIT licensed][mit-badge]][mit-url]
[![Build Status][actions-badge]][actions-url]
[![Discord chat][discord-badge]][discord-url]

[crates-badge]: https://img.shields.io/crates/v/barter-integration.svg
[crates-url]: https://crates.io/crates/barter-integration
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: https://gitlab.com/open-source-keir/financial-modelling/trading/barter-integration-rs/-/blob/main/LICENCE
[actions-badge]: https://gitlab.com/open-source-keir/financial-modelling/trading/barter-integration-rs/badges/-/blob/main/pipeline.svg
[actions-url]: https://gitlab.com/open-source-keir/financial-modelling/trading/barter-integration-rs/-/commits/main
[discord-badge]: https://img.shields.io/discord/910237311332151317.svg?logo=discord&style=flat-square
[discord-url]: https://discord.gg/wE7RqhnQMV

[API Documentation] | [Chat]

[`Barter`]: https://crates.io/crates/barter
[`Barter-Data`]: https://crates.io/crates/barter-data
[`Barter-Execution`]: https://crates.io/crates/barter-execution
[API Documentation]: https://docs.rs/barter-data/latest/barter_integration
[Chat]: https://discord.gg/wE7RqhnQMV

## 概述

Barter-Integration 是一个用于构建灵活 Web 集成的高性能、底层、可配置框架。

### RestClient

**（同步私有和公共 Http 通信）**

从高层次来看，`RestClient` 有几个主要组件，使其能够执行 `RestRequest`：

-   在目标 API 上具有可配置签名逻辑的 `RequestSigner`。
-   将 API 特定响应转换为所需输出类型的 `HttpParser`。

### ExchangeStream

**（使用 WebSocket 和 FIX 等流协议的异步通信）**

从高层次来看，`ExchangeStream` 由几个主要组件组成：

-   内部 Stream/Sink 套接字（例如 WebSocket、FIX 等）。
-   能够将输入协议消息（例如 WebSocket、FIX 等）解析为交易所特定消息的 `StreamParser`。
-   将交易所特定消息转换为所需输出类型迭代器的 `Transformer`。

## 示例

#### 使用签名 GET 请求获取 Ftx 账户余额：

```rust,no_run
use std::borrow::Cow;

use barter_integration::{
    error::SocketError,
    metric::Tag,
    model::Symbol,
    protocol::http::{
        private::{encoder::HexEncoder, RequestSigner, Signer},
        rest::{client::RestClient, RestRequest},
        HttpParser,
    },
};
use bytes::Bytes;
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use reqwest::{RequestBuilder, StatusCode};
use serde::Deserialize;
use thiserror::Error;
use tokio::sync::mpsc;

struct FtxSigner {
    api_key: String,
}

// 为每个 Ftx `RestRequest` 签名所需的配置
struct FtxSignConfig<'a> {
    api_key: &'a str,
    time: DateTime<Utc>,
    method: reqwest::Method,
    path: Cow<'static, str>,
}

impl Signer for FtxSigner {
    type Config<'a> = FtxSignConfig<'a> where Self: 'a;

    fn config<'a, Request>(
        &'a self,
        request: Request,
        _: &RequestBuilder,
    ) -> Result<Self::Config<'a>, SocketError>
    where
        Request: RestRequest,
    {
        Ok(FtxSignConfig {
            api_key: self.api_key.as_str(),
            time: Utc::now(),
            method: Request::method(),
            path: request.path(),
        })
    }

    fn add_bytes_to_sign<M>(mac: &mut M, config: &Self::Config<'a>) -> Bytes
    where
        M: Mac
    {
        mac.update(config.time.to_string().as_bytes());
        mac.update(config.method.as_str().as_bytes());
        mac.update(config.path.as_bytes());
    }

    fn build_signed_request<'a>(
        config: Self::Config<'a>,
        builder: RequestBuilder,
        signature: String,
    ) -> Result<reqwest::Request, SocketError> {
        // 添加 Ftx 所需的请求头并构建 reqwest::Request
        builder
            .header("FTX-KEY", config.api_key)
            .header("FTX-TS", &config.time.timestamp_millis().to_string())
            .header("FTX-SIGN", &signature)
            .build()
            .map_err(SocketError::from)
    }
}

struct FtxParser;

impl HttpParser for FtxParser {
    type ApiError = serde_json::Value;
    type OutputError = ExecutionError;

    fn parse_api_error(&self, status: StatusCode, api_error: Self::ApiError) -> Self::OutputError {
        // 为简单起见，使用 serde_json::Value 作为错误并提取原始字符串进行解析
        let error = api_error.to_string();

        // 解析 Ftx 错误消息以确定自定义 ExecutionError 变体
        match error.as_str() {
            message if message.contains("Invalid login credentials") => {
                ExecutionError::Unauthorised(error)
            }
            _ => ExecutionError::Socket(SocketError::HttpResponse(status, error)),
        }
    }
}

#[derive(Debug, Error)]
enum ExecutionError {
    #[error("request authorisation invalid: {0}")]
    Unauthorised(String),

    #[error("SocketError: {0}")]
    Socket(#[from] SocketError),
}

struct FetchBalancesRequest;

impl RestRequest for FetchBalancesRequest {
    type Response = FetchBalancesResponse; // 定义响应类型
    type QueryParams = (); // FetchBalances 不需要任何 QueryParams
    type Body = (); // FetchBalances 不需要任何 Body

    fn path(&self) -> Cow<'static, str> {
        Cow::Borrowed("/api/wallet/balances")
    }

    fn method() -> reqwest::Method {
        reqwest::Method::GET
    }
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct FetchBalancesResponse {
    success: bool,
    result: Vec<FtxBalance>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct FtxBalance {
    #[serde(rename = "coin")]
    symbol: Symbol,
    total: f64,
}

/// 请参阅 Barter-Execution 以获取全面的真实示例，以及您可以直接使用的代码
/// 在多个交易所执行交易。
#[tokio::main]
async fn main() {
    // 用于签名私有 http 请求的 HMAC-SHA256 编码账户 API 密钥
    let mac: Hmac<sha2::Sha256> = Hmac::new_from_slice("api_secret".as_bytes()).unwrap();

    // 构建用于使用十六进制编码签名 http 请求的 Ftx 配置 RequestSigner
    let request_signer = RequestSigner::new(
        FtxSigner {
            api_key: "api_key".to_string(),
        },
        mac,
        HexEncoder,
    );

    // 使用 Ftx 配置构建 RestClient
    let rest_client = RestClient::new("https://ftx.com", request_signer, FtxParser);

    // 获取 Result<FetchBalancesResponse, ExecutionError>
    let _response = rest_client.execute(FetchBalancesRequest).await;
}
```

#### 消费 Binance 期货逐笔交易并计算成交量的滚动总和：

```rust,no_run
use barter_integration::{
    error::SocketError,
    protocol::websocket::{WebSocket, WebSocketSerdeParser, WsMessage},
    ExchangeStream, Transformer,
};
use futures::{SinkExt, StreamExt};
use serde::{de, Deserialize};
use serde_json::json;
use std::str::FromStr;
use tokio_tungstenite::connect_async;
use tracing::debug;

// 使用 tungstenite `WebSocket` 的 `ExchangeStream` 的便捷类型别名
type ExchangeWsStream<Exchange> = ExchangeStream<WebSocketSerdeParser, WebSocket, Exchange, VolumeSum>;

// 表示 Transformer 正在生成的 VolumeSum 的通信类型别名
type VolumeSum = f64;

#[derive(Deserialize)]
#[serde(untagged, rename_all = "camelCase")]
enum BinanceMessage {
    SubResponse {
        result: Option<Vec<String>>,
        id: u32,
    },
    Trade {
        #[serde(rename = "q", deserialize_with = "de_str")]
        quantity: f64,
    },
}

struct StatefulTransformer {
    sum_of_volume: VolumeSum,
}

impl Transformer<VolumeSum> for StatefulTransformer {
    type Input = BinanceMessage;
    type OutputIter = Vec<Result<VolumeSum, SocketError>>;

    fn transform(&mut self, input: Self::Input) -> Self::OutputIter {
        // 将新的输入交易数量添加到总和
        match input {
            BinanceMessage::SubResponse { result, id } => {
                debug!("Received SubResponse for {}: {:?}", id, result);
                // 此示例中不关心这个
            }
            BinanceMessage::Trade { quantity, .. } => {
                // 将新的交易成交量添加到内部状态 VolumeSum
                self.sum_of_volume += quantity;
            }
        };

        // 返回长度为 1 的迭代器，包含成交量的运行总和
        vec![Ok(self.sum_of_volume)]
    }
}

/// 请参阅 Barter-Data 以获取全面的真实示例，以及您可以直接使用的代码
/// 从多个交易所收集实时公共市场数据。
#[tokio::main]
async fn main() {
    // 与所需的 WebSocket 服务器建立 Sink/Stream 通信
    let mut binance_conn = connect_async("wss://fstream.binance.com/ws/")
        .await
        .map(|(ws_conn, _)| ws_conn)
        .expect("failed to connect");

    // 通过套接字发送内容（例如 Binance 交易订阅）
    binance_conn
        .send(WsMessage::Text(
            json!({"method": "SUBSCRIBE","params": ["btcusdt@aggTrade"],"id": 1}).to_string(),
        ))
        .await
        .expect("failed to send WsMessage over socket");

    // 实例化一些任意 Transformer 以应用于从 WebSocket 协议解析的数据
    let transformer = StatefulTransformer { sum_of_volume: 0.0 };

    // ExchangeWsStream 包括预定义的 WebSocket Sink/Stream 和 WebSocket StreamParser
    let mut ws_stream = ExchangeWsStream::new(binance_conn, transformer);

    // 从 ExchangeStream 接收所需输出数据模型的流
    while let Some(volume_result) = ws_stream.next().await {
        match volume_result {
            Ok(cumulative_volume) => {
                // 对您的数据执行某些操作
                println!("{cumulative_volume:?}");
            }
            Err(error) => {
                // 响应内部转换产生的任何错误
                eprintln!("{error}")
            }
        }
    }
}

/// 将 `String` 反序列化为所需类型。
fn de_str<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: de::Deserializer<'de>,
    T: FromStr,
    T::Err: std::fmt::Display,
{
    let data: String = Deserialize::deserialize(deserializer)?;
    data.parse::<T>().map_err(de::Error::custom)
}
```

#### 解析二进制 protobuf 消息

`WebSocketProtobufParser` 可以使用 [`prost`] 解码 `WsMessage::Binary` 负载。当服务器发送
protobuf 编码消息时，可以在 `ExchangeStream` 中使用它来代替 `WebSocketSerdeParser`。

```rust
use barter_integration::protocol::websocket::{WebSocket, WebSocketProtobufParser};
use barter_integration::ExchangeStream;

type ProtoStream<Exchange> = ExchangeStream<WebSocketProtobufParser, WebSocket, Exchange, ()>;
```

[`prost`]: https://crates.io/crates/prost

**有关更大的"真实世界"示例，请参阅 [`Barter-Data`] 仓库。**

## 获取帮助

首先，请查看[API 文档][API Documentation]中是否已有您问题的答案。如果找不到答案，我很乐意通过[聊天][Chat]在 Discord 上帮助您并尝试回答您的问题。

## 贡献

感谢您帮助改进 Barter 生态系统！请通过 Discord 联系我们，讨论开发、新功能和未来路线图。

## 相关项目

除了 Barter-Integration crate 之外，Barter 项目还维护：

-   [`Barter`]：高性能、可扩展和模块化的交易组件，开箱即用。包含一个
    预构建的交易引擎，可用作实盘交易或回测系统。
-   [`Barter-Data`]：用于从领先的加密货币交易所流式传输公共数据的高性能 WebSocket 集成库。
-   [`Barter-Execution`]：用于交易执行的金融交易所集成 - 尚未发布！

## 路线图

-   添加新的默认 StreamParser 实现，以支持与其他流行系统（如 Kafka）的集成。

## 许可证

本项目采用 [MIT 许可证][MIT license]。

[MIT license]: https://gitlab.com/open-source-keir/financial-modelling/trading/barter-data-rs/-/blob/main/LICENSE

### 贡献

除非您明确声明，否则您有意提交以包含在 Barter-Integration 中的任何贡献均应
采用 MIT 许可证，不提供任何附加条款或条件。
