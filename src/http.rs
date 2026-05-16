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
        let res: ExecResponse = self
            .agent
            .post(self.base_url.clone() + endpoint)
            .header("Authorization", self.auth_header())
            .send_json(&body)?
            .body_mut()
            .read_json()?;

        return Ok(res);
    }

    fn get(&self, endpoint: &str) -> RequestBuilder<WithoutBody> {
        self.agent
            .get(self.base_url.clone() + endpoint)
            .header("Authorization", self.auth_header())
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.token)
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
