use serde::Deserialize;
use std::time::Duration;
use ureq::{Agent, RequestBuilder, typestate::WithoutBody};

pub(crate) const DEFAULT_URL: &str = "http://127.0.0.1:7610";

#[allow(unused)]
#[derive(Debug, Deserialize)]
pub(crate) struct HealthBody {
    status: String,
    version: String,
    // mode: String,
    // project_name: String,
    // project_path: String,
    // command_count: u32,
}

#[allow(unused)]
#[derive(Debug, Deserialize)]
pub(crate) struct CommandsBody {
    names: Vec<String>,
}

pub(crate) fn get(url: String) -> RequestBuilder<WithoutBody> {
    let config = Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .build();

    let agent: Agent = config.into();

    agent.get(url)
}
