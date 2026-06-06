#!/usr/bin/env bash
# =============================================================================
# DDNet Manager 代码合规性检查脚本
# 用法: bash scripts/check_lint.sh [--fix] [--diff <ref>] [--github-actions]
#   --fix              自动执行 cargo fmt（默认仅报告）
#   --diff <ref>       增量模式，结构性检查只扫描 git diff 的文件
#   --github-actions   输出 GitHub Actions 注解格式
# =============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
RUST_SRC_DIR="$PROJECT_ROOT/src-tauri/src"
CARGO_MANIFEST="$PROJECT_ROOT/src-tauri/Cargo.toml"
CARGO_LOCK="$PROJECT_ROOT/src-tauri/Cargo.lock"

MAX_FILE_LINES=600
HARD_MAX_FILE_LINES=1000
MAX_FUNCTION_LINES=80
MAX_FUNCTION_PARAMS=4

C_PASS='\033[32m'; C_WARN='\033[33m'; C_FAIL='\033[31m'
C_INFO='\033[36m'; C_BOLD='\033[1m'; C_RST='\033[0m'

DO_FIX=false
DO_DIFF=false
DIFF_REF=""
DO_GHA=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --fix) DO_FIX=true; shift ;;
        --diff)
            DO_DIFF=true
            if [[ $# -ge 2 && "$2" != --* ]]; then
                DIFF_REF="$2"
                shift 2
            else
                DIFF_REF="HEAD"
                shift
            fi
            ;;
        --github-actions) DO_GHA=true; shift ;;
        *) shift ;;
    esac
done

N_PASS=0
N_WARN=0
N_FAIL=0
TEMP_FILES=()

cleanup_temp() { rm -f "${TEMP_FILES[@]}" 2>/dev/null || true; }
trap cleanup_temp EXIT

make_temp() {
    local t
    t="$(mktemp)"
    TEMP_FILES+=("$t")
    printf '%s' "$t"
}

gha_emit() {
    local level="$1"
    local msg="$2"
    local file="${3:-}"
    local line="${4:-}"
    if $DO_GHA; then
        if [[ -n "$file" && -n "$line" ]]; then
            printf "::%s file=%s,line=%s::%s\n" "$level" "$file" "$line" "$msg"
        elif [[ -n "$file" ]]; then
            printf "::%s file=%s::%s\n" "$level" "$file" "$msg"
        else
            printf "::%s::%s\n" "$level" "$msg"
        fi
    fi
}

pass() {
    ((N_PASS++)) || true
    printf "  ${C_PASS}PASS${C_RST} %s\n" "$*"
    gha_emit "notice" "$*"
}

warn() {
    ((N_WARN++)) || true
    printf "  ${C_WARN}WARN${C_RST} %s\n" "$*"
    gha_emit "warning" "$*"
}

fail() {
    ((N_FAIL++)) || true
    printf "  ${C_FAIL}FAIL${C_RST} %s\n" "$*"
    gha_emit "error" "$*"
}

info() { printf "  ${C_INFO}INFO${C_RST} %s\n" "$*"; }
hdr() { printf "\n${C_BOLD}%s${C_RST}\n" "$*"; }

all_rs() {
    if $DO_DIFF && [[ -n "$DIFF_REF" ]]; then
        git diff --name-only "$DIFF_REF" -- '*.rs' 2>/dev/null |
            while IFS= read -r f; do
                [[ -f "$PROJECT_ROOT/$f" ]] && printf '%s\n' "$PROJECT_ROOT/$f"
            done
    else
        find "$RUST_SRC_DIR" -name '*.rs' -not -path '*/target/*'
    fi
}

all_source_for_placeholder_scan() {
    if $DO_DIFF && [[ -n "$DIFF_REF" ]]; then
        git diff --name-only "$DIFF_REF" 2>/dev/null |
            while IFS= read -r f; do
                [[ -f "$PROJECT_ROOT/$f" ]] && printf '%s\n' "$PROJECT_ROOT/$f"
            done | grep -E '\.(rs|ts|tsx|js|jsx)$' || true
    else
        find "$PROJECT_ROOT/src" "$PROJECT_ROOT/src-tauri/src" \
            -type f \
            \( -name '*.rs' -o -name '*.ts' -o -name '*.tsx' -o -name '*.js' -o -name '*.jsx' \) \
            -not -path '*/target/*' \
            -not -path '*/dist/*'
    fi
}

resolve_bin() {
    local name="$1"
    local path=""
    path="$(command -v "$name" 2>/dev/null || true)"
    if [[ -z "$path" && "$name" != *.exe ]]; then
        path="$(command -v "${name}.exe" 2>/dev/null || true)"
    fi
    if [[ -z "$path" ]] && command -v powershell.exe >/dev/null 2>&1; then
        path="$(powershell.exe -NoProfile -Command "(Get-Command ${name}.exe -ErrorAction SilentlyContinue).Path" 2>/dev/null | tr -d '\r' | tail -n 1)"
    fi
    printf '%s' "$path"
}

to_host_path() {
    local path="$1"
    if [[ "$path" == /mnt/* ]] && command -v wslpath >/dev/null 2>&1; then
        wslpath -w "$path"
    elif command -v cygpath >/dev/null 2>&1; then
        cygpath -w "$path"
    else
        printf '%s' "$path"
    fi
}

run_with_timeout() {
    local timeout_sec="${1:-300}"
    shift
    if command -v timeout >/dev/null 2>&1 && timeout --version 2>/dev/null | grep -q 'GNU coreutils'; then
        timeout "$timeout_sec" "$@"
    else
        "$@"
    fi
}

show_log_tail() {
    local log="$1"
    local lines="${2:-40}"
    tail -n "$lines" "$log"
}

CARGO_BIN="$(resolve_bin cargo)"
BUN_BIN="$(resolve_bin bun)"
CARGO_MANIFEST_NATIVE="$(to_host_path "$CARGO_MANIFEST")"
CARGO_LOCK_NATIVE="$(to_host_path "$CARGO_LOCK")"

if [[ -z "$CARGO_BIN" ]]; then
    fail "未找到 cargo"
fi
if [[ -z "$BUN_BIN" ]]; then
    fail "未找到 bun"
fi

hdr "=== A 组：Rust 工具链 ==="

hdr "=== A1. Rust 代码格式 (cargo fmt) ==="
if $DO_FIX; then
    "$CARGO_BIN" fmt --manifest-path "$CARGO_MANIFEST_NATIVE"
    pass "cargo fmt 已自动格式化"
else
    if "$CARGO_BIN" fmt --manifest-path "$CARGO_MANIFEST_NATIVE" -- --check 2>/dev/null; then
        pass "cargo fmt 检查通过"
    else
        fail "cargo fmt 未通过，运行 'make fmt' 或 'make check-lint-fix'"
    fi
fi

hdr "=== A2. Clippy 静态分析 (-D warnings) ==="
clippy_log="$(make_temp)"
if run_with_timeout 300 "$CARGO_BIN" clippy --manifest-path "$CARGO_MANIFEST_NATIVE" -- -D warnings >"$clippy_log" 2>&1; then
    pass "clippy 零告警"
else
    show_log_tail "$clippy_log" 80
    fail "clippy 存在告警或错误"
fi

hdr "=== A3. Rust 测试 (cargo test) ==="
rust_test_log="$(make_temp)"
if run_with_timeout 300 "$CARGO_BIN" test --manifest-path "$CARGO_MANIFEST_NATIVE" >"$rust_test_log" 2>&1; then
    pass "Rust 测试全部通过"
else
    show_log_tail "$rust_test_log" 80
    fail "Rust 测试存在失败"
fi

hdr "=== A4. 依赖安全审计 (cargo audit，可选) ==="
if "$CARGO_BIN" audit --version >/dev/null 2>&1; then
    audit_log="$(make_temp)"
    audit_exit=0
    "$CARGO_BIN" audit -f "$CARGO_LOCK_NATIVE" >"$audit_log" 2>&1 || audit_exit=$?
    if [[ "$audit_exit" -ne 0 ]]; then
        show_log_tail "$audit_log" 80
        fail "cargo audit 发现已知漏洞"
    elif grep -qE 'warning: .*allowed warnings found|Warning:|unmaintained|unsound|yanked' "$audit_log" 2>/dev/null; then
        show_log_tail "$audit_log" 20
        warn "cargo audit 发现上游告警"
    else
        pass "cargo audit 无已知漏洞"
    fi
else
    warn "cargo audit 不可用，跳过（安装: cargo install cargo-audit）"
fi

hdr "=== B 组：前端工具链 ==="

hdr "=== B1. Bun 锁文件一致性 ==="
lockfile_log="$(make_temp)"
if (cd "$PROJECT_ROOT" && "$BUN_BIN" install --frozen-lockfile) >"$lockfile_log" 2>&1; then
    pass "bun lockfile 一致"
else
    show_log_tail "$lockfile_log" 80
    fail "bun lockfile 不一致，运行 'make install' 更新"
fi

hdr "=== B2. TypeScript 类型检查 ==="
ts_log="$(make_temp)"
if (cd "$PROJECT_ROOT" && "$BUN_BIN" run check) >"$ts_log" 2>&1; then
    pass "TypeScript 类型检查通过"
else
    show_log_tail "$ts_log" 80
    fail "TypeScript 类型检查存在错误"
fi

hdr "=== C 组：Rust 结构性扫描 ==="

hdr "=== C1. 单文件行数 (WARN > $MAX_FILE_LINES | FAIL >= $HARD_MAX_FILE_LINES) ==="
oversized=0
while IFS= read -r f; do
    lines=$(wc -l < "$f")
    rel="${f#$PROJECT_ROOT/}"
    if (( lines >= HARD_MAX_FILE_LINES )); then
        fail "$rel — ${lines} 行 (>= ${HARD_MAX_FILE_LINES})"
        ((oversized++)) || true
    elif (( lines > MAX_FILE_LINES )); then
        warn "$rel — ${lines} 行 (> ${MAX_FILE_LINES})"
        ((oversized++)) || true
    fi
done < <(all_rs)
if (( oversized == 0 )); then
    pass "所有 Rust 文件 <= ${MAX_FILE_LINES} 行"
fi

hdr "=== C2. 单函数行数 (> $MAX_FUNCTION_LINES 行 → WARN) ==="
fn_warn=0
while IFS= read -r f; do
    rel="${f#$PROJECT_ROOT/}"
    result=$(awk -v file="$rel" -v max="$MAX_FUNCTION_LINES" '
    function clean(line) {
        gsub(/\/\/.*$/, "", line)
        gsub(/"([^"\\]|\\.)*"/, "", line)
        return line
    }
    /^[[:space:]]*(pub(\([^)]*\))?[[:space:]]+)?(async[[:space:]]+)?fn[[:space:]]+[A-Za-z_][A-Za-z0-9_]*/ {
        start = NR
        sig = $0
        depth = 0
        opened = 0
        do {
            line = clean($0)
            for (i = 1; i <= length(line); i++) {
                c = substr(line, i, 1)
                if (c == "{") { depth++; opened = 1 }
                if (c == "}") depth--
            }
            if (opened && depth <= 0) {
                len = NR - start + 1
                if (len > max) {
                    sub(/^[[:space:]]+/, "", sig)
                    sub(/[[:space:]]*\{.*$/, "", sig)
                    printf "%s — %s (%d 行)\n", file, sig, len
                }
                next
            }
        } while (getline > 0)
    }
    ' "$f")
    if [[ -n "$result" ]]; then
        while IFS= read -r line; do
            [[ -z "$line" ]] && continue
            warn "$line"
            ((fn_warn++)) || true
        done <<< "$result"
    fi
done < <(all_rs)
if (( fn_warn == 0 )); then
    pass "所有 Rust 函数 <= ${MAX_FUNCTION_LINES} 行"
fi

hdr "=== C3. 函数参数数量 (> $MAX_FUNCTION_PARAMS → WARN) ==="
param_warn=0
while IFS= read -r f; do
    rel="${f#$PROJECT_ROOT/}"
    result=$(awk -v file="$rel" -v max="$MAX_FUNCTION_PARAMS" '
    /^[[:space:]]*(pub(\([^)]*\))?[[:space:]]+)?(async[[:space:]]+)?fn[[:space:]]+[A-Za-z_][A-Za-z0-9_]*/ {
        sig = $0
        while (index(sig, ")") == 0 && getline > 0) sig = sig " " $0
        l = index(sig, "(")
        r = index(sig, ")")
        if (l == 0 || r <= l) next
        params = substr(sig, l + 1, r - l - 1)
        gsub(/[[:space:]]+/, " ", params)
        if (params == "") next
        n = 1
        for (i = 1; i <= length(params); i++) {
            if (substr(params, i, 1) == ",") n++
        }
        if (params ~ /^&?mut?[[:space:]]*self$/ || params ~ /^&self$/ || params ~ /^self$/) n = 0
        if (n > max) {
            line = $0
            sub(/^[[:space:]]+/, "", line)
            sub(/[[:space:]]*\{.*$/, "", line)
            printf "%s:%d — %s (%d 个参数)\n", file, NR, line, n
        }
    }
    ' "$f")
    if [[ -n "$result" ]]; then
        while IFS= read -r line; do
            [[ -z "$line" ]] && continue
            warn "$line"
            ((param_warn++)) || true
        done <<< "$result"
    fi
done < <(all_rs)
if (( param_warn == 0 )); then
    pass "所有 Rust 函数参数 <= ${MAX_FUNCTION_PARAMS} 个"
fi

hdr "=== C4. unwrap/expect 使用 (非测试模块 → WARN) ==="
unwrap_warn=0
while IFS= read -r f; do
    rel="${f#$PROJECT_ROOT/}"
    hits=$(awk '
    function clean(line) {
        gsub(/\/\/.*$/, "", line)
        gsub(/"([^"\\]|\\.)*"/, "", line)
        return line
    }
    function brace_delta(line,    i, c, delta) {
        line = clean(line)
        delta = 0
        for (i = 1; i <= length(line); i++) {
            c = substr(line, i, 1)
            if (c == "{") delta++
            if (c == "}") delta--
        }
        return delta
    }
    /^[[:space:]]*#\[cfg\(test\)\]/ { pending_test_attr = 1; next }
    pending_test_attr && /^[[:space:]]*mod[[:space:]]+[A-Za-z_][A-Za-z0-9_]*[[:space:]]*\{/ {
        in_test = 1
        test_depth = brace_delta($0)
        pending_test_attr = 0
        next
    }
    pending_test_attr && /^[[:space:]]*(pub(\([^)]*\))?[[:space:]]+)?(async[[:space:]]+)?fn[[:space:]]+[A-Za-z_][A-Za-z0-9_]*[[:space:]]*\(/ {
        pending_test_fn = 1
        fn_depth = brace_delta($0)
        if (fn_depth > 0) {
            in_test_fn = 1
            pending_test_fn = 0
        }
        next
    }
    pending_test_fn {
        fn_depth += brace_delta($0)
        if (fn_depth > 0) {
            in_test_fn = 1
            pending_test_fn = 0
        }
        next
    }
    in_test {
        test_depth += brace_delta($0)
        if (test_depth <= 0) {
            in_test = 0
            test_depth = 0
        }
        next
    }
    in_test_fn {
        fn_depth += brace_delta($0)
        if (fn_depth <= 0) {
            in_test_fn = 0
            fn_depth = 0
        }
        next
    }
    {
        pending_test_attr = 0
        line = $0
        gsub(/\/\/.*$/, "", line)
        if (line ~ /\.unwrap\(\)/ || line ~ /\.expect\(/) {
            printf "      %d: %s\n", NR, $0
        }
    }
    ' "$f")
    if [[ -n "$hits" ]]; then
        warn "$rel — 发现 unwrap/expect:"
        echo "$hits"
        ((unwrap_warn++)) || true
    fi
done < <(all_rs)
if (( unwrap_warn == 0 )); then
    pass "非测试 Rust 代码未发现 unwrap/expect"
fi

hdr "=== C5. Tauri command 文档注释 ==="
tauri_doc_warn=0
while IFS= read -r f; do
    rel="${f#$PROJECT_ROOT/}"
    hits=$(awk '
    /^[[:space:]]*\/\/\// { has_doc = 1; next }
    /^[[:space:]]*#\[/ && $0 !~ /#\[tauri::command\]/ { next }
    /^[[:space:]]*#\[tauri::command\]/ {
        pending_command = 1
        command_has_doc = has_doc
        has_doc = 0
        next
    }
    pending_command && /^[[:space:]]*(pub(\([^)]*\))?[[:space:]]+)?(async[[:space:]]+)?fn[[:space:]]+[A-Za-z_][A-Za-z0-9_]*/ {
        if (!command_has_doc) {
            line = $0
            sub(/^[[:space:]]+/, "", line)
            printf "      %d: %s\n", NR, line
        }
        pending_command = 0
        command_has_doc = 0
        next
    }
    /^[[:space:]]*$/ { has_doc = 0; next }
    { has_doc = 0; pending_command = 0; command_has_doc = 0 }
    ' "$f")
    if [[ -n "$hits" ]]; then
        warn "$rel — Tauri command 缺少 /// 文档注释:"
        echo "$hits"
        ((tauri_doc_warn++)) || true
    fi
done < <(all_rs)
if (( tauri_doc_warn == 0 )); then
    pass "所有 Tauri command 均有文档注释"
fi

hdr "=== C6. mod.rs 检查 (禁止使用 mod.rs) ==="
mod_rs_found=false
while IFS= read -r f; do
    rel="${f#$PROJECT_ROOT/}"
    fail "发现 mod.rs: $rel — 使用 name.rs + name/ 子目录模式"
    mod_rs_found=true
done < <(find "$RUST_SRC_DIR" -name 'mod.rs')
if ! $mod_rs_found; then
    pass "未发现 mod.rs 文件"
fi

hdr "=== C7. super::super:: 过度层级引用 ==="
super_warn=0
while IFS= read -r f; do
    rel="${f#$PROJECT_ROOT/}"
    hits=$(grep -n 'super::super::' "$f" 2>/dev/null || true)
    if [[ -n "$hits" ]]; then
        warn "$rel — 发现 super::super:: 引用:"
        echo "$hits" | sed 's/^/      /'
        ((super_warn++)) || true
    fi
done < <(all_rs)
if (( super_warn == 0 )); then
    pass "未发现 super::super:: 过度层级引用"
fi

hdr "=== C8. 导出 API 文档注释 (pub item → WARN) ==="
undoc_count=0
while IFS= read -r f; do
    rel="${f#$PROJECT_ROOT/}"
    undoc=$(awk '
    /\/\/\// { prev_doc = 1; next }
    /^[[:space:]]*#\[/ { next }
    /^[[:space:]]*pub(\([^)]*\))?[[:space:]]+((unsafe|async)[[:space:]]+)*(fn|struct|enum|trait|mod|const|static|type|use)[[:space:]]+/ {
        if (!prev_doc) {
            line = $0
            sub(/^[[:space:]]+/, "", line)
            printf "      %d: %s\n", NR, line
        }
        prev_doc = 0
        next
    }
    /^[[:space:]]*$/ { prev_doc = 0; next }
    { prev_doc = 0 }
    ' "$f")
    if [[ -n "$undoc" ]]; then
        warn "$rel — 缺少文档注释:"
        echo "$undoc"
        ((undoc_count++)) || true
    fi
done < <(all_rs)
if (( undoc_count == 0 )); then
    pass "所有公共 Rust API 均有文档注释"
fi

hdr "=== C9. unsafe 块 SAFETY 注释 ==="
unsafe_warn=0
while IFS= read -r f; do
    rel="${f#$PROJECT_ROOT/}"
    hits=$(awk '
    /unsafe[[:space:]]*\{/ && !/SAFETY/ {
        if (prev !~ /\/\/[[:space:]]*SAFETY:/ && prev !~ /\/\*[[:space:]]*SAFETY:/) {
            printf "      %d: %s\n", NR, $0
        }
    }
    { prev = $0 }
    ' "$f")
    if [[ -n "$hits" ]]; then
        warn "$rel — unsafe 块缺少 SAFETY 注释:"
        echo "$hits"
        ((unsafe_warn++)) || true
    fi
done < <(all_rs)
if (( unsafe_warn == 0 )); then
    pass "所有 unsafe 块均有 SAFETY 注释（或无 unsafe）"
fi

hdr "=== D 组：AI 占位符与假实现扫描 ==="
placeholder_fail=0
while IFS= read -r f; do
    rel="${f#$PROJECT_ROOT/}"
    hits=$(grep -n -I -E 'TODO|FIXME|TBD|待实现|未实现|临时实现|todo!\(|unimplemented!\(|panic!\("TODO' "$f" 2>/dev/null || true)
    if [[ -n "$hits" ]]; then
        fail "$rel — 发现占位符/假实现标记:"
        echo "$hits" | sed 's/^/      /'
        ((placeholder_fail++)) || true
    fi
done < <(all_source_for_placeholder_scan)
if (( placeholder_fail == 0 )); then
    pass "未发现 TODO/FIXME/TBD 等占位符或假实现标记"
fi

hdr "=== 汇总 ==="
printf "  PASS: ${C_BOLD}%d${C_RST}  WARN: ${C_BOLD}%d${C_RST}  FAIL: ${C_BOLD}%d${C_RST}\n" "$N_PASS" "$N_WARN" "$N_FAIL"

if $DO_DIFF; then
    info "增量模式：结构性扫描仅检查 diff $DIFF_REF 范围内文件"
fi

if (( N_FAIL > 0 )); then
    printf "${C_FAIL}存在 FAIL 项，请修复后重新检查。${C_RST}\n"
    exit 1
elif (( N_WARN > 0 )); then
    printf "${C_WARN}存在 WARN 项，建议优化。${C_RST}\n"
    exit 0
else
    printf "${C_PASS}全部检查通过。${C_RST}\n"
    exit 0
fi
