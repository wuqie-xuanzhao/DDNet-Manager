import * as React from "react";
import { Slot } from "radix-ui";
import type { VariantProps } from "class-variance-authority";
import { buttonVariants } from "./button-variants";
import { cn } from "@/lib/utils";

function Button({
  className,
  variant,
  size,
  type = "button",
  asChild = false,
  ...props
}: React.ComponentProps<"button"> & VariantProps<typeof buttonVariants> & {
  asChild?: boolean;
}) {
  const Comp = asChild ? Slot.Slot : "button";
  return <Comp type={type} className={cn(buttonVariants({ variant, size, className }))} {...props} />;
}

export { Button };
