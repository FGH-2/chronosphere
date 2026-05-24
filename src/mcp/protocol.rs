//! JSON-RPC 2.0 + MCP protocol envelope types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct Request {
    #[allow(dead_code)]
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct Response {
    pub jsonrpc: &'static str,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorObject>,
}

#[derive(Debug, Serialize)]
pub struct ErrorObject {
    pub code: i64,
    pub message: String,
}

impl Response {
    pub fn ok(id: Option<Value>, result: Value) -> Self {
        Response {
            jsonrpc: "2.0",
            id: id.unwrap_or(Value::Null),
            result: Some(result),
            error: None,
        }
    }
    pub fn error<S: Into<String>>(id: Option<Value>, code: i64, message: S) -> Self {
        Response {
            jsonrpc: "2.0",
            id: id.unwrap_or(Value::Null),
            result: None,
            error: Some(ErrorObject {
                code,
                message: message.into(),
            }),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ToolCallParams {
    pub name: String,
    #[serde(default)]
    pub arguments: Option<Value>,
}

/// Domain error returned from tool handlers; converted to JSON-RPC error envelope or
/// to a `{ "isError": true, "content": [...] }` tool-call result depending on context.
#[derive(Debug)]
pub struct McpError {
    pub code: i64,
    pub message: String,
}

impl McpError {
    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("method not found: {}", method),
        }
    }
    #[allow(dead_code)]
    pub fn invalid_args<S: Into<String>>(msg: S) -> Self {
        Self {
            code: -32602,
            message: msg.into(),
        }
    }
    pub fn internal<S: Into<String>>(msg: S) -> Self {
        Self {
            code: -32603,
            message: msg.into(),
        }
    }
}

impl From<anyhow::Error> for McpError {
    fn from(e: anyhow::Error) -> Self {
        McpError::internal(format!("{:#}", e))
    }
}
