use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, time::Duration};
use ureq::{Agent, Error, RequestBuilder, typestate::WithoutBody};

pub(crate) const DEFAULT_URL: &str = "http://127.0.0.1:7610";

pub(crate) struct Client {
    agent: Agent,
    base_url: String,
    token: String,
}

impl Client {
    pub(crate) fn new(base_url: String) -> Self {
        let config = Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(5)))
            .build();

        Self {
            base_url,
            agent: config.into(),
            token: String::new(),
        }
    }

    pub(crate) fn with_token(mut self, token: String) -> Self {
        self.token = token;
        return self;
    }

    pub(crate) fn get_response<T: serde::de::DeserializeOwned>(
        self,
        endpoint: &str,
    ) -> Result<T, Error> {
        let res = self.get(endpoint).call()?.body_mut().read_json::<T>();

        match res {
            Ok(r) => return Ok(r),
            Err(e) => return Err(e),
        }
    }

    pub(crate) fn post_exec(
        &self,
        endpoint: &str,
        body: &ExecRequest,
    ) -> Result<ExecResponse, Error> {
        let mut req = self.agent.post(self.base_url.clone() + endpoint);
        if let Some(h) = self.auth_header() {
            req = req.header("Authorization", h);
        }
        let res: ExecResponse = req.send_json(&body)?.body_mut().read_json()?;
        return Ok(res);
    }

    fn get(&self, endpoint: &str) -> RequestBuilder<WithoutBody> {
        let mut req = self.agent.get(self.base_url.clone() + endpoint);
        if let Some(h) = self.auth_header() {
            req = req.header("Authorization", h);
        }
        req
    }

    /// token が未設定なら Authorization ヘッダを送らない (SPEC §6: /health は認証不要)
    fn auth_header(&self) -> Option<String> {
        if self.token.is_empty() {
            None
        } else {
            Some(format!("Bearer {}", self.token))
        }
    }
}

#[allow(unused)]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HealthResponse {
    pub status: String,
    pub version: String,
    pub mode: String,
    pub project_name: String,
    pub project_path: String,
    pub command_count: u32,
}

#[derive(Debug, Serialize)]
pub(crate) struct ExecRequest {
    pub path: String,
    pub args: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExecResponse {
    pub success: bool,
    #[serde(default)]
    pub value: Option<Value>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub exception_type: Option<String>,
    #[serde(default)]
    pub stack_trace: Option<String>,
    pub duration_ms: f64,
    #[serde(default)]
    pub logs: Vec<LogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    #[serde(rename = "type")]
    pub r#type: String,
    pub message: String,
    #[serde(default)]
    pub stack_trace: Option<String>,
    pub timestamp: String,
}

#[allow(unused)]
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct CommandsResponse {
    pub commands: Vec<Command>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    pub path: String,
    pub name: String,
    pub category: String,
    #[serde(default)]
    pub description: Option<String>,
    pub is_async: bool,
    pub return_type: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub parameters: Vec<Parameter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Parameter {
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub position: u32,
    pub has_default: bool,
    #[serde(default)]
    pub default: Option<Value>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub choices: Option<Vec<Value>>,
}

#[allow(unused)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LogsResponse {
    pub invocations: Vec<Invocation>,
    pub total: u32,
    pub limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Invocation {
    pub path: String,
    pub timestamp: String,
    #[serde(default)]
    pub args: HashMap<String, Value>,
    pub result: ExecResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StateValue {
    pub path: String,
    #[serde(default)]
    pub value: Option<Value>,
    #[serde(rename = "type")]
    pub r#type: String,
}

#[allow(unused)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StateList {
    pub fields: Vec<StateField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateField {
    pub path: String,
    #[serde(default)]
    pub value: Option<Value>,
    #[serde(rename = "type")]
    pub r#type: String,
    pub instance_resolved: bool,
}

#[allow(unused)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenariosResponse {
    pub scenarios: Vec<Scenario>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scenario {
    pub path: String,
    #[serde(default)]
    pub description: Option<String>,
    pub step_count: i32,
}

/// クエリ文字列の値を percent-encode する (RFC 3986 unreserved 以外をエスケープ)
pub(crate) fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encode_英数字はそのまま() {
        assert_eq!(percent_encode("abc123"), "abc123");
    }

    #[test]
    fn percent_encode_unreserved_文字はそのまま() {
        // RFC 3986: ALPHA / DIGIT / "-" / "." / "_" / "~"
        assert_eq!(percent_encode("a-b_c.d~e"), "a-b_c.d~e");
    }

    #[test]
    fn percent_encode_スラッシュはエスケープ() {
        // クエリ値内では / もエスケープすべき (RFC 3986 reserved)
        assert_eq!(percent_encode("Player/Health"), "Player%2FHealth");
    }

    #[test]
    fn percent_encode_スペースはエスケープ() {
        assert_eq!(percent_encode("a b"), "a%20b");
    }

    #[test]
    fn percent_encode_等号とアンパサンドはエスケープ() {
        // ?key=value& の構造を壊さないため
        assert_eq!(percent_encode("a=b&c"), "a%3Db%26c");
    }

    #[test]
    fn percent_encode_マルチバイト_utf8() {
        // "あ" は UTF-8 で 0xE3 0x81 0x82
        assert_eq!(percent_encode("あ"), "%E3%81%82");
    }

    #[test]
    fn percent_encode_空文字() {
        assert_eq!(percent_encode(""), "");
    }
}
