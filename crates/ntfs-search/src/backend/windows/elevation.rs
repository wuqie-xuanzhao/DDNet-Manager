//! 进程 UAC elevation 检测。普通（未提升）进程无 admin 权限，$MFT 和
//! FSCTL_ENUM_USN_DATA 的 raw volume 句柄都打不开——直接跳过这两个 backend
//! 走 Walkdir，省一次无效探测。

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{
    GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// 当前进程是否以 elevated（UAC 提权/admin）运行。
///
/// 实现走 `OpenProcessToken(GetCurrentProcess) → GetTokenInformation(TokenElevation)`。
/// 任何 API 失败都按"未提权"处理，让上游安全降级到 Walkdir。
pub(crate) fn is_process_elevated() -> bool {
    // SAFETY: GetCurrentProcess 返回 pseudo handle（不持有真实资源）；
    // OpenProcessToken 在 TOKEN_QUERY only 模式下不会修改进程状态；
    // GetTokenInformation 写入的 buffer 大小与 TOKEN_ELEVATION 匹配。
    unsafe {
        let mut token: HANDLE = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }

        let mut elevation = TOKEN_ELEVATION::default();
        let mut ret_len = 0u32;
        let result = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut TOKEN_ELEVATION as *mut std::ffi::c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret_len,
        );
        let _ = CloseHandle(token);

        result.is_ok() && elevation.TokenIsElevated != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_process_elevated_does_not_panic() {
        // 测试机器可能 elevated 也可能不 elevated，只验证函数能跑通不 panic。
        let _ = is_process_elevated();
    }
}
