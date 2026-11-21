import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { LocaleKey } from "../../../../../i18n/keys";

const tauriMocks = vi.hoisted(() => ({
  cursorCancelLogin: vi.fn(),
  cursorLoginWithBrowser: vi.fn(),
  getManualCookies: vi.fn(),
  removeManualCookie: vi.fn(),
}));

vi.mock("../../../../../lib/tauri", () => tauriMocks);

import { CursorSignInCreds } from "./CursorSignInCreds";

const t = (key: LocaleKey) => key as string;

describe("CursorSignInCreds", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    tauriMocks.getManualCookies.mockResolvedValue([]);
    tauriMocks.cursorLoginWithBrowser.mockResolvedValue(undefined);
    tauriMocks.removeManualCookie.mockResolvedValue([]);
  });

  it("shows Sign in with Cursor when not signed in and calls cursorLoginWithBrowser", async () => {
    const onChanged = vi.fn();
    render(<CursorSignInCreds t={t} onChanged={onChanged} />);

    const button = await screen.findByRole("button", { name: "CursorSignInButton" });
    fireEvent.click(button);

    await waitFor(() =>
      expect(tauriMocks.cursorLoginWithBrowser).toHaveBeenCalledTimes(1),
    );
    await waitFor(() =>
      expect(screen.getByText("CursorSignInSuccess")).toBeInTheDocument(),
    );
    expect(onChanged).toHaveBeenCalledTimes(1);
  });

  it("shows Switch Account and Sign out when a Cursor session is saved", async () => {
    tauriMocks.getManualCookies.mockResolvedValue([
      { providerId: "cursor", provider: "Cursor", savedAt: "2026-11-21" },
    ]);
    const onChanged = vi.fn();
    render(<CursorSignInCreds t={t} onChanged={onChanged} />);

    expect(
      await screen.findByRole("button", { name: "CursorSwitchAccountButton" }),
    ).toBeInTheDocument();
    const signOutBtn = screen.getByRole("button", { name: "CursorSignOutButton" });
    fireEvent.click(signOutBtn);

    await waitFor(() =>
      expect(tauriMocks.removeManualCookie).toHaveBeenCalledWith("cursor"),
    );
    expect(onChanged).toHaveBeenCalledTimes(1);
  });
});
