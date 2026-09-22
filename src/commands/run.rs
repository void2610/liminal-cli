use std::io::Read;
use std::path::Path;

use anstream::println;
use anyhow::{Result, bail};
use serde_json::{Value, json};

use crate::cli::RunArgs;
use crate::commands::glob::{fnmatch, has_magic};
use crate::commands::junit::{self, Case};
use crate::error::ExecFailure;
use crate::http::{Client, ScenarioRunResponse, ScenariosResponse};
use crate::style::{CYAN, DIM, GREEN, RED};

const RUN_ENDPOINT: &str = "/api/v1/scenarios/run";

/// `liminal run`。named / glob / ad-hoc の 3 モード (SPEC §4.10)。
pub(crate) fn run(client: &Client, args: &RunArgs, json_out: bool) -> Result<()> {
    // (label, 生レスポンス) を保持する。--json はこの生の値をそのまま出す。
    let results: Vec<(String, Value)> = match (&args.path, &args.steps) {
        (Some(_), Some(_)) => bail!("PATH と --steps は同時に指定できません"),
        (None, None) => bail!("PATH か --steps のどちらかを指定してください"),
        // ad-hoc: ステップ定義をそのまま中継する (バリデーションはサーバに任せる)
        (None, Some(src)) => {
            let steps = load_steps(src)?;
            let resp = client.post_value(RUN_ENDPOINT, &json!({"steps": steps}))?;
            vec![("(ad-hoc)".to_string(), resp)]
        }
        (Some(path), None) if has_magic(path) => run_glob(client, path)?,
        (Some(path), None) => {
            let resp = client.post_value(RUN_ENDPOINT, &json!({"path": path}))?;
            vec![(path.clone(), resp)]
        }
    };

    // テキスト表示・集計・レポートには型に落とした値を使う。
    let typed: Vec<(String, ScenarioRunResponse)> = results
        .iter()
        .map(|(label, v)| {
            serde_json::from_value(v.clone()).map(|r: ScenarioRunResponse| (label.clone(), r))
        })
        .collect::<Result<_, _>>()?;

    let multiple = results.len() > 1 || args.path.as_deref().is_some_and(has_magic);
    if multiple {
        render_multi(&results, &typed, json_out)?;
    } else {
        render_single(&results[0].0, &results[0].1, &typed[0].1, json_out)?;
    }

    if let Some(report) = &args.report {
        let cases: Vec<Case<'_>> = typed
            .iter()
            .map(|(label, resp)| Case { label, resp })
            .collect();
        junit::write(Path::new(report), &junit::build(&cases))?;
    }

    // 1 つでも失敗していれば exit 2 (SPEC §11)
    if typed.iter().any(|(_, r)| !r.success) {
        return Err(ExecFailure.into());
    }
    Ok(())
}

// glob モード: シナリオ一覧を引いてパターン一致を順に実行する。
fn run_glob(client: &Client, pattern: &str) -> Result<Vec<(String, Value)>> {
    let list: ScenariosResponse = serde_json::from_value(client.get_value("/api/v1/scenarios")?)?;
    let mut matched: Vec<String> = list
        .scenarios
        .into_iter()
        .map(|s| s.path)
        .filter(|p| fnmatch(p, pattern))
        .collect();
    matched.sort();

    if matched.is_empty() {
        bail!("パターン '{pattern}' に一致するシナリオがありません");
    }

    let mut out = Vec::with_capacity(matched.len());
    for path in matched {
        let resp = client.post_value(RUN_ENDPOINT, &json!({"path": path}))?;
        out.push((path, resp));
    }
    Ok(out)
}

// --steps の読み込み。`-` は stdin。配列と {"steps": [...]} の両形式を受ける。
fn load_steps(src: &str) -> Result<Value> {
    let raw = if src == "-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        buf
    } else {
        std::fs::read_to_string(src)?
    };

    match serde_json::from_str::<Value>(&raw)? {
        Value::Array(a) => Ok(Value::Array(a)),
        Value::Object(o) => match o.get("steps") {
            Some(Value::Array(a)) => Ok(Value::Array(a.clone())),
            _ => bail!("steps JSON は配列か {{\"steps\": [...]}} である必要があります"),
        },
        _ => bail!("steps JSON は配列か {{\"steps\": [...]}} である必要があります"),
    }
}

fn render_single(label: &str, raw: &Value, r: &ScenarioRunResponse, json_out: bool) -> Result<()> {
    if json_out {
        // サーバが増やしたフィールドを落とさないよう、生のレスポンスをそのまま出す
        println!("{}", serde_json::to_string_pretty(raw)?);
        return Ok(());
    }

    let head = if r.success {
        format!("{GREEN}PASS{GREEN:#}")
    } else {
        format!("{RED}FAIL{RED:#}")
    };
    println!("{head}  {CYAN}{label}{CYAN:#}  ({:.1} ms)", r.duration_ms);

    if !r.success
        && let Some(idx) = r.failed_at_step
    {
        println!("  failedAtStep: {idx}");
    }

    for (i, s) in r.steps.iter().enumerate() {
        let mark = if s.success {
            format!("{GREEN}✓{GREEN:#}")
        } else {
            format!("{RED}✗{RED:#}")
        };
        println!(
            "  {mark} [{i}] {:<16} {}  {DIM}({:.1}ms){DIM:#}",
            s.kind,
            step_extra(s),
            s.duration_ms
        );
    }
    Ok(())
}

// kind ごとの補足表示 (SPEC §4.10)。
fn step_extra(s: &crate::http::ScenarioStepResult) -> String {
    match s.kind.as_str() {
        "Command" => s.extra_str("commandPath").unwrap_or_default(),
        "AssertEquals" | "AssertNotEquals" => {
            let actual = s.extra_str("actualValue").unwrap_or_default();
            match &s.error {
                Some(e) => format!("actual={actual}  {e}"),
                None => format!("actual={actual}"),
            }
        }
        _ => String::new(),
    }
}

fn render_multi(
    raw: &[(String, Value)],
    results: &[(String, ScenarioRunResponse)],
    json_out: bool,
) -> Result<()> {
    let passed = results.iter().filter(|(_, r)| r.success).count();
    let failed = results.len() - passed;
    let total_ms: f64 = results.iter().map(|(_, r)| r.duration_ms).sum();

    if json_out {
        // label は最後に入れて resp.path を上書きする (ad-hoc で null になる対策、SPEC §4.10)
        let items: Vec<Value> = raw
            .iter()
            .map(|(label, v)| {
                let mut v = v.clone();
                if let Value::Object(map) = &mut v {
                    map.insert("path".into(), Value::from(label.clone()));
                }
                v
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "scenarios": items,
                "total": results.len(),
                "passed": passed,
                "failed": failed,
            }))?
        );
        return Ok(());
    }

    let width = results
        .iter()
        .map(|(l, _)| l.len())
        .max()
        .unwrap_or(0)
        .min(60);

    for (label, r) in results {
        if r.success {
            println!(
                "  {GREEN}✓{GREEN:#} {label:<width$}  ({:.0} ms)",
                r.duration_ms
            );
        } else {
            let at = r
                .failed_at_step
                .map(|i| format!("  failedAtStep={i}"))
                .unwrap_or_default();
            println!(
                "  {RED}✗{RED:#} {label:<width$}  ({:.0} ms){at}",
                r.duration_ms
            );
            if let Some(msg) = first_error(r) {
                println!("      {RED}{msg}{RED:#}");
            }
        }
    }

    let head = if failed == 0 {
        format!("{GREEN}PASS{GREEN:#}")
    } else {
        format!("{RED}FAIL{RED:#}")
    };
    println!(
        "\n{head}  {} scenarios, {passed} passed, {failed} failed  {DIM}({total_ms:.1} ms total){DIM:#}",
        results.len()
    );
    Ok(())
}

// 複数実行時に 1 行だけ添える代表エラー。
fn first_error(r: &ScenarioRunResponse) -> Option<String> {
    if let Some(idx) = r.failed_at_step
        && idx >= 0
        && let Some(s) = r.steps.get(idx as usize)
        && let Some(e) = &s.error
    {
        return Some(e.clone());
    }
    r.error.clone()
}
