//! ntfs-search inspect smoke：扫盘 + inspect 拿 PE 版本资源。
//!
//! 用法：
//!     cargo run -p ntfs-search --example smoke_inspect
//!     cargo run -p ntfs-search --example smoke_inspect -- C:\Windows\System32 notepad.exe
//!
//! 演示 find_files + inspect_many 的完整调用链：业务层（DDNet-Manager）
//! 用此模式从扫描结果判定客户端身份。

use ntfs_search::{
    find_files, inspect_many, sink_from, InspectFields, InspectOutcome, NtfsScanOptions,
    ProgressEvent,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (root, target) = match args.as_slice() {
        [] => (PathBuf::from("C:\\Windows\\System32"), "notepad.exe"),
        [root, target] => (PathBuf::from(root), target.as_str()),
        _ => (PathBuf::from("C:\\Windows\\System32"), "notepad.exe"),
    };

    let matcher_target = target.to_string();
    let opts = NtfsScanOptions::new(move |n| n.eq_ignore_ascii_case(&matcher_target))
        .with_root(root.clone())
        .with_max_results(10);

    let sink = sink_from(|ev| match ev {
        ProgressEvent::DriveStarted { backend, .. } => {
            eprintln!("[scan] backend = {:?}", backend);
        }
        ProgressEvent::EntriesFound { found } => {
            eprintln!("[scan] found {} candidates so far", found);
        }
        _ => {}
    });

    eprintln!("=== step 1: scan {} for {} ===", root.display(), target);
    let started = Instant::now();
    let entries = find_files(opts, sink, CancellationToken::new()).await?;
    eprintln!(
        "=== scan done in {:?}, found {} ===\n",
        started.elapsed(),
        entries.len()
    );

    if entries.is_empty() {
        eprintln!("no matching files found");
        return Ok(());
    }

    eprintln!("=== step 2: inspect_many for VERSION_INFO ===");
    let started = Instant::now();
    let outcomes = inspect_many(
        &entries,
        InspectFields::VERSION_INFO,
        Arc::new(ntfs_search::NoopSink),
        8,
    )
    .await?;

    eprintln!("=== inspect done in {:?} ===\n", started.elapsed());

    for (i, outcome) in outcomes.iter().enumerate() {
        match outcome {
            InspectOutcome::Success(info) => {
                println!("[{i}] path: {}", entries[i].path.display());
                if let Some(vi) = &info.version_info {
                    println!(
                        "    CompanyName:       {}",
                        vi.company_name.as_deref().unwrap_or("-")
                    );
                    println!(
                        "    ProductName:       {}",
                        vi.product_name.as_deref().unwrap_or("-")
                    );
                    println!(
                        "    FileDescription:   {}",
                        vi.file_description.as_deref().unwrap_or("-")
                    );
                    println!(
                        "    FileVersion:       {}",
                        vi.file_version.as_deref().unwrap_or("-")
                    );
                    println!(
                        "    OriginalFilename:  {}",
                        vi.original_filename.as_deref().unwrap_or("-")
                    );
                } else {
                    println!("    no version_info");
                }
            }
            InspectOutcome::Failed { path, error } => {
                println!("[{i}] FAILED: {}: {error}", path.display());
            }
        }
    }

    Ok(())
}
