# Hasil M0 Spike

| | |
|---|---|
| Tanggal | 2026-09-25 |
| Mesin | Windows 11 Home 10.0.26200, Rust 1.96 (MSVC), WebView2 153 |
| Versi CLI | Claude Code 2.1.282, Codex CLI 0.153.4, OpenCode 1.18.30, Gemini CLI tidak terpasang |
| Alat uji | `src-tauri/src/bin/air-spike.rs` (subperintah `detect`, `scan`, `pty`, `headless`, `replay`, `waiting`) |
| Folder uji | folder scratch kosong di luar repo, satu per CLI |

Kriteria selesai M0 di PRD: tiga CLI bisa dijalankan interaktif dan headless dari prototipe Rust, dan hasil uji deteksi izin tercatat per CLI. Status per kriteria ada di tabel ringkasan; bagian yang belum lolos disebut terang di bawahnya.

## Ringkasan

| | Claude Code | Codex CLI | OpenCode |
|---|---|---|---|
| Deteksi + versi | lolos | lolos | lolos (shim npm diarahkan ke exe native) |
| PTY interaktif (ConPTY) | lolos | lolos | lolos dengan `--pure`; tanpa itu crash karena plugin di konfigurasi pengguna |
| Headless + parse event | lolos | hanya jalur gagal: 401 dari provider `inferhub` | lolos dengan `--pure` |
| Sinyal "menunggu izin" headless | lolos: `control_request` / `can_use_tool`, dijawab host | belum teruji (auth) | tidak ada: `run` menolak sendiri, jalurnya lewat `opencode serve` |
| Sinyal "menunggu izin" PTY | lolos: hook `PermissionRequest` dan `Notification`, plus pola teks | pola teks untuk tawaran update saja | belum teruji |
| Deteksi proses eksternal | lolos, dengan folder kerja | lolos, dengan folder kerja | lolos, dengan folder kerja; exe pembungkus dan exe platform dihitung satu sesi |

## Temuan per area

### Deteksi CLI (FR-01, FR-02)

- `which` + folder instal umum (`~/.local/bin`, `%APPDATA%/npm`, `~/.bun/bin`, `~/.opencode/bin`) menemukan ketiga CLI dalam 731 ms, paralel.
- Shim npm `.cmd` yang hanya meneruskan ke exe native (OpenCode) diganti dengan exe itu. Prompt tidak lewat `cmd.exe`, jadi aturan quoting batch tidak berlaku.
- Codex di mesin ini dipasang lewat installer native (`%LOCALAPPDATA%/Programs/OpenAI/Codex/bin/codex.exe`), bukan npm.

### PTY (FR-10)

- **ConPTY menahan semua output sampai pertanyaan posisi kursor (`ESC[6n`) dijawab.** Tanpa jawaban, setiap CLI hanya mengeluarkan 4 byte lalu diam. Rust core kini menjawabnya dari posisi kursor emulator `vt100`, sehingga sesi tanpa tampilan terminal tetap berjalan. Keputusan untuk M1: xterm.js di webview tidak boleh meneruskan jawaban CPR miliknya sendiri ke PTY.
- Output pertama sekitar 270 ms setelah spawn. Resize dan Ctrl+C berfungsi: Claude Code keluar dengan kode 1, Codex dan OpenCode dengan kode 0.
- Layar pertama tiap CLI bisa berupa dialog yang menunggu pengguna:
  - Claude Code: dialog kepercayaan folder baru. Pilihan default adalah "No, exit".
  - Codex: tawaran update. Pilihan default adalah "Update now", yang menjalankan skrip instal. **OpenCompanion tidak boleh mengirim Enter ke layar ini secara otomatis.**
- OpenCode tanpa `--pure` keluar dalam 4 detik dengan "Unexpected server error". Log OpenCode mencatat `plugin config hook failed` lalu `n.provider` null; error yang sama sudah muncul sejak 2026-09-10, jadi ini masalah plugin di konfigurasi pengguna, bukan PTY.
- `vt100` `contents()` menyambung baris yang ber-flag wrap. Teks layar kini dibaca per baris.

### Headless dan event (FR-11, FR-14)

- **Claude Code**: `-p --output-format stream-json --input-format stream-json --verbose --permission-prompts host --permission-prompt-tool stdio --permission-mode manual`, prompt dikirim sebagai pesan `user` di stdin. Event yang teramati: `system/init`, `system/hook_*`, `assistant` (text, tool_use, thinking), `user` (tool_result, `tool_use_result.filePath` untuk Write), `control_request`, `rate_limit_event`, `result`.
- Hooks, `CLAUDE.md`, 15 server MCP, dan 385 skill milik pengguna ikut termuat di mode headless (84 tool). Satu run kecil bernilai setara 0.32 sampai 0.34 USD menurut `total_cost_usd`; akunnya langganan (`apiKeySource: none`), jadi ini memakai kuota, bukan tagihan API. Untuk chat orchestrator (FR-20) konteks perlu dibatasi, misalnya `--setting-sources`, `--strict-mcp-config`, dan `--tools`.
- **Codex**: `exec --json --skip-git-repo-check -s workspace-write -C <dir> -`, prompt di stdin. Provider `inferhub` di `~/.codex/config.toml` tidak punya `env_key` maupun `requires_openai_auth`, sehingga setiap request ditolak 401. Event yang teramati: `thread.started`, `turn.started`, `item.completed` (tipe `error` untuk peringatan konfigurasi), `error` ("Reconnecting... n/5"), `turn.failed`. Jenis item untuk run yang berhasil (`agent_message`, `command_execution`, `file_change`) diambil dari dokumentasi Codex dan belum teramati.
- **OpenCode**: `run --format json --dir <dir> --pure <prompt>`. Event: `step_start`, `tool_use` (`part.tool`, `part.state.status/input/output`, `metadata.files` untuk file yang diubah), `text`, `step_finish` (token dan biaya). Tidak ada event "selesai"; status Done diambil dari exit proses. `--pure` mematikan plugin, tapi server MCP tetap termuat.
- Parser `events.rs` menyeragamkan ketiganya menjadi `SessionEvent` dan dicek ulang terhadap rekaman nyata lewat `air-spike replay`. Baris yang tidak dikenal menjadi `Raw`.

### Sinyal "menunggu izin" (FR-16, FR-17)

- **Claude Code headless**: `--permission-prompts host` saja tidak cukup; izin langsung ditolak (`system/permission_denied` dan `result.permission_denials`). Dengan tambahan `--permission-prompt-tool stdio`, permintaan datang sebagai `control_request` subtype `can_use_tool` (field `request_id`, `tool_name`, `input`, `permission_suggestions`, `tool_use_id`) 11.7 detik setelah start. Jawaban `control_response` dengan `behavior: allow` membuat file benar-benar dibuat. Ini jalur Approve/Deny utama untuk M1.
- **Claude Code PTY**: hook yang disuntik lewat `--settings <file>` terpicu saat dialog izin muncul. `PermissionRequest` membawa `tool_name` dan `tool_input`; `Notification` membawa `notification_type: permission_prompt`. Pola teks cadangan ("Do you want to …?" + "Esc to cancel") mendeteksi dialog pada layar ke-29 dari 30 dan tidak memberi deteksi palsu di 28 layar lain. Esc menolak izin.
- **OpenCode**: dengan izin `ask`, `opencode run` menulis `permission requested: edit (…); auto-rejecting` ke stderr dan tool call gagal. Approve/Deny untuk OpenCode di M1 harus lewat `opencode serve` dan API HTTP-nya.
- **Codex**: belum teruji karena auth. Jalur kandidat: `codex app-server` (JSON-RPC, eksperimental) dan hooks Codex.

### Proses eksternal (FR-31)

- `sysinfo` 0.39 membaca nama, argumen, folder kerja, waktu mulai, CPU, dan memori. Scan pertama 32 sampai 53 ms, berikutnya 13 sampai 25 ms.
- Uji dengan tiga CLI terbuka sekaligus (Claude Code, Codex, OpenCode, masing-masing di terminal terpisah): ketiganya muncul tepat satu kali dengan folder kerja yang benar.
- Nama proses di mesin ini: `claude.exe`, `codex.exe`, `opencode.exe`. Helper seperti `codex-windows-sandbox-service.exe` dan `codex-code-mode-host.exe` tidak dihitung sebagai sesi. Anak dari CLI yang sama (exe pembungkus OpenCode) dan proses turunan OpenCompanion dilewati.
- Mode dibaca dari flag (`-p`, `exec`, `run`, `serve`), tanpa menyimpan command line lengkap karena bisa berisi prompt atau token.

### Environment

- Sesi Claude Code mewariskan 10 variabel penanda (termasuk `CLAUDE_CODE_MESSAGING_TOKEN`) ke proses anak. Akibatnya CLI yang dijalankan dari dalamnya mematikan penyimpanan transcript. OpenCompanion kini membuang variabel ini dari setiap CLI yang dijalankan (`proc::INHERITED_SESSION_VARS`); peringatan "Transcript saving is off" hilang setelahnya.

## Keputusan untuk M1

1. Claude Code headless memakai protokol stdio control (`can_use_tool`) untuk Approve/Deny.
2. Claude Code interaktif memakai hook `PermissionRequest` dan `Notification` lewat `--settings` sebagai sinyal utama, pola teks sebagai cadangan, dan metode yang dipakai ditampilkan di detail sesi.
3. OpenCode dijalankan lewat `opencode serve` supaya izin bisa dijawab; spike berikutnya mengukur API itu.
4. Rust core menjawab `ESC[6n`; terminal di webview tidak meneruskan jawaban CPR.
5. Dialog pembuka CLI (trust folder, tawaran update) ditampilkan sebagai "Waiting for you" dan tidak pernah dijawab otomatis.
6. Chat orchestrator memakai konteks CLI yang dibatasi supaya kuota tidak habis untuk memuat tool dan skill.

## Belum selesai

- Codex: run yang berhasil dan sinyal izin, setelah auth provider di `~/.codex/config.toml` diperbaiki pemiliknya.
- OpenCode: PTY dan run tanpa `--pure` setelah plugin yang gagal diperbaiki; sinyal izin lewat `opencode serve`.
- Gemini CLI: belum terpasang.
