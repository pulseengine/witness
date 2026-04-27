//! End-to-end test: spin up the router on a random port, hit a few
//! routes against a fake `verdict-evidence/` tree, assert the responses.

use std::path::PathBuf;

use serde_json::json;
use witness_viz::AppState;

fn write_fake_bundle(root: &std::path::Path, name: &str) {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).expect("create verdict dir");

    let report = json!({
        "schema": "witness.mcdc.report/v0.5",
        "witness_version": "0.9.0",
        "module": "fake_module",
        "overall": {
            "decisions_total": 2,
            "decisions_full_mcdc": 1,
            "conditions_total": 4,
            "conditions_proved": 3,
            "conditions_gap": 1,
            "conditions_dead": 0,
        },
        "decisions": [
            {
                "id": 1,
                "source_file": "src/foo.rs",
                "source_line": 12,
                "status": "full_mcdc",
                "conditions": [
                    {
                        "index": 0,
                        "branch_id": 100,
                        "status": "proved",
                        "interpretation": "isolates c0",
                        "pair": [0, 1],
                    },
                    {
                        "index": 1,
                        "branch_id": 101,
                        "status": "proved",
                        "interpretation": "isolates c1",
                        "pair": [0, 2],
                    },
                ],
                "truth_table": [
                    { "row_id": 0, "evaluated": { "0": false, "1": false }, "outcome": false },
                    { "row_id": 1, "evaluated": { "0": true,  "1": false }, "outcome": true },
                    { "row_id": 2, "evaluated": { "0": false, "1": true  }, "outcome": true },
                ],
            },
            {
                "id": 2,
                "source_file": "src/bar.rs",
                "source_line": 7,
                "status": "partial_mcdc",
                "conditions": [
                    {
                        "index": 0,
                        "branch_id": 200,
                        "status": "proved",
                        "interpretation": "isolates c0",
                        "pair": [0, 1],
                    },
                    {
                        "index": 1,
                        "branch_id": 201,
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

    let manifest = json!({
        "schema": "witness.manifest/v0.5",
        "branches": [
            { "branch_id": 100, "function_name": "foo::eval", "byte_offset": 42 },
            { "branch_id": 101, "function_name": "foo::eval", "byte_offset": 64 },
            { "branch_id": 200, "function_name": "bar::eval", "byte_offset": 13 },
            { "branch_id": 201, "function_name": "bar::eval", "byte_offset": 27 },
        ],
    });
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).expect("serialize manifest"),
    )
    .expect("write manifest.json");
}

#[tokio::test]
async fn end_to_end_smoke() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let reports_dir: PathBuf = tmp.path().to_path_buf();
    write_fake_bundle(&reports_dir, "foo");

    let state = AppState::new(reports_dir);
    let app = witness_viz::router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local_addr");

    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let base = format!("http://{addr}");
    let client = reqwest::Client::builder()
        .build()
        .expect("build reqwest client");

    // /
    let r = client.get(&base).send().await.expect("GET /");
    assert_eq!(r.status(), 200);
    let body = r.text().await.expect("text");
    assert!(body.contains("witness-viz"), "missing brand: {body}");
    assert!(body.contains("foo"), "missing verdict link: {body}");
    assert!(body.contains("Compliance overview"));

    // /verdict/foo
    let r = client
        .get(format!("{base}/verdict/foo"))
        .send()
        .await
        .expect("GET /verdict/foo");
    assert_eq!(r.status(), 200);
    let body = r.text().await.expect("text");
    assert!(body.contains("Decisions"));
    assert!(body.contains("fake_module"));
    assert!(body.contains("/decision/foo/1"));
    assert!(body.contains("/decision/foo/2"));

    // /decision/foo/1
    let r = client
        .get(format!("{base}/decision/foo/1"))
        .send()
        .await
        .expect("GET /decision/foo/1");
    assert_eq!(r.status(), 200);
    let body = r.text().await.expect("text");
    assert!(body.contains("Truth table"));
    assert!(body.contains("Independent-effect pairs"));
    assert!(body.contains("isolates c0"));

    // /api/v1/summary
    let r = client
        .get(format!("{base}/api/v1/summary"))
        .send()
        .await
        .expect("GET /api/v1/summary");
    assert_eq!(r.status(), 200);
    let summary: serde_json::Value = r.json().await.expect("json");
    assert_eq!(summary["verdicts"], 1);
    assert_eq!(summary["decisions_total"], 2);
    assert_eq!(summary["decisions_full_mcdc"], 1);
    assert_eq!(summary["branches"], 4);

    // /api/v1/verdicts
    let r = client
        .get(format!("{base}/api/v1/verdicts"))
        .send()
        .await
        .expect("GET /api/v1/verdicts");
    assert_eq!(r.status(), 200);
    let verdicts: serde_json::Value = r.json().await.expect("json");
    assert_eq!(verdicts.as_array().map(Vec::len), Some(1));

    // /api/v1/verdict/foo
    let r = client
        .get(format!("{base}/api/v1/verdict/foo"))
        .send()
        .await
        .expect("GET /api/v1/verdict/foo");
    assert_eq!(r.status(), 200);

    // /api/v1/decision/foo/2
    let r = client
        .get(format!("{base}/api/v1/decision/foo/2"))
        .send()
        .await
        .expect("GET /api/v1/decision/foo/2");
    assert_eq!(r.status(), 200);
    let dec: serde_json::Value = r.json().await.expect("json");
    assert_eq!(dec["id"], 2);

    // /api/v1/verdict/missing → 404
    let r = client
        .get(format!("{base}/api/v1/verdict/missing"))
        .send()
        .await
        .expect("GET missing");
    assert_eq!(r.status(), 404);

    // /verdict/missing → 404
    let r = client
        .get(format!("{base}/verdict/missing"))
        .send()
        .await
        .expect("GET missing verdict");
    assert_eq!(r.status(), 404);

    // /assets/styles.css
    let r = client
        .get(format!("{base}/assets/styles.css"))
        .send()
        .await
        .expect("GET styles");
    assert_eq!(r.status(), 200);
    assert_eq!(
        r.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("text/css"),
    );

    server.abort();
}
