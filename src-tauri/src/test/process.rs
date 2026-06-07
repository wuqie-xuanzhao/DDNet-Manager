use std::fs;

#[test]
fn recognizes_ddnet_process_names() {
    assert!(super::is_ddnet_process_name("DDNet.exe"));
    assert!(super::is_ddnet_process_name("ddnet.exe"));
    assert!(super::is_ddnet_process_name("DDNet"));
    assert!(super::is_ddnet_process_name("ddnet"));
}

#[test]
fn rejects_non_ddnet_process_names() {
    assert!(!super::is_ddnet_process_name("notepad.exe"));
}

#[test]
fn resolve_launch_target_rejects_non_ddnet_file_name() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let install_dir = temp_dir.path();
    fs::write(install_dir.join("notepad.exe"), b"").expect("测试文件应写入成功");

    let result = super::resolve_launch_target(&install_dir.join("notepad.exe"));

    assert!(result.is_err());
}

#[test]
fn resolve_launch_target_rejects_missing_file() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let install_dir = temp_dir.path();

    let result = super::resolve_launch_target(&install_dir.join("DDNet.exe"));

    assert!(result.is_err());
}

#[test]
fn is_client_running_rejects_missing_file() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");

    let result = super::is_client_running(&temp_dir.path().join("DDNet.exe"));

    assert!(result.is_err());
}

#[test]
fn is_client_running_matches_current_test_process_path() {
    let current_exe = std::env::current_exe().expect("测试进程路径应可读取");

    let running = super::is_client_running(&current_exe).expect("当前进程应可检测");

    assert!(running);
}

#[test]
fn resolve_launch_target_rejects_incomplete_client_directory() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let install_dir = temp_dir.path();
    fs::write(install_dir.join("DDNet.exe"), b"").expect("测试可执行文件应写入成功");

    let result = super::resolve_launch_target(&install_dir.join("DDNet.exe"));

    assert!(result.is_err());
}

#[test]
fn resolve_launch_target_accepts_complete_client_directory() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let install_dir = temp_dir.path();
    let executable_path = install_dir.join("DDNet.exe");
    fs::write(&executable_path, b"").expect("测试可执行文件应写入成功");
    fs::write(install_dir.join("storage.cfg"), b"").expect("测试 storage.cfg 应写入成功");
    fs::create_dir(install_dir.join("data")).expect("测试 data 目录应创建成功");

    let target = super::resolve_launch_target(&executable_path).expect("完整客户端应可解析");

    assert_eq!(target.executable_path, executable_path);
    assert_eq!(target.working_dir, install_dir);
}
