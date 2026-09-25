import { render, screen } from "@testing-library/svelte";
import userEvent from "@testing-library/user-event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { beforeEach, expect, test, vi } from "vitest";
import TitleBar from "./TitleBar.svelte";

const win = vi.mocked(getCurrentWindow());

beforeEach(() => {
  win.minimize.mockClear();
  win.toggleMaximize.mockClear();
  win.close.mockClear();
  win.onResized.mockClear();
  win.isMaximized.mockResolvedValue(false);
});

test("the window buttons minimize, maximize and close the window", async () => {
  const user = userEvent.setup();
  render(TitleBar);
  await user.click(screen.getByRole("button", { name: "Minimize" }));
  expect(win.minimize).toHaveBeenCalledOnce();
  await user.click(screen.getByRole("button", { name: "Maximize" }));
  expect(win.toggleMaximize).toHaveBeenCalledOnce();
  await user.click(screen.getByRole("button", { name: "Close" }));
  expect(win.close).toHaveBeenCalledOnce();
});

test("Maximize turns into Restore while the window is maximized", async () => {
  win.isMaximized.mockResolvedValue(true);
  render(TitleBar);
  expect(await screen.findByRole("button", { name: "Restore" })).toHaveAttribute("title", "Restore");

  win.isMaximized.mockResolvedValue(false);
  const onResize = win.onResized.mock.calls[0][0];
  onResize({} as never);
  expect(await screen.findByRole("button", { name: "Maximize" })).toBeInTheDocument();
});

test("the bar is the drag handle and the buttons are not", () => {
  const { container } = render(TitleBar);
  expect(container.querySelector("#titlebar")).toHaveAttribute("data-tauri-drag-region");
  for (const button of screen.getAllByRole("button")) expect(button).not.toHaveAttribute("data-tauri-drag-region");
});
