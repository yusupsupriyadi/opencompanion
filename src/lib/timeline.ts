import type { EventRow } from "./api";
import { clock } from "./format";
import { t } from "./i18n.svelte";

/** One line of a session's timeline, with its terminal color (`.t-*` in app.css). */
export type TimelineLine = { cls: string; text: string };

/** How one event reads in the timeline, on the desktop Session screen and on the phone. */
export function timelineLines(row: EventRow): TimelineLine[] {
  const e = row.event;
  switch (e.kind) {
    case "started":
      return [{ cls: "t-d", text: `${clock(row.at)}  ${t("sessions.timeline.started")}` }];
    case "message":
      // The backend writes these in English; the prefix picks the color, not the words shown.
      if (e.text.startsWith("You: ")) return [{ cls: "t-g", text: `› ${e.text.slice(5)}` }];
      if (/^(Approved|Denied|Stopped) /.test(e.text)) return [{ cls: e.text.startsWith("Approved") ? "t-g" : "t-r", text: `  ${e.text}` }];
      return [{ cls: "", text: e.text }];
    case "tool_call":
      return [{ cls: "t-d", text: `• ${e.tool} ${e.summary}`.trimEnd() }];
    case "file_changed":
      return [{ cls: "t-d", text: `  ${t("sessions.timeline.edited", { path: e.path })}` }];
    case "permission_request":
      return [{ cls: "t-y", text: `? ${t("sessions.timeline.asks", { tool: e.tool, summary: e.summary })}` }];
    case "permission_denied":
      return [{ cls: "t-r", text: `✗ ${t("sessions.timeline.refused", { tool: e.tool, summary: e.summary })}` }];
    case "tool_failed":
      return [{ cls: "t-r", text: `✗ ${t("sessions.timeline.toolFailed", { tool: e.tool || t("sessions.timeline.tool"), message: e.message })}` }];
    case "retrying":
      return [{ cls: "t-y", text: e.message }];
    case "error":
      return [{ cls: "t-r", text: e.message }];
    case "done":
      return [
        {
          cls: e.ok ? "t-g" : "t-r",
          text: e.ok ? `✓ ${t("sessions.timeline.finished", { time: clock(row.at) })}` : `✗ ${t("sessions.timeline.failed", { summary: e.summary })}`,
        },
      ];
    default:
      return [];
  }
}
