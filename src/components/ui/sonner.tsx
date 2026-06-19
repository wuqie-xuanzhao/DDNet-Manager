import { Toaster as Sonner } from "sonner";

/// 项目启动器全局 Toaster。封装 sonner 默认外观，让 toast 视觉对齐工业霓虹
/// 设计系统（用 --app-* CSS 变量，自动跟随 dark/light 主题切换）。
///
/// 用法：在 main.tsx 挂载一次 `<Toaster />`，组件里直接 `import { toast } from "sonner"`
/// 调 `toast.success("xxx")` / `toast.error("xxx")` / `toast.message("xxx")`。
///
/// 不抽 useToast()——sonner 的 toast 函数已经足够好用，多套一层抽象反而损失类型推导。
type ToasterProps = React.ComponentProps<typeof Sonner>;

const Toaster = ({ ...props }: ToasterProps) => {
  return (
    <Sonner
      position="bottom-center"
      closeButton={false}
      richColors={false}
      className="toaster group"
      toastOptions={{
        classNames: {
          toast:
            "group toast group-[.toaster]:bg-[var(--app-surface)] group-[.toaster]:text-[var(--app-text)] group-[.toaster]:border-[var(--app-border)] group-[.toaster]:shadow-[0_12px_40px_rgba(0,0,0,0.6)] group-[.toaster]:rounded-lg",
          description: "group-[.toast]:text-[var(--app-text-muted)]",
          actionButton:
            "group-[.toast]:bg-[var(--app-accent)] group-[.toast]:text-[var(--app-accent-foreground)] group-[.toast]:rounded-md",
          cancelButton:
            "group-[.toast]:bg-[var(--app-input)] group-[.toast]:text-[var(--app-text-muted)] group-[.toast]:rounded-md",
        },
      }}
      {...props}
    />
  );
};

export { Toaster };
