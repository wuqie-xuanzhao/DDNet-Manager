//! 压缩包解压的测试：zip / tar.xz / dmg。

use super::{write_tar_xz, write_zip};
use crate::download::{extract_package_to_staging, extract_zip_to_staging, PackageKind};
use std::fs;

#[test]
fn extract_zip_to_staging_extracts_safe_zip() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let zip_path = temp_dir.path().join("safe.zip");
    let staging_dir = temp_dir.path().join("staging");
    write_zip(&zip_path, &[("QmClient/DDNet.exe", b"exe".as_slice())]);

    extract_zip_to_staging(&zip_path, &staging_dir).expect("安全 zip 应解压成功");

    assert_eq!(
        fs::read(staging_dir.join("QmClient").join("DDNet.exe")).expect("解压文件应可读取"),
        b"exe"
    );
}

#[test]
fn extract_zip_to_staging_rejects_path_traversal() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let zip_path = temp_dir.path().join("evil.zip");
    let staging_dir = temp_dir.path().join("staging");
    write_zip(&zip_path, &[("../evil.txt", b"nope".as_slice())]);

    let error = extract_zip_to_staging(&zip_path, &staging_dir).expect_err("路径穿越应被拒绝");

    assert!(error.contains("unsafe zip entry path"));
    assert!(!temp_dir.path().join("evil.txt").exists());
}

#[test]
fn extract_zip_to_staging_rejects_symlink_entry() {
    // symlink entry：mode 标记为 S_IFLNK（0o120000），内容是 target 路径字节。
    // zip-rs 不会主动拒绝这种条目，extract_zip_to_staging 必须自己拒。
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let zip_path = temp_dir.path().join("symlink.zip");
    let staging_dir = temp_dir.path().join("staging");

    let file = fs::File::create(&zip_path).expect("测试 zip 文件应创建成功");
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    zip.add_symlink("evil_link", "/etc/passwd", options)
        .expect("symlink entry 应创建成功");
    zip.finish().expect("测试 zip 应写入完成");

    let error =
        extract_zip_to_staging(&zip_path, &staging_dir).expect_err("symlink entry 应被拒绝");

    assert!(error.contains("unsafe zip symlink entry"));
    assert!(!staging_dir.join("evil_link").exists());
}

#[test]
fn extract_tar_xz_to_staging_extracts_safe_archive() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let archive_path = temp_dir.path().join("client.tar.xz");
    let staging_dir = temp_dir.path().join("staging");
    write_tar_xz(
        &archive_path,
        &[
            ("QmClient/DDNet", b"exe".as_slice()),
            ("QmClient/storage.cfg", b"".as_slice()),
            ("QmClient/data/mapres/.keep", b"".as_slice()),
        ],
    );

    extract_package_to_staging(&archive_path, &staging_dir, PackageKind::TarXz)
        .expect("安全 tar.xz 应解包成功");

    assert_eq!(
        fs::read(staging_dir.join("QmClient").join("DDNet")).expect("解包文件应可读取"),
        b"exe"
    );
    assert!(staging_dir
        .join("QmClient")
        .join("data")
        .join("mapres")
        .exists());
}

#[test]
fn extract_tar_xz_to_staging_rejects_path_traversal() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let archive_path = temp_dir.path().join("evil.tar.xz");
    let staging_dir = temp_dir.path().join("staging");
    write_tar_xz(&archive_path, &[("../evil.txt", b"nope".as_slice())]);

    let error = extract_package_to_staging(&archive_path, &staging_dir, PackageKind::TarXz)
        .expect_err("tar.xz 路径穿越应被拒绝");

    assert!(error.contains("unsafe tar entry path"));
    assert!(!temp_dir.path().join("evil.txt").exists());
}

#[test]
fn extract_dmg_to_staging_has_platform_specific_manager_owned_boundary() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let dmg_path = temp_dir.path().join("client.dmg");
    let staging_dir = temp_dir.path().join("staging");
    fs::write(&dmg_path, b"not-a-real-dmg").expect("测试 dmg 应写入成功");

    let result = extract_package_to_staging(&dmg_path, &staging_dir, PackageKind::Dmg);

    if cfg!(target_os = "macos") {
        let error = result.expect_err("无效 dmg 应在 macOS 挂载阶段失败");
        assert!(error.contains("failed to attach dmg") || error.contains("failed to copy app"));
    } else {
        let error = result.expect_err("非 macOS 不能执行 dmg Manager-owned 安装");
        assert_eq!(
            error,
            "automatic .dmg install requires macOS hdiutil and app bundle copy support"
        );
    }
}

#[test]
fn extract_zip_to_staging_extracts_many_files_in_parallel() {
    // 100 个文件触发 Phase B 并行解压路径（>1 文件 → std::thread::scope）
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let zip_path = temp_dir.path().join("many.zip");
    let staging_dir = temp_dir.path().join("staging");

    let entries: Vec<(String, Vec<u8>)> = (0..100)
        .map(|i| (format!("data/file_{i:03}.bin"), vec![i as u8; 64]))
        .collect();
    let entries_ref: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
        .collect();
    write_zip(&zip_path, &entries_ref);

    extract_zip_to_staging(&zip_path, &staging_dir).expect("多文件 zip 应并行解压成功");

    for (name, expected) in &entries {
        let path = staging_dir.join(name);
        let actual = fs::read(&path)
            .unwrap_or_else(|e| panic!("解压文件 {name} 应存在且可读：{e}"));
        assert_eq!(actual, *expected, "解压文件 {name} 内容应匹配");
    }
}

#[test]
fn extract_zip_to_staging_rejects_symlink_in_mixed_zip() {
    // 混合 zip：一个安全文件 + 一个 symlink。Phase A 串行预处理应捕获 symlink，
    // Phase B 并行解压不应执行（staging 内不能有任何文件落地）。
    use std::io::Write;
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let zip_path = temp_dir.path().join("mixed.zip");
    let staging_dir = temp_dir.path().join("staging");

    let file = fs::File::create(&zip_path).expect("测试 zip 文件应创建成功");
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("safe.txt", options)
        .expect("安全 entry 应创建成功");
    zip.write_all(b"safe").expect("安全 entry 内容应写入");
    zip.add_symlink("evil_link", "/etc/passwd", options)
        .expect("symlink entry 应创建成功");
    zip.finish().expect("测试 zip 应写入完成");

    let error = extract_zip_to_staging(&zip_path, &staging_dir)
        .expect_err("混合 zip 中的 symlink 应在 Phase A 被拒绝");
    assert!(error.contains("unsafe zip symlink entry"));
}

#[test]
fn extract_zip_to_staging_single_file_uses_fast_path() {
    // 单文件 zip 走快速路径（不进 std::thread::scope），仍应正确解压
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let zip_path = temp_dir.path().join("single.zip");
    let staging_dir = temp_dir.path().join("staging");
    write_zip(&zip_path, &[("DDNet.exe", b"binary".as_slice())]);

    extract_zip_to_staging(&zip_path, &staging_dir).expect("单文件 zip 应解压成功");

    assert_eq!(
        fs::read(staging_dir.join("DDNet.exe")).expect("解压文件应可读取"),
        b"binary"
    );
}

#[test]
fn extract_zip_to_staging_preserves_directory_structure_in_parallel() {
    // 混合 目录 + 多层嵌套文件 + 多个平级文件，验证并行解压下目录结构正确
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let zip_path = temp_dir.path().join("nested.zip");
    let staging_dir = temp_dir.path().join("staging");
    write_zip(
        &zip_path,
        &[
            ("QmClient/DDNet.exe", b"exe".as_slice()),
            ("QmClient/data/maps/a.map", b"a".as_slice()),
            ("QmClient/data/maps/b.map", b"b".as_slice()),
            ("QmClient/data/mapres/x.png", b"x".as_slice()),
            ("QmClient/storage.cfg", b"cfg".as_slice()),
        ],
    );

    extract_zip_to_staging(&zip_path, &staging_dir).expect("嵌套 zip 应并行解压成功");

    assert_eq!(
        fs::read(staging_dir.join("QmClient").join("DDNet.exe")).unwrap(),
        b"exe"
    );
    assert_eq!(
        fs::read(staging_dir.join("QmClient").join("data").join("maps").join("a.map")).unwrap(),
        b"a"
    );
    assert!(staging_dir.join("QmClient").join("data").join("mapres").is_dir());
}