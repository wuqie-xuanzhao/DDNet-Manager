export function ResourcePanel() {
  return (
    <section className="rounded-[34px] border border-[var(--dm-border)] bg-white/82 p-6 text-[var(--dm-ink)] shadow-[0_24px_70px_rgba(47,52,64,0.10)] backdrop-blur-2xl">
      <h2 className="text-3xl font-black tracking-[-0.05em]">资源位置</h2>
      <p className="mt-2 text-sm font-semibold text-[#4f5663]">当前版本不可用，后续版本添加。</p>

      <div className="mt-5 rounded-[26px] bg-[var(--dm-soft)] p-5">
        <div className="rounded-[22px] border border-dashed border-[var(--dm-border)] bg-white/70 px-5 py-8">
          <div className="text-[11px] font-black tracking-[0.18em] text-[#59606d]">MVP 状态</div>
          <div className="mt-3 text-lg font-black text-[var(--dm-ink)]">资源位置功能暂不开放</div>
          <p className="mt-3 max-w-2xl text-sm font-semibold leading-7 text-[#4f5663]">
            当前版本不可用，后续版本添加。
          </p>
        </div>
      </div>
    </section>
  );
}
