import { render, screen } from "@testing-library/svelte";
import { expect, test } from "vitest";
import type { EventRow } from "./api";
import Timeline from "./Timeline.svelte";

const row = (id: number, event: EventRow["event"]): EventRow => ({ id, sessionId: "s", at: Date.now(), event });

test("headless events read as a terminal log", () => {
  // `events` is also a Svelte mount option, so props go under `props`.
  render(Timeline, {
    props: {
      events: [
      row(1, { kind: "message", text: "You: add a hello file" }),
      row(2, { kind: "tool_call", tool: "Write", summary: "hello.txt" }),
      row(3, { kind: "permission_request", request_id: "r", tool: "Write", summary: "hello.txt" }),
      row(4, { kind: "message", text: "Approved Write in OpenCompanion" }),
      row(5, { kind: "file_changed", path: "hello.txt" }),
      row(6, { kind: "done", ok: true, summary: "done" }),
      ],
    },
  });
  expect(screen.getByText("› add a hello file")).toHaveClass("t-g");
  expect(screen.getByText("• Write hello.txt")).toBeInTheDocument();
  expect(screen.getByText("? Write asks for permission: hello.txt")).toHaveClass("t-y");
  expect(screen.getByText("edited hello.txt", { exact: false })).toBeInTheDocument();
  expect(screen.getByText(/✓ Finished at/)).toHaveClass("t-g");
});

test("an empty timeline says it is waiting", () => {
  render(Timeline, { props: { events: [] } });
  expect(screen.getByText("Waiting for the first event…")).toBeInTheDocument();
});
