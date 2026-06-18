"use client"

import * as React from "react"
import { RadioGroup as RadioGroupPrimitive } from "radix-ui"

import { cn } from "@/lib/utils"

function RadioGroup({
  className,
  ...props
}: React.ComponentProps<typeof RadioGroupPrimitive.Root>) {
  return (
    <RadioGroupPrimitive.Root
      data-slot="radio-group"
      className={cn("grid w-full gap-2", className)}
      {...props}
    />
  )
}

/// 项目自有的米哈游风格 RadioGroupItem：w-4 h-4 圆形外框，
/// 选中时内部 2.5px 实心 amber 圆点（含外圈 amber 高亮边）。
/// 底层用 radix RadioGroupPrimitive.Item，视觉与原 radio 完全一致。
function RadioGroupItem({
  className,
  ...props
}: React.ComponentProps<typeof RadioGroupPrimitive.Item>) {
  return (
    <RadioGroupPrimitive.Item
      data-slot="radio-group-item"
      className={cn(
        "relative flex shrink-0 w-4 h-4 rounded-full border items-center justify-center cursor-pointer outline-none transition-colors focus-visible:ring-2 focus-visible:ring-[var(--app-focus)] data-[state=unchecked]:border-[var(--app-text-muted)] data-[state=checked]:border-[var(--app-accent)] disabled:cursor-not-allowed disabled:opacity-50",
        className
      )}
      {...props}
    >
      <RadioGroupPrimitive.Indicator
        data-slot="radio-group-indicator"
        className="flex items-center justify-center w-full h-full relative"
      >
        {/* 选中态：外圈 amber 描边（撑满整个 radio）+ 中心 amber 实心圆点 */}
        <div className="absolute inset-0 rounded-full border border-[var(--app-accent)]" />
        <div className="w-2.5 h-2.5 rounded-full bg-[var(--app-accent)]" />
      </RadioGroupPrimitive.Indicator>
    </RadioGroupPrimitive.Item>
  )
}

export { RadioGroup, RadioGroupItem }
