use std::path::Path;

use anyhow::Result;

use crate::http::ScenarioRunResponse;

/// 1 シナリオ分の結果 (ラベル付き)。
pub(crate) struct Case<'a> {
    pub label: &'a str,
    pub resp: &'a ScenarioRunResponse,
}

/// JUnit XML を組み立てる (SPEC §7)。
pub(crate) fn build(cases: &[Case<'_>]) -> String {
    let total = cases.len();
    let failures = cases.iter().filter(|c| !c.resp.success).count();
    let total_sec: f64 = cases.iter().map(|c| c.resp.duration_ms).sum::<f64>() / 1000.0;

    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!(
        "<testsuites name=\"liminal\" tests=\"{total}\" failures=\"{failures}\" time=\"{total_sec:.3}\">\n"
    ));
    out.push_str(&format!(
        "  <testsuite name=\"liminal\" tests=\"{total}\" failures=\"{failures}\" time=\"{total_sec:.3}\">\n"
    ));

    for c in cases {
        let sec = c.resp.duration_ms / 1000.0;
        let name = escape(c.label);
        if c.resp.success {
            out.push_str(&format!(
                "    <testcase name=\"{name}\" time=\"{sec:.3}\"/>\n"
            ));
            continue;
        }
        out.push_str(&format!(
            "    <testcase name=\"{name}\" time=\"{sec:.3}\">\n"
        ));
        out.push_str(&format!(
            "      <failure message=\"{}\">{}</failure>\n",
            escape(&failure_message(c.resp)),
            escape(&failure_body(c.resp))
        ));
        out.push_str("    </testcase>\n");
    }

    out.push_str("  </testsuite>\n</testsuites>\n");
    out
}

/// `failure` 要素の message 属性。` — ` で連結する (SPEC §7)。
fn failure_message(r: &ScenarioRunResponse) -> String {
    let idx = r.failed_at_step.unwrap_or(-1);
    let mut parts = vec![format!("failedAtStep={idx}")];
    if let Some(step) = failed_step(r) {
        parts.push(step.kind.clone());
        if let Some(e) = &step.error {
            parts.push(e.clone());
        }
    }
    parts.join(" — ")
}

/// `failure` 要素の本文。失敗ステップが取れない場合は resp.error にフォールバックする。
fn failure_body(r: &ScenarioRunResponse) -> String {
    let Some(step) = failed_step(r) else {
        return r.error.clone().unwrap_or_default();
    };
    let idx = r.failed_at_step.unwrap_or(-1);
    let mut lines = vec![format!("step[{idx}] {}", step.kind)];
    if let Some(v) = step.extra_str("actualValue") {
        lines.push(format!("  actualValue: {v}"));
    }
    if let Some(v) = step.extra_str("expected") {
        lines.push(format!("  expected: {v}"));
    }
    if let Some(e) = &step.error {
        lines.push(format!("  error: {e}"));
    }
    lines.join("\n")
}

// failedAtStep が steps の範囲内のときだけ該当ステップを返す。
fn failed_step(r: &ScenarioRunResponse) -> Option<&crate::http::ScenarioStepResult> {
    let idx = r.failed_at_step?;
    if idx < 0 {
        return None;
    }
    r.steps.get(idx as usize)
}

/// XML エスケープ。`&` `<` `>` `"` の 4 つだけ (SPEC §7、`'` はエスケープしない)。
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// レポートを書き出す。親ディレクトリが無ければ作る (SPEC §7)。
pub(crate) fn write(path: &Path, xml: &str) -> Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, xml)?;
    Ok(())
}

#[cfg(test)]
// テスト名は日本語で書く方針のため、snake_case 検査から除外する
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use serde_json::json;

    fn resp(v: serde_json::Value) -> ScenarioRunResponse {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn junit_成功のみは自己終了タグ() {
        let r = resp(json!({"success": true, "durationMs": 1200.0}));
        let xml = build(&[Case {
            label: "A/B",
            resp: &r,
        }]);
        assert!(
            xml.contains(r#"<testcase name="A/B" time="1.200"/>"#),
            "{xml}"
        );
        assert!(xml.contains(r#"failures="0""#), "{xml}");
    }

    #[test]
    fn junit_失敗はfailure要素を持つ() {
        let r = resp(json!({
            "success": false, "durationMs": 500.0, "failedAtStep": 1,
            "steps": [
                {"kind": "Command", "success": true, "durationMs": 1.0},
                {"kind": "AssertEquals", "success": false, "durationMs": 0.1,
                 "error": "expected '70' but got '65'", "actualValue": 65, "expected": "70"}
            ]
        }));
        let xml = build(&[Case {
            label: "A",
            resp: &r,
        }]);
        assert!(
            xml.contains("failedAtStep=1 — AssertEquals — expected"),
            "{xml}"
        );
        assert!(xml.contains("step[1] AssertEquals"), "{xml}");
        assert!(xml.contains("actualValue: 65"), "{xml}");
        assert!(xml.contains("expected: 70"), "{xml}");
        assert!(xml.contains(r#"failures="1""#), "{xml}");
    }

    #[test]
    fn junit_failedAtStepが範囲外なら_error_にフォールバック() {
        let r = resp(json!({
            "success": false, "durationMs": 10.0, "failedAtStep": 9,
            "error": "scenario blew up", "steps": []
        }));
        let xml = build(&[Case {
            label: "A",
            resp: &r,
        }]);
        assert!(xml.contains("scenario blew up"), "{xml}");
        assert!(xml.contains("failedAtStep=9"), "{xml}");
    }

    #[test]
    fn junit_failedAtStep無しでも壊れない() {
        let r = resp(json!({"success": false, "durationMs": 10.0, "error": "x"}));
        let xml = build(&[Case {
            label: "A",
            resp: &r,
        }]);
        assert!(xml.contains("failedAtStep=-1"), "{xml}");
    }

    #[test]
    fn junit_特殊文字をエスケープする() {
        let r = resp(json!({
            "success": false, "durationMs": 1.0, "failedAtStep": 0,
            "steps": [{"kind": "A<b>&\"c\"", "success": false, "durationMs": 0.0, "error": "x < y & \"z\""}]
        }));
        let xml = build(&[Case {
            label: "a&b<c>\"d\"",
            resp: &r,
        }]);
        assert!(xml.contains("a&amp;b&lt;c&gt;&quot;d&quot;"), "{xml}");
        assert!(xml.contains("x &lt; y &amp; &quot;z&quot;"), "{xml}");
    }

    #[test]
    fn junit_シングルクォートはエスケープしない() {
        assert_eq!(escape("it's"), "it's");
    }

    #[test]
    fn junit_集計は件数と合計秒() {
        let ok = resp(json!({"success": true, "durationMs": 1000.0}));
        let ng = resp(json!({"success": false, "durationMs": 2000.0, "error": "e"}));
        let xml = build(&[
            Case {
                label: "A",
                resp: &ok,
            },
            Case {
                label: "B",
                resp: &ng,
            },
        ]);
        assert!(xml.contains(r#"tests="2""#), "{xml}");
        assert!(xml.contains(r#"failures="1""#), "{xml}");
        assert!(xml.contains(r#"time="3.000""#), "{xml}");
    }

    #[test]
    fn junit_レポートは親ディレクトリを作る() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("nested/dir/report.xml");
        write(&path, "<x/>").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "<x/>");
    }
}
