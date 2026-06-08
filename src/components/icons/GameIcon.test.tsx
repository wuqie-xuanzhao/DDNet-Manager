import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { GameIcon } from "./GameIcon";

describe("GameIcon", () => {
  it("renders the Game-Icon-Pack svg shape instead of a square mask box", () => {
    const { container } = render(<GameIcon name="play" className="text-white" />);

    const icon = container.querySelector("[data-game-icon='play']");
    const svg = icon?.querySelector("svg");

    expect(icon).toBeInTheDocument();
    expect(svg).toBeInTheDocument();
    expect(icon?.getAttribute("style") ?? "").not.toContain("mask");
    expect(svg?.querySelector("path")).toHaveAttribute("fill", "currentColor");
  });
});
