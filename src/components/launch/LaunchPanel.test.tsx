import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { LaunchPanel } from "./LaunchPanel";
import type { LaunchReadiness, LauncherState } from "@/types";

vi.mock("@/components/icons/GameIcon", () => ({
  GameIcon: () => <span data-testid="game-icon" />
}));

const readiness: LaunchReadiness = {
  client: null,
  can_launch: false,
  running: true,
  status_label: "正在运行",
  user_message: "QmClient 正在运行。",
  blocking_reasons: [],
  checked_at: "2026-06-08T00:00:00.000Z"
};

function renderLaunchPanel(state: LauncherState, onPrimaryAction = vi.fn()) {
  render(
    <LaunchPanel
      tauriRuntime
      state={state}
      clientPath="D:/Games/QmClient"
      readiness={readiness}
      errorMessage={null}
      mobileUpdateStatus={null}
      onBrowse={vi.fn()}
      onValidate={vi.fn()}
      onPrimaryAction={onPrimaryAction}
    />
  );

  return { onPrimaryAction };
}

describe("LaunchPanel", () => {
  it("disables the primary action when the client is already running", () => {
    const { onPrimaryAction } = renderLaunchPanel("running");
    const launchButton = screen.getByRole("button", { name: "运行中" });

    expect(launchButton).toBeDisabled();
    fireEvent.click(launchButton);

    expect(onPrimaryAction).not.toHaveBeenCalled();
  });
});
