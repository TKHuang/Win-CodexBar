import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { LocaleKey } from "../../../../../i18n/keys";

const tauriMocks = vi.hoisted(() => ({
  antigravityOAuthCancelLogin: vi.fn(),
  antigravityOAuthLogin: vi.fn(),
  antigravityOAuthSignOut: vi.fn(),
  getAntigravityOAuthStatus: vi.fn(),
  openPath: vi.fn(),
}));

vi.mock("../../../../../lib/tauri", () => tauriMocks);

import { AntigravityOAuthCreds } from "./AntigravityOAuthCreds";

const t = (key: LocaleKey) => key as string;

describe("AntigravityOAuthCreds", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    tauriMocks.antigravityOAuthLogin.mockResolvedValue({
      email: "user@example.com",
      credentialsPath: "C:\\Users\\test\\.codexbar\\antigravity\\oauth_creds.json",
    });
    tauriMocks.antigravityOAuthSignOut.mockResolvedValue(undefined);
  });

  it("shows Client ID / Secret inputs when no OAuth client is discovered", async () => {
    tauriMocks.getAntigravityOAuthStatus.mockResolvedValue({
      signedIn: false,
      email: null,
      credentialsPath: "C:\\Users\\test\\.codexbar\\antigravity\\oauth_creds.json",
      clientSource: null,
      hasClientConfig: false,
    });
    const onChanged = vi.fn();
    render(<AntigravityOAuthCreds t={t} onChanged={onChanged} />);

    const signInBtn = await screen.findByRole("button", {
      name: "AntigravitySignInButton",
    });
    expect(signInBtn).toBeDisabled();

    fireEvent.change(screen.getByLabelText("AntigravityClientIdLabel"), {
      target: { value: "my-client-id" },
    });
    fireEvent.change(screen.getByLabelText("AntigravityClientSecretLabel"), {
      target: { value: "my-client-secret" },
    });
    expect(signInBtn).toBeEnabled();

    fireEvent.click(signInBtn);
    await waitFor(() =>
      expect(tauriMocks.antigravityOAuthLogin).toHaveBeenCalledWith(
        "my-client-id",
        "my-client-secret",
      ),
    );
    expect(onChanged).toHaveBeenCalledTimes(1);
  });

  it("lets a signed-in user sign out when client config is present", async () => {
    tauriMocks.getAntigravityOAuthStatus.mockResolvedValue({
      signedIn: true,
      email: "dev@example.com",
      credentialsPath: "C:\\Users\\test\\.codexbar\\antigravity\\oauth_creds.json",
      clientSource: "credentials",
      hasClientConfig: true,
    });
    const onChanged = vi.fn();
    render(<AntigravityOAuthCreds t={t} onChanged={onChanged} />);

    expect(
      await screen.findByText("CredsStatusAuthenticated (dev@example.com)"),
    ).toBeInTheDocument();
    const signOutBtn = screen.getByRole("button", {
      name: "AntigravitySignOutButton",
    });
    fireEvent.click(signOutBtn);

    await waitFor(() =>
      expect(tauriMocks.antigravityOAuthSignOut).toHaveBeenCalledTimes(1),
    );
    expect(onChanged).toHaveBeenCalledTimes(1);
  });
});
