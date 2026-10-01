import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test } from "vitest";
import type { UpdateView } from "./api";
import { session, backend } from "../test/fixtures";
import { app } from "./store.svelte";
import { update } from "./update.svelte";
import UpdateCard from "./UpdateCard.svelte";
import UpdateDialog from "./UpdateDialog.svelte";

const offer: UpdateView = {
  current: "0.3.0",
  available: "0.4.0",
  dismissed: false,
  phase: "idle",
  received: 0,
  total: null,
  checkedAt: 1,
  error: null,
};

beforeEach(() => {
  app.sessions = [];
  update.view = null;
  update.confirming = false;
});

test("no card until a check finds a newer version, and none after Later", () => {
  const { rerender } = render(UpdateCard);
  expect(screen.queryByRole("region")).toBeNull();
  update.view = { ...offer, available: null };
  rerender({});
  expect(screen.queryByText(/is available/)).toBeNull();
  update.view = { ...offer, dismissed: true };
  rerender({});
  expect(screen.queryByText(/is available/)).toBeNull();
});

test("the card links to Settings › Updates and Later hides its version", async () => {
  const calls = backend({ dismiss_update: () => ({ ...offer, dismissed: true }) });
  update.view = offer;
  const user = userEvent.setup();
  render(UpdateCard);
  expect(screen.getByRole("link", { name: "OpenCompanion 0.4.0 is available" })).toHaveAttribute("href", "/settings?s=updates");
  expect(screen.getByText("Updating restarts the app.")).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Later" }));
  expect(calls.calls("dismiss_update")).toHaveLength(1);
});

test("Update installs straight away when no session would stop", async () => {
  const calls = backend({ install_update: () => undefined });
  update.view = offer;
  app.sessions = [session({ status: "done" })];
  const user = userEvent.setup();
  render(UpdateCard);
  await user.click(screen.getByRole("button", { name: "Update" }));
  expect(calls.calls("install_update")).toHaveLength(1);
  expect(update.confirming).toBe(false);
});

test("Update asks first when running sessions would stop, and Keep working changes nothing", async () => {
  const calls = backend({ install_update: () => undefined });
  update.view = offer;
  app.sessions = [session({ id: "a", status: "running" }), session({ id: "b", status: "shell" })];
  const user = userEvent.setup();
  render(UpdateCard);
  render(UpdateDialog);
  await user.click(screen.getByRole("button", { name: "Update" }));
  expect(calls.calls("install_update")).toHaveLength(0);
  expect(await screen.findByRole("heading", { name: "Restart OpenCompanion to update?" })).toBeInTheDocument();
  expect(screen.getByText(/stops the 2 sessions that are running now/)).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Keep working" }));
  expect(update.confirming).toBe(false);
  expect(calls.calls("install_update")).toHaveLength(0);

  await user.click(screen.getByRole("button", { name: "Update" }));
  await user.click(await screen.findByRole("button", { name: "Stop sessions and update" }));
  expect(calls.calls("install_update")).toHaveLength(1);
});

test("the card follows the download and shows why an install failed", () => {
  update.view = { ...offer, dismissed: true, phase: "downloading", received: 3 * 1048576, total: 12 * 1048576 };
  const { rerender } = render(UpdateCard);
  expect(screen.getByRole("link", { name: "Downloading 0.4.0" })).toBeInTheDocument();
  expect(screen.getByText("25%")).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Update" })).toBeNull();

  update.view = { ...offer, dismissed: true, phase: "installing" };
  rerender({});
  expect(screen.getByRole("status")).toHaveTextContent("OpenCompanion restarts when it is done.");

  update.view = { ...offer, dismissed: true, error: { step: "install", message: "pkexec was dismissed" } };
  rerender({});
  expect(screen.getByRole("link", { name: "The update did not install" })).toBeInTheDocument();
  expect(screen.getByRole("alert")).toHaveTextContent("pkexec was dismissed");
  expect(screen.getByRole("button", { name: "Try again" })).toBeInTheDocument();
});
