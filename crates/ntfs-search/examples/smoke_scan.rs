//! ntfs-search smoke 测试：扫描系统盘找已知存在的文件，验证 M1 端到端可用。
//!
//! 用法：
//!     cargo run -p ntfs-search --example smoke_scan
//!     cargo run -p ntfs-search --example smoke_scan -- C: D:
//!     cargo run -p ntfs-search --example smoke_scan -- E:/Games DDNet.exe
//!
//! 不修改任何业务代码，仅做 ntfs-search 自身能力验证。

use ntfs_search::{
    find_files, sink_from, BackendKind, NtfsScanOptions, ProgressEvent,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // 默认扫 C:\ 找 notepad.exe（系统文件，一定存在）
    let (roots, target) = match args.as_slice() {
        [] => (vec![PathBuf::from("C:\\")], "notepad.exe"),
        [single] => {
            // 单参数：若含 . 视为 target，否则视为 root（默认 notepad.exe）
            if single.contains('.') {
                (vec![PathBuf::from("C:\\")], single.as_str())
            } else {
                (vec![PathBuf::from(single)], "notepad.exe")
            }
        }
        [last @ .., target] if !last.is_empty() => {
            let roots: Vec<PathBuf> = last.iter().cloned().map(PathBuf::from).collect();
            (roots, target.as_str())
        }
        _ => (vec![PathBuf::from("C:\\")], "notepad.exe"),
    };

    let matcher_target = target.to_string();
    let opts = NtfsScanOptions::new(move |name| name.eq_ignore_ascii_case(&matcher_target))
        .with_roots(roots.clone())
        .with_max_results(20)
        .with_max_records_scanned(2_000_000)
        .with_timeout(std::time::Duration::from_secs(60));

    let started = Instant::now();
    let found_count = Arc::new(AtomicUsize::new(0));
    let scanned_count = Arc::new(AtomicUsize::new(0));
    let downgrades = Arc::new(AtomicUsize::new(0));
    let errors = Arc::new(AtomicUsize::new(0));
    let backends = Arc::new(std::sync::Mutex::new(Vec::<BackendKind>::new()));

    let sink = sink_from({
        let found_count = Arc::clone(&found_count);
        let scanned_count = Arc::clone(&scanned_count);
        let downgrades = Arc::clone(&downgrades);
        let errors = Arc::clone(&errors);
        let backends = Arc::clone(&backends);
        move |ev| match ev {
            ProgressEvent::DriveStarted { backend, .. } => {
                backends.lock().unwrap().push(backend);
                eprintln!("[start] backend = {:?}", backend);
            }
            ProgressEvent::EntriesFound { found } => {
                found_count.store(found, Ordering::Relaxed);
            }
            ProgressEvent::DriveCompleted { scanned, found, .. } => {
                scanned_count.store(scanned, Ordering::Relaxed);
                found_count.store(found, Ordering::Relaxed);
            }
            ProgressEvent::BackendDowngraded { from, to, reason, .. } => {
                downgrades.fetch_add(1, Ordering::Relaxed);
                eprintln!("[downgrade] {:?} -> {:?}: {}", from, to, reason);
            }
            ProgressEvent::EntryError { .. } => {
                errors.fetch_add(1, Ordering::Relaxed);
            }
            ProgressEvent::ScanLimitHit { kind, limit } => {
                eprintln!("[limit] {:?}={}", kind, limit);
            }
            ProgressEvent::DriveSkipped { root, reasons } => {
                eprintln!("[skip] {} ({})", root.display(), reasons.join(", "));
            }
        }
    });

    eprintln!(
        "=== smoke_scan: roots={:?} target={} ===",
        roots, target
    );
    let entries = find_files(opts, sink, CancellationToken::new()).await?;
    let elapsed = started.elapsed();

    println!();
    println!("=== results ===");
    println!("backend(s) used: {:?}", backends.lock().unwrap());
    println!(
        "scanned {} records, found {} entries in {:.2?}",
        scanned_count.load(Ordering::Relaxed),
        entries.len(),
        elapsed
    );
    println!("downgrades: {}, entry_errors: {}",
        downgrades.load(Ordering::Relaxed),
        errors.load(Ordering::Relaxed));
    println!();
    println!("matched entries:");
    for (i, entry) in entries.iter().enumerate() {
        println!(
            "  [{}] {} ({} bytes, modified {:?})",
            i,
            entry.path.display(),
            entry.size,
            entry.modified
        );
    }

    Ok(())
}
