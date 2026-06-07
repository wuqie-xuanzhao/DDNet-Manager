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
