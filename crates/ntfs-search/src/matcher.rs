//! 谓词闭包的类型包装与工厂方法。
//!
//! crate 内部 backend 直接持有 `&(dyn Fn(&str) -> bool + Send + Sync)` 引用，
//! 不需要这个 Matcher 类型；它的作用是给调用方提供一个**类型安全的工厂**，
//! 把常见匹配模式（精确名/后缀/任一）封成可读 API，避免每次都要写裸闭包。

use std::sync::Arc;

/// 文件名匹配器。`Matcher::matches(name)` 调用底层闭包。
///
/// 内部用 `Arc<dyn Fn>` 共享，便于跨多个并发 backend clone。
#[derive(Clone)]
pub struct Matcher {
    predicate: Arc<dyn Fn(&str) -> bool + Send + Sync>,
}

impl Matcher {
    /// 从任意 `Fn(&str) -> bool + Send + Sync + 'static` 闭包构造。
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&str) -> bool + Send + Sync + 'static,
    {
        Self {
            predicate: Arc::new(f),
        }
    }

    /// 大小写不敏感的精确名匹配（任一）。常用于 DDNet.exe / ddnet.exe 这类组合。
    pub fn any_exact_ci<S: AsRef<str>>(names: &[S]) -> Self {
        let owned: Vec<String> = names
            .iter()
            .map(|s| s.as_ref().to_ascii_lowercase())
            .collect();
        Self::new(move |name: &str| owned.iter().any(|n| n == &name.to_ascii_lowercase()))
    }

    /// 大小写不敏感的后缀匹配（如 ".exe"）。
    pub fn suffix_ci(suffix: &str) -> Self {
        let lower = suffix.to_ascii_lowercase();
        Self::new(move |name: &str| name.to_ascii_lowercase().ends_with(&lower))
    }

    /// 测试一条 entry 的文件名是否匹配。
    #[inline]
    pub fn matches(&self, name: &str) -> bool {
        (self.predicate)(name)
    }
}

impl std::fmt::Debug for Matcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Matcher")
            .field("predicate", &"<closure>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closure_matcher_invokes_predicate() {
        let m = Matcher::new(|n| n == "DDNet.exe");
        assert!(m.matches("DDNet.exe"));
        assert!(!m.matches("ddnet.exe")); // 大小写敏感
    }

    #[test]
    fn any_exact_ci_matches_case_insensitive() {
        let m = Matcher::any_exact_ci(&["DDNet.exe", "ddnet"]);
        assert!(m.matches("DDNET.EXE"));
        assert!(m.matches("ddnet.exe"));
        assert!(m.matches("DDNet"));
        assert!(!m.matches("other.exe"));
    }

    #[test]
    fn suffix_ci_matches_extension() {
        let m = Matcher::suffix_ci(".exe");
        assert!(m.matches("foo.exe"));
        assert!(m.matches("FOO.EXE"));
        assert!(m.matches(".exe"));
        assert!(!m.matches("foo.ex"));
        assert!(!m.matches("foo"));
    }

    #[test]
    fn matcher_is_clone_and_send_sync() {
        fn assert_send_sync<T: Send + Sync + Clone>() {}
        assert_send_sync::<Matcher>();

        let m = Matcher::any_exact_ci(&["DDNet.exe"]);
        let m2 = m.clone();
        assert!(m.matches("DDNet.exe"));
        assert!(m2.matches("DDNet.exe"));
    }

    #[test]
    fn debug_does_not_panic_on_closure() {
        let m = Matcher::new(|_| true);
        let s = format!("{:?}", m);
        assert!(s.contains("Matcher"));
    }
}
