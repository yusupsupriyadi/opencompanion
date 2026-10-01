//! Models and thinking levels for the chat planner. Each CLI names them its own way: Claude
//! Code takes an alias plus `--effort`, Codex a catalog slug plus `model_reasoning_effort`,
//! OpenCode `provider/model` plus a per-model `--variant`, Pi and omp `provider/model` plus
//! `--thinking`, and Cursor CLI a model id that holds its thinking level.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use crate::cli::{CliInstall, CliKind};
use crate::proc;

const LIST_TIMEOUT: Duration = Duration::from_secs(30);

/// Thinking levels from least to most, the order the pickers show them in.
const EFFORT_ORDER: [&str; 8] = ["none", "minimal", "low", "medium", "high", "xhigh", "max", "ultra"];

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelOption {
    /// The value handed to the CLI's model flag.
    pub id: String,
    pub label: String,
    /// Provider, for a CLI that reaches several (OpenCode).
    pub group: Option<String>,
    /// Thinking levels this model takes; empty when the CLI offers none for it.
    pub efforts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelList {
    pub models: Vec<ModelOption>,
    /// Thinking levels that are safe with the CLI's own default model.
    pub default_efforts: Vec<String>,
}

fn sorted(mut efforts: Vec<String>) -> Vec<String> {
    efforts.sort_by_key(|e| EFFORT_ORDER.iter().position(|o| o == e).unwrap_or(EFFORT_ORDER.len()));
    efforts
}

/// Stand-in when Claude Code has no catalog on disk: the aliases from its `--help`, which it
/// resolves to the latest model of each family, with its `--effort` levels.
fn claude_aliases() -> ModelList {
    let efforts: Vec<String> = ["low", "medium", "high", "xhigh", "max"].map(String::from).to_vec();
    let models = [("fable", "Fable"), ("opus", "Opus"), ("sonnet", "Sonnet"), ("haiku", "Haiku")]
        .into_iter()
        .map(|(id, label)| ModelOption {
            id: id.into(),
            label: format!("{label} (latest)"),
            group: None,
            efforts: efforts.clone(),
        })
        .collect();
    ModelList {
        models,
        default_efforts: efforts,
    }
}

fn version_parts(v: &str) -> Vec<u64> {
    v.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).filter_map(|p| p.parse().ok()).collect()
}

/// Reads the catalog behind Claude Code's own `/model` picker: full names such as "Opus 5.5"
/// and each model's effort levels. Models that need a newer Claude Code than `version` are left
/// out. `None` when the file holds no models.
pub fn parse_claude_catalog(json: &str, version: Option<&str>) -> Option<ModelList> {
    let v: Value = serde_json::from_str(json).ok()?;
    let catalog = &v["catalog"];
    let have = version.map(version_parts);
    let mut models: Vec<ModelOption> = catalog["config"]["models"]
        .as_array()?
        .iter()
        .filter(|m| match (m["min_claude_code_version"].as_str(), &have) {
            (Some(min), Some(have)) => version_parts(min) <= *have,
            _ => true,
        })
        .filter_map(|m| {
            let id = m["id"].as_str()?.to_string();
            let efforts = m["thinking"]["effort_options"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|e| e["id"].as_str().map(str::to_owned))
                .collect();
            Some(ModelOption {
                label: m["name"].as_str().unwrap_or(&id).to_string(),
                // Claude Code lists its current models first and the rest under "more".
                group: (m["section"] != "main").then(|| "More models".to_string()),
                efforts: sorted(efforts),
                id,
            })
        })
        .collect();
    if models.is_empty() {
        return None;
    }
    models.sort_by_key(|m| m.group.is_some());
    // The levels of the model Claude Code picks when none is named.
    let default_efforts = catalog["state"]["model"]
        .as_str()
        .and_then(|d| models.iter().find(|m| m.id == d))
        .map(|m| m.efforts.clone())
        .unwrap_or_else(|| claude_aliases().default_efforts);
    Some(ModelList { models, default_efforts })
}

/// `CLAUDE_CONFIG_DIR` when set, else `~/.claude`, as Claude Code itself resolves it.
fn claude_config_dir() -> Option<PathBuf> {
    std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from).or_else(|| {
        std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(|h| PathBuf::from(h).join(".claude"))
    })
}

/// The newest `*-cc.json` in the model catalog caches of these Claude Code config folders (one
/// file per signed-in account).
fn claude_catalog(config_dirs: &[PathBuf]) -> Option<String> {
    let newest = config_dirs
        .iter()
        .filter_map(|d| fs::read_dir(d.join("cache").join("model-catalog")).ok())
        .flatten()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().ends_with("-cc.json"))
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())?;
    fs::read_to_string(newest.path()).ok()
}

fn claude(config_dirs: &[PathBuf], version: Option<&str>) -> ModelList {
    claude_catalog(config_dirs)
        .and_then(|text| parse_claude_catalog(&text, version))
        .unwrap_or_else(claude_aliases)
}

/// Reads `codex debug models`. Models Codex hides from its own picker stay hidden here too.
pub fn parse_codex(json: &str) -> Result<ModelList, String> {
    let v: Value = serde_json::from_str(json.trim()).map_err(|_| "Codex CLI printed no model catalog.".to_string())?;
    let models: Vec<ModelOption> = v["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|m| m["visibility"] == "list")
        .filter_map(|m| {
            let id = m["slug"].as_str()?.to_string();
            let efforts = m["supported_reasoning_levels"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|l| l["effort"].as_str().map(str::to_owned))
                .collect();
            Some(ModelOption {
                label: m["display_name"].as_str().unwrap_or(&id).to_string(),
                group: None,
                efforts: sorted(efforts),
                id,
            })
        })
        .collect();
    // The configured default can be any model, custom providers included, so only the levels
    // every listed model takes.
    let default_efforts = models
        .first()
        .map(|first| {
            first
                .efforts
                .iter()
                .filter(|e| models.iter().all(|m| m.efforts.contains(e)))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    Ok(ModelList { models, default_efforts })
}

/// Reads `opencode models --verbose`: each `provider/model` line is followed by that model's
/// JSON, whose `variants` keys are its thinking levels.
pub fn parse_opencode(text: &str) -> ModelList {
    let mut models = Vec::new();
    let mut rest = text.trim_start();
    while !rest.is_empty() {
        let (line, after) = rest.split_once('\n').unwrap_or((rest, ""));
        let id = line.trim();
        let mut stream = serde_json::Deserializer::from_str(after).into_iter::<Value>();
        match stream.next() {
            Some(Ok(v)) if id.contains('/') && v.is_object() => {
                rest = after[stream.byte_offset()..].trim_start();
                if v["status"] == "deprecated" {
                    continue;
                }
                let (provider, name) = id.split_once('/').unwrap_or(("", id));
                let efforts = v["variants"].as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
                models.push(ModelOption {
                    id: id.to_string(),
                    label: v["name"].as_str().unwrap_or(name).to_string(),
                    group: Some(provider.to_string()),
                    efforts: sorted(efforts),
                });
            }
            _ => rest = after.trim_start(),
        }
    }
    // A variant belongs to one model, so none is safe without knowing the default model.
    ModelList {
        models,
        default_efforts: vec![],
    }
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn output(kind: CliKind, exe: &Path, work_dir: &Path, args: &[&str]) -> Result<String, String> {
    output_when(kind, exe, work_dir, args, |_| false)
}

/// `output`, but a failed exit still counts when `complete` accepts what the CLI printed.
fn output_when(kind: CliKind, exe: &Path, work_dir: &Path, args: &[&str], complete: fn(&str) -> bool) -> Result<String, String> {
    std::fs::create_dir_all(work_dir).map_err(|e| e.to_string())?;
    let mut cmd = proc::command(exe);
    cmd.current_dir(work_dir).args(args);
    let out = proc::run_with_timeout(cmd, LIST_TIMEOUT).map_err(|e| format!("Could not start {}: {e}", kind.label()))?;
    if out.timed_out {
        return Err(format!("{} did not list its models within {} s.", kind.label(), LIST_TIMEOUT.as_secs()));
    }
    if out.status != Some(0) && !complete(&out.stdout) {
        let stderr = strip_ansi(&out.stderr);
        // omp's first stderr line can be its startup watchdog, which is not the reason.
        let reason = kind.runs_pi().then(|| crate::pi::stderr_reason(&stderr)).flatten();
        let why = reason
            .or_else(|| stderr.lines().map(str::trim).find(|l| !l.is_empty()).map(str::to_owned))
            .map(|l| l.chars().take(300).collect::<String>())
            .unwrap_or_else(|| format!("exit code {:?}", out.status));
        return Err(format!("{} could not list its models: {why}", kind.label()));
    }
    Ok(out.stdout)
}

/// The models `cli` can plan with. `extra` is the CLI's extra arguments from Settings.
pub fn list(cli: &CliInstall, work_dir: &Path, extra: &[String]) -> Result<ModelList, String> {
    let kind = cli.kind;
    let exe = cli.path.as_deref().ok_or(format!("{} is not installed.", kind.label()))?;
    match kind {
        CliKind::Claude => Ok(claude(&Vec::from_iter(claude_config_dir()), cli.version.as_deref())),
        // CCS reports its own version, not Claude Code's, so no model is left out as too new.
        CliKind::Ccs => Ok(claude(&crate::projects::home().map(|h| crate::ccs::claude_dirs(&h)).unwrap_or_default(), None)),
        CliKind::Codex => parse_codex(&output(kind, exe, work_dir, &["debug", "models"])?),
        CliKind::Opencode => {
            let pure = ["models", "--verbose", "--pure"];
            let text = if extra.iter().any(|a| a == "--pure") {
                output(kind, exe, work_dir, &pure)?
            } else {
                // A plugin that fails to load breaks the listing; without plugins it still works.
                output(kind, exe, work_dir, &["models", "--verbose"]).or_else(|_| output(kind, exe, work_dir, &pure))?
            };
            Ok(parse_opencode(&text))
        }
        CliKind::Gemini => Err("Gemini CLI cannot be the chat planner yet.".into()),
        CliKind::Terminal => Err("A blank terminal cannot be the chat planner.".into()),
        CliKind::Pi => Ok(crate::pi::parse_models(&output(kind, exe, work_dir, &["--list-models"])?)),
        CliKind::Omp => {
            let text = output_when(kind, exe, work_dir, &["models", "--json"], crate::pi::omp_models_complete)?;
            Ok(crate::pi::parse_omp_models(&text))
        }
        CliKind::Cursor => Ok(crate::cursor::parse_models(&output(kind, exe, work_dir, &["--list-models"])?)),
    }
}

/// A model id is passed as its own argument; one that could read as a flag is refused.
fn plain_model(m: &str) -> bool {
    !m.is_empty() && !m.starts_with('-') && m.chars().all(|c| c.is_ascii_alphanumeric() || "._-/:@[]".contains(c))
}

/// Flags that run the planner on `model` with `effort`. An empty value keeps the CLI's own
/// default.
pub fn args(kind: CliKind, model: &str, effort: &str) -> Vec<String> {
    let model = Some(model.trim()).filter(|m| plain_model(m));
    let effort = Some(effort.trim()).filter(|e| !e.is_empty() && e.chars().all(|c| c.is_ascii_lowercase()));
    let (model_flag, effort_flag) = match kind {
        CliKind::Claude | CliKind::Ccs => ("--model", "--effort"),
        CliKind::Codex => ("-m", "-c"),
        CliKind::Opencode => ("-m", "--variant"),
        CliKind::Gemini | CliKind::Terminal => return vec![],
        CliKind::Pi | CliKind::Omp => ("--model", "--thinking"),
        // The thinking level is part of a Cursor model id, such as `sonnet-4-thinking`.
        CliKind::Cursor => ("--model", ""),
    };
    let mut out = Vec::new();
    if let Some(m) = model {
        out.extend([model_flag.to_string(), m.to_string()]);
    }
    if let Some(e) = effort.filter(|_| !effort_flag.is_empty()) {
        // Codex reads a `-c` value that is not valid TOML as a plain string, so no quotes.
        let value = if kind == CliKind::Codex { format!("model_reasoning_effort={e}") } else { e.to_string() };
        out.extend([effort_flag.to_string(), value]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_lists_visible_models_and_the_levels_they_share() {
        let json = r#"{"models":[
            {"slug":"gpt-6-astra","display_name":"GPT-6-Astra","visibility":"list","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"max"}]},
            {"slug":"gpt-5.4","display_name":"GPT-5.4","visibility":"hide","supported_reasoning_levels":[{"effort":"low"}]},
            {"slug":"gpt-5.2","display_name":"GPT-5.2","visibility":"list","supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"high"},{"effort":"xhigh"}]}
        ]}"#;
        let list = parse_codex(json).unwrap();
        let ids: Vec<&str> = list.models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["gpt-6-astra", "gpt-5.2"]);
        assert_eq!(list.models[0].label, "GPT-6-Astra");
        assert_eq!(list.models[0].efforts, ["low", "medium", "high", "max"]);
        assert_eq!(list.default_efforts, ["low", "medium", "high"]);
        assert!(parse_codex("not json").is_err());
    }

    #[test]
    fn opencode_reads_each_model_and_orders_its_variants() {
        let text = "opencode/big-pickle\n{\n  \"id\": \"big-pickle\",\n  \"name\": \"Big Pickle\",\n  \"variants\": {}\n}\r\n\
            openai/gpt-5.5\n{\"name\":\"GPT-5.5\",\"variants\":{\"xhigh\":{},\"high\":{},\"none\":{},\"low\":{},\"medium\":{}}}\n\
            openai/gpt-3\n{\"name\":\"GPT-3\",\"status\":\"deprecated\",\"variants\":{}}\n\
            inferhub/cb/gpt-6-astra\n{\"name\":\"cb/gpt-6-astra\"}\n";
        let list = parse_opencode(text);
        let ids: Vec<&str> = list.models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["opencode/big-pickle", "openai/gpt-5.5", "inferhub/cb/gpt-6-astra"]);
        assert_eq!(list.models[1].efforts, ["none", "low", "medium", "high", "xhigh"]);
        assert_eq!(list.models[1].group.as_deref(), Some("openai"));
        assert_eq!(list.models[2].group.as_deref(), Some("inferhub"));
        assert!(list.models[0].efforts.is_empty());
        assert!(list.default_efforts.is_empty());
    }

    #[test]
    fn claude_catalog_gives_full_names_and_each_models_levels() {
        let effort = |ids: &[&str]| {
            let opts: Vec<Value> = ids.iter().map(|i| serde_json::json!({ "id": i })).collect();
            serde_json::json!({ "type": "effort", "effort_options": opts })
        };
        let json = serde_json::json!({
            "version": 2,
            "catalog": {
                "config": { "models": [
                    { "id": "claude-opus-4-8", "name": "Opus 4.8", "section": "overflow", "thinking": effort(&["low", "high"]) },
                    { "id": "claude-opus-5-5", "name": "Opus 5.5", "section": "main", "thinking": effort(&["max", "low", "medium"]), "min_claude_code_version": "2.1.280" },
                    { "id": "claude-next", "name": "Next", "section": "main", "thinking": effort(&["low"]), "min_claude_code_version": "2.2.0" },
                    { "id": "claude-haiku-4-5-20251001", "name": "Haiku 4.5", "section": "main", "thinking": { "type": "none" } }
                ]},
                "state": { "model": "claude-opus-5-5" }
            }
        })
        .to_string();
        let list = parse_claude_catalog(&json, Some("2.1.282")).unwrap();
        let names: Vec<&str> = list.models.iter().map(|m| m.label.as_str()).collect();
        assert_eq!(names, ["Opus 5.5", "Haiku 4.5", "Opus 4.8"]);
        assert_eq!(list.models[0].id, "claude-opus-5-5");
        assert_eq!(list.models[0].efforts, ["low", "medium", "max"]);
        assert!(list.models[1].efforts.is_empty());
        assert_eq!(list.models[2].group.as_deref(), Some("More models"));
        assert_eq!(list.default_efforts, ["low", "medium", "max"]);

        let older = parse_claude_catalog(&json, Some("2.1.279")).unwrap();
        assert!(!older.models.iter().any(|m| m.id == "claude-opus-5-5"));
        assert!(parse_claude_catalog(r#"{"catalog":{"config":{"models":[]}}}"#, None).is_none());
        assert!(parse_claude_catalog("not json", None).is_none());
    }

    #[test]
    fn plain_model_ids_without_verbose_json_are_skipped() {
        assert!(parse_opencode("openai/gpt-5.5\nopenai/gpt-5.4\n").models.is_empty());
    }

    #[test]
    fn each_cli_gets_its_own_flags() {
        assert_eq!(args(CliKind::Claude, "opus", "high"), ["--model", "opus", "--effort", "high"]);
        assert_eq!(args(CliKind::Codex, "gpt-5.2", "xhigh"), ["-m", "gpt-5.2", "-c", "model_reasoning_effort=xhigh"]);
        assert_eq!(args(CliKind::Opencode, "openai/gpt-5.5", "low"), ["-m", "openai/gpt-5.5", "--variant", "low"]);
        assert_eq!(args(CliKind::Claude, "", "max"), ["--effort", "max"]);
        assert_eq!(args(CliKind::Codex, "", ""), Vec::<String>::new());
        assert_eq!(args(CliKind::Gemini, "gemini-pro", "high"), Vec::<String>::new());
        assert_eq!(args(CliKind::Pi, "anthropic/claude-sonnet-4-5", "high"), ["--model", "anthropic/claude-sonnet-4-5", "--thinking", "high"]);
        assert_eq!(args(CliKind::Omp, "amazon-bedrock/anthropic.claude-opus-4-6-v1", "max"), ["--model", "amazon-bedrock/anthropic.claude-opus-4-6-v1", "--thinking", "max"]);
        assert_eq!(args(CliKind::Cursor, "sonnet-4-thinking", "high"), ["--model", "sonnet-4-thinking"]);
    }

    #[test]
    fn values_that_could_read_as_flags_are_dropped() {
        assert_eq!(args(CliKind::Claude, "--dangerously-skip-permissions", "high"), ["--effort", "high"]);
        assert_eq!(args(CliKind::Claude, "opus", "high --tools Bash"), ["--model", "opus"]);
        assert_eq!(args(CliKind::Claude, "opus[1m]", ""), ["--model", "opus[1m]"]);
    }

    #[test]
    fn ansi_colours_leave_error_lines() {
        assert_eq!(strip_ansi("\u{1b}[91m\u{1b}[1mError: \u{1b}[0mUnexpected error"), "Error: Unexpected error");
    }
}
