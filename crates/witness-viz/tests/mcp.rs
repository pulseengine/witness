//! MCP-over-HTTP integration tests. Spins up the Axum router on an
//! ephemeral port and exercises the JSON-RPC envelope against a fake
//! verdict bundle that contains both a fully-proved and a gap condition.

use std::path::PathBuf;

use serde_json::{Value, json};
use witness_viz::AppState;

fn write_fixture(root: &std::path::Path, name: &str) {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).expect("create verdict dir");

    let report = json!({
        "schema": "witness.mcdc.report/v0.5",
        "witness_version": "0.9.0",
        "module": "fixture_module",
        "overall": {
            "decisions_total": 1,
            "decisions_full_mcdc": 0,
            "conditions_total": 2,
            "conditions_proved": 1,
            "conditions_gap": 1,
            "conditions_dead": 0,
        },
        "decisions": [
            {
                "id": 1,
                "source_file": "src/fixture.rs",
                "source_line": 9,
                "status": "partial_mcdc",
                "conditions": [
                    {
                        "index": 0,
                        "branch_id": 10,
                        "status": "proved",
                        "interpretation": "isolates c0",
                        "pair": [0, 1],
                    },
                    {
                        "index": 1,
                        "branch_id": 11,
                        "status": "gap",
                    },
                ],
                "truth_table": [
                    { "row_id": 0, "evaluated": { "0": false, "1": false }, "outcome": false },
                    { "row_id": 1, "evaluated": { "0": true,  "1": false }, "outcome": true },
                ],
            }
        ],
    });

    std::fs::write(
        dir.join("report.json"),
        serde_json::to_vec_pretty(&report).expect("serialize report"),
    )
    .expect("write report.json");
}

async fn spawn() -> (String, tokio::task::JoinHandle<()>, tempfile::TempDir) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let reports_dir: PathBuf = tmp.path().to_path_buf();
    write_fixture(&reports_dir, "alpha");

    let state = AppState::new(reports_dir);
    let app = witness_viz::router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local_addr");
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{addr}"), server, tmp)
}

async fn rpc(client: &reqwest::Client, base: &str, payload: Value) -> Value {
    client
        .post(format!("{base}/mcp"))
        .json(&payload)
        .send()
        .await
        .expect("POST /mcp")
        .json::<Value>()
        .await
        .expect("decode json")
}

#[tokio::test]
async fn tools_list_returns_three_tools() {
    let (base, server, _tmp) = spawn().await;
    let client = reqwest::Client::builder().build().expect("client");

    let resp = rpc(
        &client,
        &base,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }),
    )
    .await;
    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    let tools = resp["result"]["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), 3, "expected 3 tools, got {}", tools.len());
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(names.contains(&"get_decision_truth_table"));
    assert!(names.contains(&"find_missing_witness"));
    assert!(names.contains(&"list_uncovered_conditions"));

    server.abort();
}

#[tokio::test]
async fn get_decision_truth_table_returns_decision_fields() {
    let (base, server, _tmp) = spawn().await;
    let client = reqwest::Client::builder().build().expect("client");

    let resp = rpc(
        &client,
        &base,
        json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": {
                "name": "get_decision_truth_table",
                "arguments": { "verdict": "alpha", "decision_id": 1 }
            }
        }),
    )
    .await;
    assert_eq!(resp["id"], 7);
    let text = resp["result"]["content"][0]["text"]
        .as_str()
        .expect("text content");
    let inner: Value = serde_json::from_str(text).expect("inner json");
    assert_eq!(inner["id"], 1);
    assert_eq!(inner["source_file"], "src/fixture.rs");
    assert_eq!(inner["status"], "partial_mcdc");
    let conditions = inner["conditions"].as_array().expect("conditions");
    assert_eq!(conditions.len(), 2);

    server.abort();
}

#[tokio::test]
async fn unknown_tool_returns_jsonrpc_error() {
    let (base, server, _tmp) = spawn().await;
    let client = reqwest::Client::builder().build().expect("client");

    let resp = rpc(
        &client,
        &base,
        json!({
            "jsonrpc": "2.0",
            "id": 12,
            "method": "tools/call",
            "params": { "name": "does_not_exist", "arguments": {} }
        }),
    )
    .await;
    let code = resp["error"]["code"].as_i64().expect("error code");
    assert!(
        code == -32601 || code == -32602,
        "expected -32601 or -32602, got {code}"
    );

    // Also verify a totally unknown method name yields -32601.
    let resp = rpc(
        &client,
        &base,
        json!({ "jsonrpc": "2.0", "id": 13, "method": "fake/method" }),
    )
    .await;
    assert_eq!(resp["error"]["code"], -32601);

    server.abort();
}

#[tokio::test]
async fn find_missing_witness_proved_returns_already_proved() {
    let (base, server, _tmp) = spawn().await;
    let client = reqwest::Client::builder().build().expect("client");

    let resp = rpc(
        &client,
        &base,
        json!({
            "jsonrpc": "2.0",
            "id": 21,
            "method": "tools/call",
            "params": {
                "name": "find_missing_witness",
                "arguments": {
                    "verdict": "alpha",
                    "decision_id": 1,
                    "condition_index": 0
                }
            }
        }),
    )
    .await;
    let text = resp["result"]["content"][0]["text"]
        .as_str()
        .expect("text");
    let inner: Value = serde_json::from_str(text).expect("inner json");
    assert_eq!(inner["status"], "proved");
    let rationale = inner["rationale"].as_str().expect("rationale");
    assert!(
        rationale.contains("Already proved"),
        "rationale should mention proved: {rationale}"
    );
    assert!(inner["needed_row"].is_null());
    assert!(inner["paired_row_id"].is_null());

    server.abort();
}

#[tokio::test]
async fn find_missing_witness_gap_returns_needed_row() {
    let (base, server, _tmp) = spawn().await;
    let client = reqwest::Client::builder().build().expect("client");

    let resp = rpc(
        &client,
        &base,
        json!({
            "jsonrpc": "2.0",
            "id": 22,
            "method": "tools/call",
            "params": {
                "name": "find_missing_witness",
                "arguments": {
                    "verdict": "alpha",
                    "decision_id": 1,
                    "condition_index": 1
                }
            }
        }),
    )
    .await;
    let text = resp["result"]["content"][0]["text"]
        .as_str()
        .expect("text");
    let inner: Value = serde_json::from_str(text).expect("inner json");
    assert_eq!(inner["status"], "gap");
    let needed_row = inner["needed_row"].as_array().expect("needed_row array");
    assert_eq!(needed_row.len(), 2);
    let paired = inner["paired_row_id"].as_u64().expect("paired_row_id");
    // Row 0 has both conditions false; flipping index 1 yields [false, true].
    assert_eq!(paired, 0);
    assert_eq!(needed_row[1], json!(true));

    server.abort();
}

#[tokio::test]
async fn list_uncovered_conditions_returns_array() {
    let (base, server, _tmp) = spawn().await;
    let client = reqwest::Client::builder().build().expect("client");

    let resp = rpc(
        &client,
        &base,
        json!({
            "jsonrpc": "2.0",
            "id": 33,
            "method": "tools/call",
            "params": {
                "name": "list_uncovered_conditions",
                "arguments": {}
            }
        }),
    )
    .await;
    let text = resp["result"]["content"][0]["text"]
        .as_str()
        .expect("text");
    let inner: Value = serde_json::from_str(text).expect("inner json");
    let arr = inner.as_array().expect("array");
    // Fixture has exactly one gap condition (decision 1, condition 1).
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["verdict"], "alpha");
    assert_eq!(arr[0]["decision_id"], 1);
    assert_eq!(arr[0]["condition_index"], 1);
    assert_eq!(arr[0]["status"], "gap");

    server.abort();
}
