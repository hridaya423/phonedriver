//! JSON-RPC 2.0 framing, one object per line (`docs/protocol.md` section 1).

use phonedriver_core::error::Error;
use serde::Deserialize;
use serde_json::{Value, json};

pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const VERSION_MISMATCH: i32 = -32010;

#[derive(Debug, Clone, PartialEq)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    pub data: Value,
}

impl RpcError {
    pub fn new(code: i32, message: impl Into<String>, data: Value) -> RpcError {
        RpcError {
            code,
            message: message.into(),
            data,
        }
    }

    pub fn invalid_params(message: impl Into<String>) -> RpcError {
        RpcError::new(INVALID_PARAMS, message, Value::Null)
    }

    pub fn to_json(&self) -> Value {
        json!({"code": self.code, "message": self.message, "data": self.data})
    }
}

impl From<Error> for RpcError {
    fn from(e: Error) -> RpcError {
        let v = e.to_rpc();
        RpcError {
            code: e.rpc_code(),
            message: e.to_string(),
            data: v["data"].clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Request {
    #[serde(default)]
    pub id: Value,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

pub fn parse(line: &str) -> Result<Request, RpcError> {
    let v: Value = serde_json::from_str(line)
        .map_err(|e| RpcError::new(PARSE_ERROR, format!("not JSON: {e}"), Value::Null))?;
    serde_json::from_value(v).map_err(|e| {
        RpcError::new(
            INVALID_REQUEST,
            format!("not a JSON-RPC request: {e}"),
            Value::Null,
        )
    })
}

pub fn reply(id: &Value, result: Result<Value, RpcError>) -> String {
    let body = match result {
        Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
        Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": e.to_json()}),
    };
    body.to_string()
}

pub fn params<T: serde::de::DeserializeOwned>(v: &Value) -> Result<T, RpcError> {
    let v = if v.is_null() { json!({}) } else { v.clone() };
    serde_json::from_value(v).map_err(|e| RpcError::invalid_params(e.to_string()))
}
