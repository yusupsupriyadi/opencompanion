# Privacy and data

- OpenCompanion makes network connections of its own in two cases only: the phone companion, on your local network and only while phone access is on, and a custom Chat planner endpoint, if you set one.
- The CLIs you run talk to their own providers with your own login and quota, as in your terminal. A Chat planner that runs a CLI headless uses that CLI's quota.
- The Chat planner receives your message with its context: recent Chat turns, the installed CLIs, your project folder paths and the current sessions. A CLI planner passes that to the CLI's provider; a custom endpoint receives it directly.
- Data lives in the app data folder. Settings shows the exact path.
  - Windows: `%APPDATA%\dev.opencompanion.app`
  - macOS: `~/Library/Application Support/dev.opencompanion.app`
  - Linux: `~/.local/share/dev.opencompanion.app`
- That folder holds `opencompanion.db` (SQLite: sessions, events, Chat, settings and paired devices), terminal logs under `sessions/`, the hook files for Claude Code sessions under `hooks/`, and the planner's working folder `planner/`.
- Images you paste into a terminal are saved in the system temp folder, under `opencompanion-paste` (`%TEMP%\opencompanion-paste` on Windows). OpenCompanion does not delete them; they stay until you or the system clear the temp folder.
- The Files, Changes and Branch tabs of a session read its folder and run your own `git` there (status, diff, branch list and log, with `GIT_OPTIONAL_LOCKS=0` so they never lock the index). Only Switch branch changes anything, and file contents never reach the phone.
- Shell tabs are not saved: their output lives in memory while the tab is open and is gone when you close it, delete its session or quit. Retention skips a session while a shell is open in it.
- Paired phones are stored as SHA-256 hashes of their device tokens, not the tokens themselves.
- The API key of a custom Chat planner endpoint is saved with the settings in `opencompanion.db`, unencrypted.
- OpenCompanion reads the CLIs' own files and never writes to them: session history for transcripts (OpenCode's database is opened read-only) and skill folders. Claude Code hooks are passed per session with `--settings`, so your own Claude Code settings stay as they are.
- Settings › History keeps finished sessions Forever by default, or for 90, 30, 7 or 1 day, after which they are deleted with their events, logs and hook files. Delete finished sessions clears them at once.
