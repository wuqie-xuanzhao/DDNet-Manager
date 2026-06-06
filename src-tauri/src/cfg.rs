use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::models::{BindConflict, BindRecord, CfgAnalysis, CfgExecRecord, CfgUnbindRecord};

/// 解析 cfg 文本中的 bind 记录并保留来源文件与行号。
pub fn parse_cfg_binds(source_file: &str, content: &str) -> Vec<BindRecord> {
    analyze_cfg_text(source_file, content).binds
}

/// 从磁盘读取 cfg 文件并解析其中的 bind 记录。
pub fn parse_cfg_file(path: &Path) -> Result<Vec<BindRecord>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read cfg file: {error}"))?;
    let source_file = display_source_file(path);

    Ok(parse_cfg_binds(&source_file, &content))
}

/// 解析 cfg 文本中的 bind、unbind、exec 与冲突信息。
pub fn analyze_cfg_text(source_file: &str, content: &str) -> CfgAnalysis {
    let mut analysis = CfgAnalysis {
        binds: Vec::new(),
        unbinds: Vec::new(),
        execs: Vec::new(),
        conflicts: Vec::new(),
        missing_exec_targets: Vec::new(),
    };

    for (index, raw_line) in content.lines().enumerate() {
        let line_number = index + 1;
        if let Some(record) = parse_bind_line(source_file, line_number, raw_line) {
            analysis.binds.push(record);
            continue;
        }

        if let Some(record) = parse_unbind_line(source_file, line_number, raw_line) {
            analysis.unbinds.push(record);
            continue;
        }

        if let Some(record) = parse_exec_line(source_file, line_number, raw_line) {
            analysis.execs.push(record);
        }
    }

    analysis.conflicts = collect_conflicts(&analysis.binds);
    analysis
}

/// 从磁盘读取 cfg 文件并递归分析 bind、unbind、exec 与冲突信息。
pub fn analyze_cfg_file(path: &Path) -> Result<CfgAnalysis, String> {
    let canonical_root = canonicalize_with_fallback(path)?;
    let mut recursion_stack = HashSet::new();

    analyze_cfg_file_recursive(&canonical_root, &mut recursion_stack)
}

fn analyze_cfg_file_recursive(
    path: &Path,
    recursion_stack: &mut HashSet<PathBuf>,
) -> Result<CfgAnalysis, String> {
    let canonical_path = canonicalize_with_fallback(path)?;
    if !recursion_stack.insert(canonical_path.clone()) {
        return Ok(empty_analysis());
    }

    let content = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read cfg file: {error}"))?;
    let source_file = display_source_file(path);
    let mut analysis = analyze_cfg_text(&source_file, &content);

    let execs = analysis.execs.clone();
    for exec_record in execs {
        let resolved_path = resolve_exec_target(path, &exec_record.target);
        let resolved_display = resolved_path
            .as_ref()
            .map(|resolved| display_path(resolved));

        if let Some(existing) = analysis
            .execs
            .iter_mut()
            .find(|record| same_exec_record(record, &exec_record))
        {
            existing.resolved_path = resolved_display.clone();
            existing.missing = resolved_path
                .as_ref()
                .is_some_and(|candidate| !candidate.is_file());
        }

        match resolved_path {
            Some(candidate) if candidate.is_file() => {
                let nested_analysis = analyze_cfg_file_recursive(&candidate, recursion_stack)?;
                merge_analysis(&mut analysis, nested_analysis);
            }
            Some(candidate) => {
                let missing_record = CfgExecRecord {
                    target: exec_record.target.clone(),
                    source_file: exec_record.source_file.clone(),
                    line: exec_record.line,
                    resolved_path: Some(display_path(&candidate)),
                    missing: true,
                };
                replace_exec_record(&mut analysis.execs, &missing_record);
                analysis.missing_exec_targets.push(missing_record);
            }
            None => {}
        }
    }

    analysis.conflicts = collect_conflicts(&analysis.binds);
    recursion_stack.remove(&canonical_path);
    Ok(analysis)
}

fn parse_bind_line(source_file: &str, line_number: usize, raw_line: &str) -> Option<BindRecord> {
    let line = raw_line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    let bind_body = strip_bind_keyword(line)?;
    let (key, command) = split_bind_body(bind_body)?;

    Some(BindRecord {
        key: key.to_string(),
        command: strip_outer_quotes(command).to_string(),
        source_file: source_file.to_string(),
        line: line_number,
        managed_by_manager: false,
        matched_workshop_id: None,
    })
}

fn parse_unbind_line(
    source_file: &str,
    line_number: usize,
    raw_line: &str,
) -> Option<CfgUnbindRecord> {
    let line = raw_line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    let key = strip_command_keyword(line, "unbind")?;
    if key.split_whitespace().count() != 1 {
        return None;
    }

    Some(CfgUnbindRecord {
        key: key.to_string(),
        source_file: source_file.to_string(),
        line: line_number,
    })
}

fn parse_exec_line(source_file: &str, line_number: usize, raw_line: &str) -> Option<CfgExecRecord> {
    let line = raw_line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    let target = strip_command_keyword(line, "exec")?;

    Some(CfgExecRecord {
        target: strip_outer_quotes(target).to_string(),
        source_file: source_file.to_string(),
        line: line_number,
        resolved_path: None,
        missing: false,
    })
}

fn strip_bind_keyword(line: &str) -> Option<&str> {
    strip_command_keyword(line, "bind")
}

fn strip_command_keyword<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let bind_body = line.strip_prefix(keyword)?;
    let first = bind_body.chars().next()?;
    if !first.is_whitespace() {
        return None;
    }

    let trimmed = bind_body.trim_start();
    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed)
}

fn split_bind_body(bind_body: &str) -> Option<(&str, &str)> {
    let trimmed = bind_body.trim();
    let first_space = trimmed.find(char::is_whitespace)?;
    let key = trimmed[..first_space].trim();
    let command = trimmed[first_space..].trim();

    if key.is_empty() || command.is_empty() {
        return None;
    }

    Some((key, command))
}

fn strip_outer_quotes(command: &str) -> &str {
    if command.len() >= 2 && command.starts_with('"') && command.ends_with('"') {
        &command[1..command.len() - 1]
    } else {
        command
    }
}

fn collect_conflicts(records: &[BindRecord]) -> Vec<BindConflict> {
    let mut grouped = HashMap::<String, Vec<BindRecord>>::new();
    for record in records {
        grouped
            .entry(record.key.clone())
            .or_default()
            .push(record.clone());
    }

    let mut conflicts = grouped
        .into_iter()
        .filter_map(|(key, records)| {
            if records.len() >= 2 {
                Some(BindConflict { key, records })
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    conflicts.sort_by(|left, right| left.key.cmp(&right.key));
    conflicts
}

fn empty_analysis() -> CfgAnalysis {
    CfgAnalysis {
        binds: Vec::new(),
        unbinds: Vec::new(),
        execs: Vec::new(),
        conflicts: Vec::new(),
        missing_exec_targets: Vec::new(),
    }
}

fn merge_analysis(target: &mut CfgAnalysis, mut nested: CfgAnalysis) {
    target.binds.append(&mut nested.binds);
    target.unbinds.append(&mut nested.unbinds);
    target.execs.append(&mut nested.execs);
    target
        .missing_exec_targets
        .append(&mut nested.missing_exec_targets);
}

fn resolve_exec_target(current_file: &Path, target: &str) -> Option<PathBuf> {
    let current_dir = current_file.parent()?;
    Some(current_dir.join(target))
}

fn canonicalize_with_fallback(path: &Path) -> Result<PathBuf, String> {
    path.canonicalize().or_else(|_| {
        path.parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| format!("failed to resolve cfg file path: {}", path.display()))
            .map(|parent| parent.join(path.file_name().unwrap_or_default()))
    })
}

fn display_source_file(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map_or_else(|| "unknown.cfg".to_string(), ToString::to_string)
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace("\\\\?\\", "")
}

fn same_exec_record(left: &CfgExecRecord, right: &CfgExecRecord) -> bool {
    left.target == right.target && left.source_file == right.source_file && left.line == right.line
}

fn replace_exec_record(records: &mut [CfgExecRecord], replacement: &CfgExecRecord) {
    if let Some(existing) = records
        .iter_mut()
        .find(|record| same_exec_record(record, replacement))
    {
        *existing = replacement.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn analyze_cfg_text_collects_binds_unbinds_execs_and_conflicts() {
        let cfg = r#"
# comment
bind mouse3 "echo Kill; kill"
unbind mouse3
exec extra.cfg
bind mouse3 "say rebound"
"#;

        let analysis = analyze_cfg_text("settings_ddnet.cfg", cfg);

        assert_eq!(analysis.binds.len(), 2);
        assert_eq!(analysis.unbinds.len(), 1);
        assert_eq!(analysis.execs.len(), 1);
        assert_eq!(analysis.execs[0].target, "extra.cfg");
        assert_eq!(analysis.execs[0].resolved_path, None);
        assert!(!analysis.execs[0].missing);
        assert_eq!(analysis.conflicts.len(), 1);
        assert_eq!(analysis.conflicts[0].key, "mouse3");
        assert_eq!(analysis.conflicts[0].records.len(), 2);
        assert!(analysis.missing_exec_targets.is_empty());
    }

    #[test]
    fn analyze_cfg_file_recursively_collects_exec_targets_and_conflicts() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root_cfg_path = temp.path().join("settings_ddnet.cfg");
        let extra_cfg_path = temp.path().join("extra.cfg");

        fs::write(
            &root_cfg_path,
            "bind mouse3 \"echo root\"\nexec extra.cfg\nbind f1 \"say root\"\n",
        )
        .expect("write root");
        fs::write(
            &extra_cfg_path,
            "bind mouse3 \"echo nested\"\nunbind tab\nbind f2 \"say nested\"\n",
        )
        .expect("write extra");

        let analysis = analyze_cfg_file(&root_cfg_path).expect("analyze file");

        assert_eq!(analysis.binds.len(), 4);
        assert_eq!(analysis.unbinds.len(), 1);
        assert_eq!(analysis.execs.len(), 1);
        assert_eq!(analysis.execs[0].source_file, "settings_ddnet.cfg");
        assert_eq!(
            analysis.execs[0].resolved_path.as_deref(),
            Some(extra_cfg_path.to_string_lossy().as_ref())
        );
        assert!(!analysis.execs[0].missing);
        assert_eq!(analysis.conflicts.len(), 1);
        assert_eq!(analysis.conflicts[0].key, "mouse3");
        assert_eq!(analysis.conflicts[0].records.len(), 2);
        assert!(analysis.missing_exec_targets.is_empty());
    }

    #[test]
    fn analyze_cfg_file_tracks_missing_exec_target() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root_cfg_path = temp.path().join("settings_ddnet.cfg");

        fs::write(&root_cfg_path, "exec missing.cfg\nbind f1 \"say root\"\n").expect("write root");

        let analysis = analyze_cfg_file(&root_cfg_path).expect("analyze file");

        assert_eq!(analysis.execs.len(), 1);
        assert!(analysis.execs[0].missing);
        assert_eq!(analysis.missing_exec_targets.len(), 1);
        assert_eq!(analysis.missing_exec_targets[0], analysis.execs[0]);
    }

    #[test]
    fn analyze_cfg_file_skips_exec_cycles_without_error() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root_cfg_path = temp.path().join("settings_ddnet.cfg");
        let extra_cfg_path = temp.path().join("extra.cfg");

        fs::write(&root_cfg_path, "bind f1 \"say root\"\nexec extra.cfg\n").expect("write root");
        fs::write(
            &extra_cfg_path,
            "bind f2 \"say nested\"\nexec settings_ddnet.cfg\n",
        )
        .expect("write extra");

        let analysis = analyze_cfg_file(&root_cfg_path).expect("analyze file");

        assert_eq!(analysis.binds.len(), 2);
        assert_eq!(analysis.execs.len(), 2);
        assert!(analysis.missing_exec_targets.is_empty());
    }

    #[test]
    fn analyze_cfg_file_counts_shared_exec_file_for_each_branch() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root_cfg_path = temp.path().join("root.cfg");
        let a_cfg_path = temp.path().join("a.cfg");
        let b_cfg_path = temp.path().join("b.cfg");
        let shared_cfg_path = temp.path().join("shared.cfg");

        fs::write(&root_cfg_path, "exec a.cfg\nexec b.cfg\n").expect("write root");
        fs::write(&a_cfg_path, "bind f1 \"echo a\"\nexec shared.cfg\n").expect("write a");
        fs::write(&b_cfg_path, "bind f2 \"echo b\"\nexec shared.cfg\n").expect("write b");
        fs::write(&shared_cfg_path, "bind mouse3 \"echo shared\"\n").expect("write shared");

        let analysis = analyze_cfg_file(&root_cfg_path).expect("analyze file");

        assert_eq!(analysis.binds.len(), 4);
        let shared_binds = analysis
            .binds
            .iter()
            .filter(|record| {
                record.source_file == "shared.cfg"
                    && record.key == "mouse3"
                    && record.command == "echo shared"
            })
            .count();
        assert_eq!(shared_binds, 2);
        let shared_conflict = analysis
            .conflicts
            .iter()
            .find(|conflict| conflict.key == "mouse3")
            .expect("shared bind should conflict");
        assert_eq!(shared_conflict.records.len(), 2);
    }

    #[test]
    fn parse_cfg_file_keeps_shallow_bind_only_semantics() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root_cfg_path = temp.path().join("root.cfg");
        let child_cfg_path = temp.path().join("child.cfg");

        fs::write(&root_cfg_path, "bind f1 \"echo root\"\nexec child.cfg\n").expect("write root");
        fs::write(&child_cfg_path, "bind f2 \"echo child\"\n").expect("write child");

        let records = parse_cfg_file(&root_cfg_path).expect("parse file");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].key, "f1");
        assert_eq!(records[0].command, "echo root");
        assert_eq!(records[0].source_file, "root.cfg");
    }

    #[test]
    fn parses_bind_lines_with_source_line() {
        let cfg = r#"
# comment
bind mouse3 "echo Kill; kill"
bind ctrl+z "echo 自救; say /rescue"
"#;

        let records = parse_cfg_binds("settings_ddnet.cfg", cfg);

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].key, "mouse3");
        assert_eq!(records[0].command, "echo Kill; kill");
        assert_eq!(records[0].line, 3);
        assert_eq!(records[1].key, "ctrl+z");
    }

    #[test]
    fn parses_bind_lines_with_tab_and_multiple_spaces() {
        let cfg = "bind\tmouse3 \"echo tab\"\nbind    ctrl+z    \"echo spaces\"";

        let records = parse_cfg_binds("settings_ddnet.cfg", cfg);

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].key, "mouse3");
        assert_eq!(records[0].command, "echo tab");
        assert_eq!(records[1].key, "ctrl+z");
        assert_eq!(records[1].command, "echo spaces");
    }

    #[test]
    fn ignores_blank_comment_non_bind_and_incomplete_bind_lines() {
        let cfg = r#"

# comment
unbind mouse3
bindmouse3 "ignored"
bind
bind mouse3
bind mouse4 "echo kept"
"#;

        let records = parse_cfg_binds("settings_ddnet.cfg", cfg);

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].key, "mouse4");
        assert_eq!(records[0].command, "echo kept");
        assert_eq!(records[0].line, 8);
    }

    #[test]
    fn parses_cfg_file_from_disk() {
        let temp = tempfile::tempdir().expect("tempdir");
        let cfg_path = temp.path().join("settings_ddnet.cfg");
        fs::write(&cfg_path, "bind mouse3 \"echo Kill; kill\"\n").expect("write cfg");

        let records = parse_cfg_file(&cfg_path).expect("parse file");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].source_file, "settings_ddnet.cfg");
    }
}
