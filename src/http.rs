use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{collections::HashMap, time::Duration};
use ureq::{Agent, Error, RequestBuilder, typestate::WithoutBody};

pub(crate) struct Client {
    agent: Agent,
    base_url: String,
    token: String,
}

impl Client {
    pub(crate) fn new(base_url: String) -> Self {
        let config = Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(10)))
            .build();

        Self {
            base_url,
            agent: config.into(),
            token: String::new(),
        }
    }

    pub(crate) fn with_token(mut self, token: String) -> Self {
        self.token = token;
        self
    }

    pub(crate) fn get_response<T: serde::de::DeserializeOwned>(
        self,
        endpoint: &str,
    ) -> Result<T, Error> {
        let res = self.get(endpoint).call()?.body_mut().read_json::<T>();

        {
            let r = res?;
            Ok(r)
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
        let res: ExecResponse = req.send_json(body)?.body_mut().read_json()?;
        Ok(res)
    }

    /// 任意の JSON body を POST して JSON を受け取る (scenarios/run 用)。
    pub(crate) fn post_json<T: serde::de::DeserializeOwned>(
        &self,
        endpoint: &str,
        body: &Value,
    ) -> Result<T, Error> {
        let mut req = self.agent.post(self.base_url.clone() + endpoint);
        req = req.header("Accept", "application/json");
        if let Some(h) = self.auth_header() {
            req = req.header("Authorization", h);
        }
        req.send_json(body)?.body_mut().read_json::<T>()
    }

    /// scenarios 一覧の取得 (&self で呼べる版。get_response は self を消費するため)。
    pub(crate) fn get_scenarios(&self, endpoint: &str) -> Result<ScenariosResponse, Error> {
        self.get(endpoint).call()?.body_mut().read_json()
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

/// discovery 中の `/health` probe タイムアウト (SPEC §2)。立っていないポートに長く待たない。
pub(crate) const PROBE_TIMEOUT: Duration = Duration::from_millis(400);

/// `--base-url` 明示時の best-effort probe タイムアウト (SPEC §2)。
pub(crate) const BASE_URL_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// `127.0.0.1:{port}` の `/health` を叩く。
/// 2xx かつ JSON object のときだけ Some。それ以外 (非 JSON / 配列 / 4xx / 接続不可 / タイムアウト) は None。
pub(crate) fn probe_port(port: u16, timeout: Duration) -> Option<Map<String, Value>> {
    probe_url(&format!("http://127.0.0.1:{port}"), timeout)
}

/// base_url 指定版の probe。判定規則は probe_port と同じ。
pub(crate) fn probe_url(base_url: &str, timeout: Duration) -> Option<Map<String, Value>> {
    let agent: Agent = Agent::config_builder()
        .timeout_global(Some(timeout))
        .build()
        .into();
    let mut res = agent
        .get(format!("{base_url}/api/v1/health"))
        .header("Accept", "application/json")
        .call()
        .ok()?;
    // 2xx 以外は ureq がエラーにするが、明示的にも弾いておく。
    if !(200..300).contains(&res.status().as_u16()) {
        return None;
    }
    match res.body_mut().read_json::<Value>() {
        Ok(Value::Object(m)) => Some(m),
        _ => None,
    }
}

#[allow(unused)]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HealthResponse {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub version: String,
    // mode / projectName / projectPath は古いサーバだと欠けることがあるので既定値で埋める。
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub project_name: String,
    #[serde(default)]
    pub project_path: String,
    #[serde(default)]
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

/// `POST /api/v1/scenarios/run` のレスポンス (SPEC §6)。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioRunResponse {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub duration_ms: f64,
    #[serde(default)]
    pub failed_at_step: Option<i64>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub already_running: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub steps: Vec<ScenarioStepResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioStepResult {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub duration_ms: f64,
    #[serde(default)]
    pub error: Option<String>,
    // kind ごとに付いたり付かなかったりするフィールドは素の JSON のまま保持する。
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl ScenarioStepResult {
    /// extra から文字列として取り出す (数値や bool も文字列化する)。
    pub(crate) fn extra_str(&self, key: &str) -> Option<String> {
        match self.extra.get(key)? {
            Value::Null => None,
            Value::String(s) => Some(s.clone()),
            other => Some(other.to_string()),
        }
    }
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
// テスト名は日本語で書く方針のため、snake_case 検査から除外する
#[allow(non_snake_case)]
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
