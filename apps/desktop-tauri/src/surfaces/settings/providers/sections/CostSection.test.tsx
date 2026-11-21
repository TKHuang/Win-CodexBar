import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import type { CostSnapshotBridge } from "../../../../types/bridge";
import type { LocaleKey } from "../../../../i18n/keys";
import { CostSection } from "./CostSection";

describe("CostSection", () => {
  it("does not present the zero usage carrier as spend for balance-only providers", () => {
    const cost: CostSnapshotBridge = {
      used: 0,
      limit: null,
      remaining: null,
      currencyCode: "USD",
      period: "Atlas Cloud balance",
      resetsAt: null,
      formattedUsed: "$0.00",
      formattedLimit: null,
      balance: 95.5,
      formattedBalance: "$95.50",
    };

    const { container } = render(
      <CostSection cost={cost} t={(key: LocaleKey) => key} />,
    );

    expect(container.firstChild).toBeNull();
    expect(screen.queryByText("DetailCostUsed")).toBeNull();
    expect(screen.queryByText("$0.00")).toBeNull();
  });

  it("renders balance alongside used and limit for capped enterprise pools", () => {
    const cost: CostSnapshotBridge = {
      used: 42.5,
      limit: 200,
      remaining: 157.5,
      currencyCode: "USD",
      period: "Enterprise pool (billing cycle)",
      resetsAt: "2026-12-01T00:00:00Z",
      formattedUsed: "$42.50",
      formattedLimit: "$200.00",
      balance: 157.5,
      formattedBalance: "$157.50",
    };

    render(<CostSection cost={cost} t={(key: LocaleKey) => key} />);

    expect(screen.getByText("DetailCostUsed")).toBeTruthy();
    expect(screen.getByText("$42.50")).toBeTruthy();
    expect(screen.getByText("DetailCostLimit")).toBeTruthy();
    expect(screen.getByText("$200.00")).toBeTruthy();
    expect(screen.getByText("DetailCostBalance")).toBeTruthy();
    expect(screen.getByText("$157.50")).toBeTruthy();
  });
});
