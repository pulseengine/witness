//! Pragmatic MCP-over-HTTP endpoint.
//!
//! Implements a minimal JSON-RPC 2.0 surface following the Model Context
//! Protocol's `tools/list` and `tools/call` conventions. Three tools are
//! exposed: `get_decision_truth_table`, `find_missing_witness`, and
//! `list_uncovered_conditions`. The full `rmcp` crate is intentionally
//! avoided; serde_json::Value is enough for the tiny surface needed here.
//!
//! Mounted at `POST /mcp` by [`crate::router`].

use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};

use crate::data::{self, DecisionReport, McdcReport, TruthRow, VerdictBundle};
use crate::state::AppState;

/// JSON-RPC error codes used by the handler.
const ERR_PARSE: i64 = -32700;
const ERR_INVALID_REQUEST: i64 = -32600;
const ERR_METHOD_NOT_FOUND: i64 = -32601;
const ERR_INVALID_PARAMS: i64 = -32602;
const ERR_INTERNAL: i64 = -32603;
const ERR_NOT_FOUND: i64 = -32001;

/// Errors raised by tool dispatch. Mapped to JSON-RPC error codes by
/// [`handler`].
pub enum McpError {
    InvalidParams(String),
    NotFound(String),
    Internal(String),
}

impl McpError {
    fn code(&self) -> i64 {
        match self {
            Self::InvalidParams(_) => ERR_INVALID_PARAMS,
            Self::NotFound(_) => ERR_NOT_FOUND,
            Self::Internal(_) => ERR_INTERNAL,
        }
    }

    fn message(&self) -> &str {
        match self {
            Self::InvalidParams(m) | Self::NotFound(m) | Self::Internal(m) => m,
        }
    }
}

/// `POST /mcp` entry point. Parses the JSON-RPC envelope, dispatches to
/// the right method, and wraps the result/error per spec.
pub async fn handler(State(state): State<AppState>, Json(body): Json<Value>) -> Json<Value> {
    let id = body.get("id").cloned().unwrap_or(Value::Null);

    if body.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Json(rpc_error(id, ERR_INVALID_REQUEST, "jsonrpc must be \"2.0\""));
    }

    let method = match body.get("method").and_then(Value::as_str) {
        Some(m) => m,
        None => return Json(rpc_error(id, ERR_PARSE, "missing method")),
    };

    match method {
        "tools/list" => Json(rpc_ok(id, tools_list())),
        "tools/call" => {
            let params = body.get("params").cloned().unwrap_or(Value::Null);
            let name = match params.get("name").and_then(Value::as_str) {
                Some(n) => n.to_string(),
                None => {
                    return Json(rpc_error(id, ERR_INVALID_PARAMS, "missing params.name"));
                }
            };
            let empty = json!({});
            let args = params.get("arguments").unwrap_or(&empty);
            match dispatch(&state, &name, args) {
                Ok(tool_result) => Json(rpc_ok(id, tool_call_envelope(&tool_result))),
                Err(e) => Json(rpc_error(id, e.code(), e.message())),
            }
        }
        other => {
            tracing::warn!(method = other, "mcp: unknown method");
            Json(rpc_error(id, ERR_METHOD_NOT_FOUND, "method not found"))
        }
    }
}

/// JSON-RPC success envelope.
fn rpc_ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// JSON-RPC error envelope.
fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
}

/// Wrap a tool's JSON output in MCP's `{ content: [{ type, text }] }` shape.
fn tool_call_envelope(result: &Value) -> Value {
    let text = serde_json::to_string(result).unwrap_or_else(|_| "null".to_string());
    json!({
        "content": [
            { "type": "text", "text": text }
        ]
    })
}

/// Tool descriptor list returned by `tools/list`.
pub fn tools_list() -> Value {
    json!({
        "tools": [
            {
                "name": "get_decision_truth_table",
                "description": "Return the full DecisionReport (conditions, truth table, status) for a given verdict and decision id.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "verdict": { "type": "string" },
                        "decision_id": { "type": "integer" }
                    },
                    "required": ["verdict", "decision_id"]
                }
            },
            {
                "name": "find_missing_witness",
                "description": "Suggest the truth-table row needed to prove MC/DC for a specific condition. Returns the inferred row vector, the existing row that pairs with it, and a tutorial-style rationale.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "verdict": { "type": "string" },
                        "decision_id": { "type": "integer" },
                        "condition_index": { "type": "integer" }
                    },
                    "required": ["verdict", "decision_id", "condition_index"]
                }
            },
            {
                "name": "list_uncovered_conditions",
                "description": "List every condition whose status is not 'proved' (i.e. gap or dead). Optional verdict filter.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "verdict": { "type": "string" }
                    }
                }
            }
        ]
    })
}

/// Route a `tools/call` to the matching tool function.
pub fn dispatch(state: &AppState, name: &str, args: &Value) -> Result<Value, McpError> {
    match name {
        "get_decision_truth_table" => tool_get_decision_truth_table(state, args),
        "find_missing_witness" => tool_find_missing_witness(state, args),
        "list_uncovered_conditions" => tool_list_uncovered_conditions(state, args),
        other => {
            tracing::warn!(tool = other, "mcp: unknown tool");
            Err(McpError::InvalidParams(format!("unknown tool: {other}")))
        }
    }
}

fn require_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, McpError> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| McpError::InvalidParams(format!("missing string param: {key}")))
}

fn require_u32(args: &Value, key: &str) -> Result<u32, McpError> {
    let v = args
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| McpError::InvalidParams(format!("missing integer param: {key}")))?;
    u32::try_from(v).map_err(|_| McpError::InvalidParams(format!("{key} out of u32 range")))
}

fn load_verdict(state: &AppState, name: &str) -> Result<VerdictBundle, McpError> {
    match data::find_verdict(state.reports_dir(), name) {
        Ok(Some(b)) => Ok(b),
        Ok(None) => Err(McpError::NotFound(format!("verdict not found: {name}"))),
        Err(e) => Err(McpError::Internal(e.to_string())),
    }
}

fn tool_get_decision_truth_table(state: &AppState, args: &Value) -> Result<Value, McpError> {
    let verdict = require_str(args, "verdict")?;
    let decision_id = require_u32(args, "decision_id")?;
    let bundle = load_verdict(state, verdict)?;
    let decision = find_decision(&bundle.report, decision_id)
        .ok_or_else(|| McpError::NotFound(format!("decision not found: {decision_id}")))?;
    serde_json::to_value(decision).map_err(|e| McpError::Internal(e.to_string()))
}

fn tool_find_missing_witness(state: &AppState, args: &Value) -> Result<Value, McpError> {
    let verdict = require_str(args, "verdict")?;
    let decision_id = require_u32(args, "decision_id")?;
    let condition_index = require_u32(args, "condition_index")?;

    let bundle = load_verdict(state, verdict)?;
    let decision = find_decision(&bundle.report, decision_id)
        .ok_or_else(|| McpError::NotFound(format!("decision not found: {decision_id}")))?;

    let condition = decision
        .conditions
        .iter()
        .find(|c| c.index == condition_index)
        .ok_or_else(|| {
            McpError::NotFound(format!("condition_index not found: {condition_index}"))
        })?;

    let status = condition.status.as_str();

    if status == "proved" {
        let rationale = match condition.pair {
            Some([a, b]) => format!("Already proved by rows {a} and {b}."),
            None => "Already proved.".to_string(),
        };
        return Ok(json!({
            "decision_id": decision_id,
            "condition_index": condition_index,
            "status": status,
            "needed_row": Value::Null,
            "paired_row_id": Value::Null,
            "rationale": rationale,
            "interpretation": condition.interpretation.clone().unwrap_or_else(|| "unique-cause".to_string()),
        }));
    }

    // Gap or dead: build a hypothetical row that would pair with an
    // existing one. We pick any existing row whose target-condition value
    // is opposite to "all-but-target true". Concretely: if any row has
    // condition_index = true, the needed_row flips it to false (and
    // copies the rest); otherwise we flip false -> true.
    let condition_keys = condition_keys_in_order(&decision.conditions);
    let target_key = condition_index.to_string();

    let mut needed_row: Vec<Value> = vec![Value::Null; condition_keys.len()];
    let mut paired_row_id: Option<u32> = None;
    let rationale: String;

    if let Some(base_row) = decision.truth_table.first() {
        let base_target = base_row.evaluated.get(&target_key).copied();
        let flipped_target = base_target.map(|v| !v);
        for (i, key) in condition_keys.iter().enumerate() {
            let val = if key == &target_key {
                flipped_target
            } else {
                base_row.evaluated.get(key).copied()
            };
            needed_row[i] = match val {
                Some(b) => Value::Bool(b),
                None => Value::Null,
            };
        }
        paired_row_id = Some(base_row.row_id);

        let base_repr = format_row(base_row, &condition_keys);
        let needed_repr = format_needed_row(&needed_row);
        rationale = format!(
            "To prove condition {condition_index}, pair existing row {row} ({base_repr}) with a new row {needed_repr} that flips condition {condition_index} while holding the others fixed; the decision outcome must change.",
            row = base_row.row_id,
        );
    } else {
        rationale = format!(
            "Decision has no recorded truth-table rows; cannot infer a witness for condition {condition_index} without an executed baseline."
        );
    }

    let interpretation = if status == "dead" {
        Value::Null
    } else {
        Value::String(
            condition
                .interpretation
                .clone()
                .unwrap_or_else(|| "unique-cause".to_string()),
        )
    };

    Ok(json!({
        "decision_id": decision_id,
        "condition_index": condition_index,
        "status": status,
        "needed_row": needed_row,
        "paired_row_id": paired_row_id,
        "rationale": rationale,
        "interpretation": interpretation,
    }))
}

fn tool_list_uncovered_conditions(state: &AppState, args: &Value) -> Result<Value, McpError> {
    let filter = args.get("verdict").and_then(Value::as_str);

    let verdicts = if let Some(name) = filter {
        match data::find_verdict(state.reports_dir(), name) {
            Ok(Some(b)) => vec![b],
            Ok(None) => return Err(McpError::NotFound(format!("verdict not found: {name}"))),
            Err(e) => return Err(McpError::Internal(e.to_string())),
        }
    } else {
        data::load_verdicts(state.reports_dir()).map_err(|e| McpError::Internal(e.to_string()))?
    };

    let mut out: Vec<Value> = Vec::new();
    for v in &verdicts {
        for d in &v.report.decisions {
            for c in &d.conditions {
                if c.status != "proved" {
                    out.push(json!({
                        "verdict": v.name,
                        "decision_id": d.id,
                        "source_file": d.source_file,
                        "source_line": d.source_line,
                        "condition_index": c.index,
                        "branch_id": c.branch_id,
                        "status": c.status,
                    }));
                }
            }
        }
    }
    Ok(Value::Array(out))
}

/// Look up a decision by id within a parsed report.
pub fn find_decision(report: &McdcReport, decision_id: u32) -> Option<&DecisionReport> {
    report.decisions.iter().find(|d| d.id == decision_id)
}

fn condition_keys_in_order(conditions: &[crate::data::ConditionReport]) -> Vec<String> {
    let mut ordered: Vec<String> = conditions.iter().map(|c| c.index.to_string()).collect();
    ordered.sort();
    ordered.dedup();
    ordered
}

fn format_row(row: &TruthRow, keys: &[String]) -> String {
    let cells: Vec<String> = keys
        .iter()
        .map(|k| match row.evaluated.get(k) {
            Some(true) => "T".to_string(),
            Some(false) => "F".to_string(),
            None => "?".to_string(),
        })
        .collect();
    format!("[{}]→{}", cells.join(","), if row.outcome { "T" } else { "F" })
}

fn format_needed_row(row: &[Value]) -> String {
    let cells: Vec<String> = row
        .iter()
        .map(|v| match v.as_bool() {
            Some(true) => "T".to_string(),
            Some(false) => "F".to_string(),
            None => "?".to_string(),
        })
        .collect();
    format!("[{}]", cells.join(","))
}
