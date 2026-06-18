import * as React from "react"
import { Switch as SwitchPrimitive } from "radix-ui"

import { cn } from "@/lib/utils"

/// 项目自有的米哈游风格 Switch：w-11 h-6 圆角胶囊，amber 边框（选中），
/// 白底圆点带勾选图标（选中）/ 中灰圆点（未选中）。
/// 底层用 radix SwitchPrimitive，视觉与原 Toggle 完全一致。
function Switch({
  className,
  ...props
}: React.ComponentProps<typeof SwitchPrimitive.Root>) {
  return (
    <SwitchPrimitive.Root
      data-slot="switch"
      className={cn(
        "relative inline-flex shrink-0 items-center w-11 h-6 rounded-full transition-all duration-200 px-[3px] cursor-pointer border-[2.5px] bg-[var(--app-surface)] outline-none focus-visible:ring-2 focus-visible:ring-[var(--app-focus)] data-[state=checked]:border-[var(--app-accent)] data-[state=unchecked]:border-[var(--app-text-dim)] disabled:cursor-not-allowed disabled:opacity-50",
        className
      )}
      {...props}
    >
      <SwitchPrimitive.Thumb
        data-slot="switch-thumb"
        className="pointer-events-none block w-3.5 h-3.5 rounded-full transition-all duration-200 flex items-center justify-center shadow-sm data-[state=checked]:translate-x-5 data-[state=checked]:bg-white data-[state=checked]:text-[#111215] data-[state=unchecked]:translate-x-0 data-[state=unchecked]:bg-[var(--app-text-muted)] data-[state=unchecked]:text-transparent"
      >
        {props.checked ? (
          <svg
            viewBox="0 0 24 24"
            className="w-2.5 h-2.5 stroke-[4.5]"
            fill="none"
            stroke="currentColor"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <polyline points="20 6 9 17 4 12" />
          </svg>
        ) : null}
      </SwitchPrimitive.Thumb>
    </SwitchPrimitive.Root>
  )
}

export { Switch }
