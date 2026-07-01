use super::content_length_matches;
use crate::download::race::select_best_source;
use reqwest::Client;
use tokio_util::sync::CancellationToken;

/// 构建测试用 reqwest 客户端：follow_redirects=true，无代理。
fn test_client() -> Client {
    Client::builder()
        .redirect(reqwest::redirect::Policy::default())
        .build()
        .expect("测试客户端应构建成功")
}

#[tokio::test]
async fn select_best_source_rejects_empty_candidates() {
    let client = test_client();
    let error = select_best_source(&client, &[], 100, None)
        .await
        .expect_err("空候选列表应返回错误");
    assert!(error.contains("no candidate urls"));
}

#[tokio::test]
async fn select_best_source_returns_winner_when_single_candidate_alive() {
    let mut server = mockito::Server::new_async().await;
    let body = vec![0u8; 1024];
    // HEAD 存活：200 + Accept-Ranges + Content-Length 符合
    let _head = server
        .mock("HEAD", "/file.zip")
        .with_status(200)
        .with_header("accept-ranges", "bytes")
        .with_header("content-length", "1024")
        .create_async()
        .await;
    // Range 测速返回 1024 字节
    let _range = server
        .mock("GET", "/file.zip")
        .match_header("range", "bytes=0-5242879")
        .with_status(206)
        .with_header("content-length", "1024")
        .with_body(body.clone())
        .create_async()
        .await;

    let url = format!("{}/file.zip", server.url());
    let candidate = vec![url.clone()];
    let winner = select_best_source(&test_client(), &candidate, 1024, None)
        .await
        .expect("单存活候选应返回胜出者");
    assert_eq!(winner.url, url);
    assert_eq!(winner.head_start.len(), 1024);
}

#[tokio::test]
async fn select_best_source_rejects_candidate_without_accept_ranges() {
    let mut server = mockito::Server::new_async().await;
    // HEAD 200 但无 Accept-Ranges header
    let _head = server
        .mock("HEAD", "/no-range.zip")
        .with_status(200)
        .with_header("content-length", "100")
        .create_async()
        .await;

    let url = format!("{}/no-range.zip", server.url());
    let error = select_best_source(&test_client(), &[url], 100, None)
        .await
        .expect_err("不支持 Range 的候选应被淘汰，导致全部不可达");
    assert!(error.contains("unreachable"));
}

#[test]
fn content_length_matches_rejects_mismatched_size() {
    // size 不符应淘汰（这是原集成测试想验证的逻辑）
    assert!(!content_length_matches(Some(999), 100));
    assert!(!content_length_matches(Some(1025), 1024));
}

#[test]
fn content_length_matches_accepts_zero_as_indeterminate() {
    // mockito HEAD 强制 content-length=0，视为无法判定，放行不淘汰
    assert!(content_length_matches(Some(0), 1024));
    assert!(content_length_matches(Some(0), 0));
}

#[test]
fn content_length_matches_accepts_none_as_indeterminate() {
    // 反代可能不返回 Content-Length 头
    assert!(content_length_matches(None, 1024));
    assert!(content_length_matches(None, 0));
}

#[test]
fn content_length_matches_accepts_exact_size() {
    assert!(content_length_matches(Some(1024), 1024));
    assert!(content_length_matches(Some(100), 100));
}

#[tokio::test]
async fn select_best_source_rejects_non_2xx_head() {
    let mut server = mockito::Server::new_async().await;
    let _head = server
        .mock("HEAD", "/forbidden.zip")
        .with_status(403)
        .create_async()
        .await;

    let url = format!("{}/forbidden.zip", server.url());
    let error = select_best_source(&test_client(), &[url], 100, None)
        .await
        .expect_err("403 HEAD 应被淘汰");
    assert!(error.contains("unreachable"));
}

#[tokio::test]
async fn select_best_source_picks_first_successful_when_multiple_alive() {
    let mut server = mockito::Server::new_async().await;
    // 两个候选都存活
    let _head_a = server
        .mock("HEAD", "/a.zip")
        .with_status(200)
        .with_header("accept-ranges", "bytes")
        .with_header("content-length", "100")
        .create_async()
        .await;
    let _head_b = server
        .mock("HEAD", "/b.zip")
        .with_status(200)
        .with_header("accept-ranges", "bytes")
        .with_header("content-length", "100")
        .create_async()
        .await;
    let _range_a = server
        .mock("GET", "/a.zip")
        .with_status(206)
        .with_body(vec![0u8; 100])
        .create_async()
        .await;
    let _range_b = server
        .mock("GET", "/b.zip")
        .with_status(206)
        .with_body(vec![1u8; 100])
        .create_async()
        .await;

    let url_a = format!("{}/a.zip", server.url());
    let url_b = format!("{}/b.zip", server.url());
    let candidates = vec![url_a.clone(), url_b.clone()];
    let winner = select_best_source(&test_client(), &candidates, 100, None)
        .await
        .expect("多存活候选应有胜出者");
    // 胜出者应是其中一个候选
    assert!(winner.url == url_a || winner.url == url_b);
    assert_eq!(winner.head_start.len(), 100);
}

#[tokio::test]
async fn select_best_source_skips_failed_range_probe_and_picks_next() {
    let mut server = mockito::Server::new_async().await;
    // a 的 HEAD 存活但 Range 返回 500
    let _head_a = server
        .mock("HEAD", "/a.zip")
        .with_status(200)
        .with_header("accept-ranges", "bytes")
        .with_header("content-length", "100")
        .create_async()
        .await;
    let _range_a = server
        .mock("GET", "/a.zip")
        .with_status(500)
        .create_async()
        .await;
    // b 的 HEAD 和 Range 都成功
    let _head_b = server
        .mock("HEAD", "/b.zip")
        .with_status(200)
        .with_header("accept-ranges", "bytes")
        .with_header("content-length", "100")
        .create_async()
        .await;
    let _range_b = server
        .mock("GET", "/b.zip")
        .with_status(206)
        .with_body(vec![1u8; 100])
        .create_async()
        .await;

    let url_a = format!("{}/a.zip", server.url());
    let url_b = format!("{}/b.zip", server.url());
    let candidates = vec![url_a, url_b.clone()];
    let winner = select_best_source(&test_client(), &candidates, 100, None)
        .await
        .expect("a 失败后应选 b");
    assert_eq!(winner.url, url_b);
}

#[tokio::test]
async fn select_best_source_returns_error_when_all_range_probes_fail() {
    let mut server = mockito::Server::new_async().await;
    let _head = server
        .mock("HEAD", "/x.zip")
        .with_status(200)
        .with_header("accept-ranges", "bytes")
        .with_header("content-length", "100")
        .create_async()
        .await;
    let _range = server
        .mock("GET", "/x.zip")
        .with_status(500)
        .create_async()
        .await;

    let url = format!("{}/x.zip", server.url());
    let error = select_best_source(&test_client(), &[url], 100, None)
        .await
        .expect_err("所有 Range 探测失败应返回错误");
    assert!(error.contains("failed range probe"));
}

#[tokio::test]
async fn select_best_source_respects_cancel_token() {
    let mut server = mockito::Server::new_async().await;
    // HEAD 存活但 Range 故意慢（mockito 默认立即返回，这里靠提前 cancel 测试）
    let _head = server
        .mock("HEAD", "/slow.zip")
        .with_status(200)
        .with_header("accept-ranges", "bytes")
        .with_header("content-length", "100")
        .create_async()
        .await;
    let _range = server
        .mock("GET", "/slow.zip")
        .with_status(206)
        .with_body(vec![0u8; 100])
        .create_async()
        .await;

    let url = format!("{}/slow.zip", server.url());
    let token = CancellationToken::new();
    token.cancel();
    let error = select_best_source(&test_client(), &[url], 100, Some(&token))
        .await
        .expect_err("已 cancel 的 token 应立即返回错误");
    assert_eq!(error, "download canceled");
}
