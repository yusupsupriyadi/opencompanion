import { render, screen, within } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, test, vi } from "vitest";
import { app } from "$lib/store.svelte";
import { setUrl } from "../../../test/app-state.svelte";
import { CLIS } from "../../../test/fixtures";
import ClisPage from "./+page.svelte";

const WINDOWS_UA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0 Safari/537.36 Edg/140.0";
const LINUX_UA = "Mozilla/5.0 (X11; Ubuntu; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15";

afterEach(() => {
  vi.restoreAllMocks();
});

test("CLIs is a Settings screen, named by its H1", () => {
  setUrl("/settings/clis");
  app.clis = CLIS;
  app.clisState = "ready";
  render(ClisPage);
  expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("CLIs");
  expect(screen.getByRole("button", { name: "Rescan" })).toBeInTheDocument();
  expect(screen.getByRole("table", { name: "Coding CLIs found on this computer" })).toBeInTheDocument();
});

test("CCS shows its install command, and its settings say the profile comes first", async () => {
  const user = userEvent.setup();
  setUrl("/settings/clis");
  app.clis = CLIS;
  app.clisState = "ready";
  render(ClisPage);
  expect(screen.getByText("npm install -g @kaitranntt/ccs")).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Set a path for CCS" }));
  expect(screen.getByLabelText("Extra arguments")).toHaveAttribute("placeholder", "For example work --effort high");
  expect(screen.getByText(/Placed right after ccs, so the profile comes first/)).toBeInTheDocument();
});

test("Pi shows its install command, and once found, its JSON mode and the planner button", () => {
  setUrl("/settings/clis");
  app.clis = CLIS;
  app.clisState = "ready";
  const { unmount } = render(ClisPage);
  expect(screen.getByText("npm install -g --ignore-scripts @earendil-works/pi-coding-agent")).toBeInTheDocument();
  unmount();
  app.clis = CLIS.map((c) => (c.kind === "pi" ? { ...c, path: String.raw`C:\Users\me\.pi\agent\bin\pi.cmd`, version: "0.87.1", tested: true } : c));
  render(ClisPage);
  const row = screen.getByText("Pi", { selector: "b" }).closest("tr")!;
  expect(row).toHaveTextContent("pi --mode json");
  expect(row).toHaveTextContent("0.87.1");
  expect(within(row).getByRole("button", { name: "Use as planner" })).toBeInTheDocument();
});

test("omp installs with Bun, and once found, shows its JSON mode and the planner button", () => {
  setUrl("/settings/clis");
  app.clis = CLIS;
  app.clisState = "ready";
  const { unmount } = render(ClisPage);
  expect(screen.getByText("bun install -g @oh-my-pi/pi-coding-agent")).toBeInTheDocument();
  unmount();
  app.clis = CLIS.map((c) => (c.kind === "omp" ? { ...c, path: String.raw`C:\Users\me\.bun\bin\omp.exe`, version: "18.3.5", tested: true } : c));
  render(ClisPage);
  const row = screen.getByText("omp", { selector: "b" }).closest("tr")!;
  expect(row).toHaveTextContent("omp --mode json");
  expect(row).toHaveTextContent("18.3.5");
  expect(within(row).getByRole("button", { name: "Use as planner" })).toBeInTheDocument();
});

test("Cursor CLI shows the installer for this OS, and once found, its stream-json mode and the planner button", () => {
  setUrl("/settings/clis");
  app.clis = CLIS;
  app.clisState = "ready";
  vi.spyOn(navigator, "userAgent", "get").mockReturnValue(WINDOWS_UA);
  const windows = render(ClisPage);
  expect(screen.getByText("irm 'https://cursor.com/install?win32=true' | iex")).toBeInTheDocument();
  windows.unmount();
  vi.spyOn(navigator, "userAgent", "get").mockReturnValue(LINUX_UA);
  const linux = render(ClisPage);
  expect(screen.getByText("curl https://cursor.com/install -fsS | bash")).toBeInTheDocument();
  linux.unmount();
  const path = String.raw`C:\Users\me\AppData\Local\cursor-agent\cursor-agent.cmd`;
  app.clis = CLIS.map((c) => (c.kind === "cursor" ? { ...c, path, version: "2026.09.28", tested: true } : c));
  render(ClisPage);
  const row = screen.getByText("Cursor CLI", { selector: "b" }).closest("tr")!;
  expect(row).toHaveTextContent("cursor-agent -p --output-format stream-json");
  expect(row).toHaveTextContent("2026.09.28");
  expect(within(row).getByRole("button", { name: "Use as planner" })).toBeInTheDocument();
});
