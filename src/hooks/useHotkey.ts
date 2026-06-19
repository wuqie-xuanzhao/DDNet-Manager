import { useEffect } from "react";

/// 全局快捷键绑定。`key` 用 KeyboardEvent.key（区分大小写，比较时转 lower-case），
/// 组合键 modifier 显式声明。`ctrl` 在 macOS 上同时匹配 Ctrl 和 Cmd（业界惯例：
/// 用户期望 Cmd+, 和 Ctrl+, 行为一致），其他平台只匹配 Ctrl。
export interface HotkeyBinding {
  /// KeyboardEvent.key（如 "Escape", "s", "F5"）。比较时大小写不敏感。
  key: string;
  /// 是否要求 Ctrl（macOS 上 Cmd 也算）。
  ctrl?: boolean;
  /// 是否要求 Shift。
  shift?: boolean;
  /// 是否要求 Alt。
  alt?: boolean;
  /// 触发后的回调。
  handler: () => void;
}

/// 判断当前事件 target 是否是可编辑元素（input/textarea/contenteditable）。
/// 在可编辑元素里，纯字符键不触发快捷键（避免输入 "s" 时触发 Ctrl+S 之外的 binding）；
/// 但显式带 ctrl/alt 的组合键仍然触发（让 Ctrl+S 等保存在编辑框内可用）。
function isEditableTarget(event: KeyboardEvent): boolean {
  const target = event.target as HTMLElement | null;
  if (!target) return false;
  const tag = target.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || target.isContentEditable;
}

/// 单个 binding 是否匹配当前事件。匹配规则：
/// - key 大小写不敏感相等
/// - ctrl 修饰符：要求 ctrl=true 时，event.ctrlKey 或 event.metaKey（macOS Cmd）任一为 true
/// - shift/alt 修饰符：要求显式声明时 event 对应为 true，未声明时为 false
/// - 可编辑元素内：纯字符键（无 ctrl/alt）跳过
function matchesBinding(event: KeyboardEvent, binding: HotkeyBinding): boolean {
  const inEditable = isEditableTarget(event);
  if (inEditable && !binding.ctrl && !binding.alt) {
    return false;
  }
  if (event.key.toLowerCase() !== binding.key.toLowerCase()) {
    return false;
  }
  if (binding.ctrl && !(event.ctrlKey || event.metaKey)) return false;
  if (!binding.ctrl && (event.ctrlKey || event.metaKey)) return false;
  if (!!binding.shift !== event.shiftKey) return false;
  if (!!binding.alt !== event.altKey) return false;
  return true;
}

/// 注册全局快捷键。bindings 数组在每次 render 重建时 effect 会重新绑定——
/// 调用方应保证 handler 引用稳定（useCallback）避免频繁 unbind/rebind。
///
/// `enabled=false` 时跳过绑定（条件快捷键，如弹窗打开时禁用）。
///
/// 不接管 stopPropagation：调用方应在 handler 内决定是否阻断后续 listener。
///
/// 与 useEscToClose 的关系：useEscToClose 是单按钮 Esc 关闭的专用 hook；
/// useHotkey 是通用快捷键注册器，支持任意键 + 组合键。
export function useHotkey(bindings: HotkeyBinding[], enabled: boolean = true): void {
  useEffect(() => {
    if (!enabled) return;
    const handler = (event: KeyboardEvent) => {
      for (const binding of bindings) {
        if (matchesBinding(event, binding)) {
          event.preventDefault();
          binding.handler();
          return;
        }
      }
    };
    document.addEventListener("keydown", handler);
    return () => {
      document.removeEventListener("keydown", handler);
    };
  }, [bindings, enabled]);
}
