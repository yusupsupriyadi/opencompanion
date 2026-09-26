# PRD: OpenCompanion

| | |
|---|---|
| Status | Draft v0.1 |
| Tanggal | 2026-09-25 |
| Platform | Desktop: Tauri 2 + Svelte 5. Companion: web mobile (PWA) dilayani oleh desktop. Native mobile menyusul. |
| Desain | `design/ai-remote.pen` (Pencil), arah visual di `DESIGN.md` |

## 1. Ringkasan

OpenCompanion adalah aplikasi desktop lokal untuk mengelola AI coding CLI seperti Claude Code, Codex CLI, dan OpenCode. Dari satu jendela, Anda bisa:

1. melihat CLI apa saja yang terpasang beserta versinya,
2. menjalankan sesi CLI di folder project mana pun,
3. memberi tugas lewat AI chat bawaan yang menyusun rencana lalu mengirim tugas ke CLI yang tepat,
4. memonitor semua sesi yang sedang berjalan, termasuk yang dibuka di luar aplikasi,
5. memantau dan menjawab prompt izin dari HP lewat web companion di jaringan lokal,
6. mengatur tugas di Board kanban (Pending, Todo, In progress, Done) yang bisa langsung dijalankan ke CLI.

Semua berjalan di komputer Anda. OpenCompanion tidak punya server cloud, akun, atau telemetri. Model AI tetap diproses oleh provider masing-masing CLI, karena CLI itu sendiri yang memanggil API-nya. Satu pengecualian yang Anda pilih sendiri: planner Chat bisa memakai custom provider (FR-29), yang dipanggil langsung oleh OpenCompanion.

## 2. Masalah

- Developer yang memakai beberapa AI CLI sekaligus harus membuka banyak jendela terminal dan berpindah-pindah untuk tahu mana yang selesai, mana yang macet, dan mana yang menunggu izin.
- Prompt izin (misalnya "boleh jalankan perintah ini?") sering menunggu lama karena tidak ada yang melihat terminalnya.
- Memilih CLI mana untuk tugas apa, lalu menyalin konteks yang sama ke tiap CLI, adalah kerja manual yang berulang.
- Saat menjauh dari meja, tidak ada cara melihat status sesi tanpa remote desktop penuh.

## 3. Tujuan dan non-goals

### Tujuan

- **G1** Satu tempat untuk melihat semua sesi AI CLI di mesin ini beserta statusnya.
- **G2** Menjalankan dan mengendalikan sesi CLI (mulai, kirim input, hentikan, lanjutkan) tanpa membuka terminal terpisah.
- **G3** Memberi tugas dalam bahasa biasa lewat chat, yang diterjemahkan menjadi rencana dispatch yang Anda setujui sebelum dijalankan.
- **G4** Tahu dalam hitungan detik ketika sebuah sesi selesai, error, atau menunggu Anda, baik di desktop maupun di HP.
- **G5** Tetap lokal: tidak ada data sesi yang keluar dari mesin atau jaringan lokal Anda, kecuali yang dikirim CLI itu sendiri ke provider-nya, atau konteks Chat yang dikirim ke custom provider yang Anda atur (FR-29).

### Non-goals (v1)

- Bukan pengganti CLI. OpenCompanion tidak punya agen coding sendiri. Satu-satunya panggilan API model langsung adalah planner Chat saat Anda memilih custom provider (FR-29); sesi coding tetap selalu CLI.
- Tidak ada sinkronisasi cloud, akun, atau multi-user.
- Tidak mengontrol sesi eksternal (yang dibuka di luar app). Sesi eksternal hanya dilihat, tidak dikendalikan.
- Tidak ada editor kode atau viewer diff lengkap. Diff cukup ditampilkan ringkas, pekerjaan detail tetap di editor Anda.
- Tidak ada relay internet untuk mobile. Akses di luar jaringan rumah memakai VPN milik Anda sendiri (misalnya Tailscale).

## 4. Pengguna dan skenario

**Pengguna utama**: developer solo atau tim kecil yang menjalankan 2 atau lebih AI coding CLI secara paralel di Windows (utama), macOS, atau Linux.

Skenario:

- **S1** Pagi hari, Anda membuka OpenCompanion dan menulis di chat: "Codex perbaiki test yang gagal di ai-remote, Claude Code tulis dokumentasi API di uninote." Chat menyusun dua dispatch, Anda setujui, dua sesi berjalan.
- **S2** Anda sedang makan siang. HP bergetar: "Claude Code is waiting for approval in uninote: run `npm install`". Anda buka web companion, baca perintahnya, tekan Approve.
- **S3** Anda lupa bahwa kemarin membuka OpenCode di Windows Terminal. Overview menampilkannya di bagian "Opened outside OpenCompanion" lengkap dengan folder dan lama berjalan.
- **S4** Codex error karena belum login. Sesi ditandai error dengan pesan asli dari CLI dan saran langkah berikutnya (`codex login`).

## 5. Prinsip produk

1. **Lokal dulu.** Semua state disimpan di mesin. Server companion mati secara default.
2. **CLI adalah pekerjanya.** OpenCompanion mengatur, menampilkan, dan meneruskan. Keputusan coding tetap di CLI.
3. **Anda memegang izin.** Chat hanya mengusulkan dispatch. Tidak ada sesi yang dimulai tanpa konfirmasi Anda, kecuali Anda sendiri yang mengaktifkan auto-dispatch per project.
4. **Jujur soal batas.** Sesi eksternal ditandai read-only, dan data yang tidak bisa dibaca ditampilkan sebagai "tidak tersedia", bukan ditebak.

## 6. CLI yang didukung

Setiap CLI dihubungkan lewat **adapter** dengan kontrak yang sama: `detect`, `version`, `start_interactive`, `start_headless`, `parse_event`, `find_external`, `read_transcript`.

Data di bawah diverifikasi pada 2026-09-25 dari `--help` CLI yang terpasang di mesin pengembang. Flag bisa berubah di versi lain, jadi adapter wajib membaca versi dan menandai versi yang belum diuji.

| CLI | Versi terverifikasi | Mode headless | Format event | Lanjutkan sesi | Riwayat sesi lokal |
|---|---|---|---|---|---|
| Claude Code (`claude`) | 2.1.282 | `-p` / `--print` | `--output-format text\|json\|stream-json`, input `--input-format stream-json` | `--resume <id>`, `--continue` | `~/.claude/projects/` |
| Codex CLI (`codex`) | 0.153.4 | `codex exec` | `--json` (JSONL ke stdout), `-o <file>` untuk pesan terakhir | `codex exec resume` | `~/.codex/sessions/` |
| OpenCode (`opencode`) | 1.18.30 | `opencode run` | `--format json` | `--session <id>`, `--continue` | `~/.local/share/opencode/` (SQLite) |
| Gemini CLI (`gemini`) | belum terpasang | belum diverifikasi | belum diverifikasi | belum diverifikasi | belum diverifikasi |

Catatan per CLI:

- **Claude Code**: `--permission-mode` (pilihan `acceptEdits`, `auto`, `bypassPermissions`, `manual`, `dontAsk`, `plan`) dan `--allowedTools` menentukan apa yang boleh dijalankan tanpa bertanya. Terverifikasi di M0: headless dengan `--permission-prompt-tool stdio` mengirim permintaan izin sebagai `control_request` yang bisa dijawab host, dan sesi interaktif memicu hook `PermissionRequest` dan `Notification` (lihat `docs/spike/M0-results.md`).
- **Codex CLI**: `-C/--cd <DIR>` untuk folder kerja, `-s/--sandbox` untuk kebijakan sandbox. Codex juga punya `mcp-server` dan `app-server` (eksperimental) yang bisa menjadi jalur integrasi lebih kaya di masa depan. Sinyal izin belum teruji di M0 karena auth provider di mesin uji gagal.
- **OpenCode**: `opencode serve` menjalankan server headless, dan `opencode run --attach <url>` bisa menempel ke server itu. `--dir` menentukan folder kerja. Terverifikasi di M0: `opencode run` menolak sendiri izin yang berstatus `ask`, jadi Approve/Deny harus lewat `opencode serve`.
- CLI berikutnya setelah MVP: Gemini CLI, Aider, Qwen Code, dan CLI lain yang punya mode non-interaktif.

## 7. Functional requirements

Prioritas: **P0** wajib untuk MVP, **P1** penting setelah MVP, **P2** nanti.

### A. CLI Registry

| ID | Prio | Requirement | Acceptance criteria |
|---|---|---|---|
| FR-01 | P0 | Deteksi CLI yang terpasang lewat PATH dan lokasi instal umum. | Saat app dibuka dan saat klik Rescan, setiap CLI yang didukung tampil dengan status Installed / Not found, path executable, dan versi dari `--version`. |
| FR-02 | P0 | Tandai versi yang belum diuji adapter. | Jika versi di luar rentang yang diuji, baris CLI menampilkan "Untested version" dan fitur headless tetap bisa dicoba. |
| FR-03 | P1 | Tampilkan cara instal untuk CLI yang belum ada. | Baris "Not found" menampilkan perintah instal resmi yang bisa disalin, tanpa menjalankannya otomatis. |
| FR-04 | P1 | Path executable kustom. | Pengguna bisa memilih file executable secara manual; versi dibaca ulang setelah disimpan. |

### B. Sessions

| ID | Prio | Requirement | Acceptance criteria |
|---|---|---|---|
| FR-10 | P0 | Mulai sesi interaktif di PTY. | Pilih CLI + folder project → CLI berjalan di terminal tertanam (xterm) dengan warna ANSI dan resize yang benar. |
| FR-11 | P0 | Mulai sesi headless dengan prompt. | Pilih CLI + folder + prompt → CLI berjalan dalam mode headless; event di-parse menjadi timeline (pesan, tool call, file diubah, selesai). |
| FR-12 | P0 | Kirim input ke sesi yang berjalan. | Ketikan di input bar diteruskan ke PTY; untuk headless, dikirim sebagai giliran lanjutan bila CLI mendukung. |
| FR-13 | P0 | Hentikan sesi. | Stop mengirim sinyal berhenti yang wajar dulu, lalu kill paksa setelah jeda yang bisa diatur; status menjadi Stopped. |
| FR-14 | P0 | Status sesi. | Setiap sesi punya satu status: Starting, Running, Waiting for you, Idle, Done, Error, Stopped. Perubahan status tercatat dengan waktu. |
| FR-15 | P1 | Lanjutkan sesi. | Sesi Done/Stopped bisa dilanjutkan memakai mekanisme resume milik CLI (lihat tabel bagian 6). |
| FR-16 | P1 | Deteksi "Waiting for you". | Prompt izin atau pertanyaan dari CLI dideteksi lewat event stream (headless), hooks (bila CLI mendukung), atau pola teks per adapter (PTY). Metode yang dipakai ditampilkan di detail sesi. |
| FR-17 | P1 | Setujui / tolak dari UI. | Saat Waiting for you, detail sesi menampilkan isi permintaan dan tombol Approve / Deny yang meneruskan jawaban ke CLI. |
| FR-18 | P2 | Sesi berjalan setelah jendela ditutup. | Menutup jendela memindahkan app ke system tray; sesi tetap berjalan sampai app benar-benar keluar. Ikon tray: klik membuka jendela, menunya "Open OpenCompanion" dan "Quit OpenCompanion and stop its sessions". Pertama kali jendela masuk tray, satu notifikasi menjelaskannya. Settings › "Closing the window" bisa mematikannya, sehingga menutup jendela keluar dan menghentikan sesi. |

### C. Orchestrator Chat

| ID | Prio | Requirement | Acceptance criteria |
|---|---|---|---|
| FR-20 | P0 | Chat memakai CLI terpasang sebagai otak. | Settings menyediakan pilihan CLI untuk chat (default: Claude Code bila terpasang). Chat dijalankan dalam mode headless CLI tersebut, tanpa API key tambahan. Sebagai gantinya bisa dipilih custom provider (FR-29). |
| FR-21 | P0 | Chat menghasilkan rencana dispatch terstruktur. | Untuk permintaan tugas, jawaban chat berisi satu atau lebih kartu dispatch: nama tugas singkat, CLI, folder, prompt, mode (interactive/headless), dan alasan singkat pemilihan CLI. Sesi yang dimulai dari kartu (Chat atau Board) memakai nama tugas itu sebagai judul; sesi Claude Code interaktif tanpa nama kartu memakai nama tugas yang ditulis Claude Code sendiri di judul terminal. |
| FR-22 | P0 | Konfirmasi sebelum dispatch. | Kartu dispatch punya tombol Run, Edit, dan Discard. Tidak ada sesi yang dimulai tanpa Run, kecuali auto-dispatch aktif untuk project itu. |
| FR-23 | P0 | Validasi rencana. | Output chat divalidasi di Rust (CLI terpasang, folder ada). Jika tidak valid, kartu menampilkan alasannya dan tombol Run nonaktif. Jika output bukan JSON yang valid, jawaban tampil sebagai teks biasa. |
| FR-24 | P1 | Chat tahu konteks. | Chat menerima daftar CLI terpasang, project yang pernah dipakai, dan sesi aktif sebagai konteks, sehingga bisa menjawab "apa yang sedang dikerjakan Codex?". |
| FR-24a | P1 | Tunjuk folder dengan `@`. | Mengetik `@` di composer Chat membuka daftar folder project yang dikenal (nama + path), menyempit sesuai huruf yang diketik; panah atas/bawah memilih, Enter atau Tab menulis path lengkapnya (`@C:\path`, dalam tanda kutip bila ada spasi), Esc menutup. Opsi terakhir "Browse for a folder…" membuka dialog folder. Planner memakai folder yang ditunjuk untuk kartunya, dan folder itu ikut bisa dibaca planner (read-only) bila akses baca aktif. `nama@host` tidak membuka daftar. |
| FR-25 | P1 | Tugas lanjutan ke sesi yang ada. | Chat bisa mengusulkan "kirim ke sesi X" untuk sesi yang masih berjalan, dengan konfirmasi yang sama. Planner melihat id tiap sesi dan apakah sesi itu bisa menerima pesan sekarang (terminal yang berjalan, giliran Claude Code headless yang berjalan, atau sesi headless selesai yang punya id CLI), lalu menjawab dengan kartu follow-up. Kartu itu bernama "Follow-up for {judul sesi}", tombolnya "Send to session", hanya pesannya yang bisa diedit, dan tidak bisa masuk Board. Send mengetik pesan lalu Enter di terminal, atau memulai giliran lanjutan headless; tidak ada sesi baru. Sesi yang tidak bisa menerima pesan membuat kartunya menyebut alasannya dan Send nonaktif. |
| FR-26 | P2 | Auto-dispatch per project. | Pengguna bisa mengizinkan dispatch tanpa konfirmasi untuk project tertentu; indikator jelas tampil di chat. Settings › Chat planner › "Run cards without asking": folder ditambah lewat daftar folder yang dikenal atau Browse, dengan tekanan kedua "Start them without asking". Kartu sesi baru yang valid untuk folder itu (dan folder di dalamnya) langsung berjalan dengan mode izin default, kecuali Bypass yang turun menjadi Ask me; kartu follow-up dan kartu bermasalah tetap menunggu. Composer Chat di desktop dan HP menyebut foldernya ("Cards for uninote start without asking; the rest wait for Run."), dan kartu yang berjalan sendiri menulis "Started by itself: … runs cards without asking." |
| FR-27 | P1 | Riwayat chat per percakapan. | Setiap percakapan dengan planner tersimpan sebagai chat sendiri, dinamai dari baris pertama pesan pertamanya. Daftar "Chats" menampilkan semua chat (terbaru di atas) dan membuka riwayatnya lewat `/chat?id=`. "New chat" memulai percakapan kosong. Planner hanya membaca pesan dari chat yang sedang dibuka. Hapus chat butuh tekanan kedua dan tidak menghentikan sesi yang dimulai dari kartunya. Percakapan lama dari sebelum fitur ini menjadi satu chat. |
| FR-28 | P1 | Model dan tingkat thinking planner. | Composer Chat punya pilihan Model dan Thinking untuk CLI planner, tersimpan per CLI di Settings dan berlaku mulai pesan berikutnya. "CLI default" tidak mengirim flag apa pun. Daftar diambil dari CLI itu sendiri dan menampilkan nama berversi: Claude Code dari katalog `/model` miliknya (`~/.claude/cache/model-catalog`, misalnya "Opus 5.5", "Fable 5.1", model lama di grup "More models", ID lengkap lewat `--model` dan tingkat per model lewat `--effort`; tanpa katalog, alias `fable`, `opus`, `sonnet`, `haiku` sebagai cadangan); Codex `codex debug models` dengan `-c model_reasoning_effort`; OpenCode `opencode models --verbose` dengan `--variant` per model, dikelompokkan per provider. Tingkat yang tidak didukung model baru kembali ke default. Pilihan ini tidak tampil saat planner memakai custom provider. |
| FR-29 | P1 | Custom provider untuk planner. | Settings, Chat planner punya pilihan "Custom provider (OpenAI-compatible API)" dengan Base URL, Model, dan API key (opsional, untuk Ollama atau LM Studio boleh kosong). Chat mengirim `POST {base}/chat/completions` langsung dari OpenCompanion dengan konteks yang sama seperti CLI (FR-24), lalu jawabannya divalidasi seperti FR-23. Provider tidak punya akses file, jadi hanya melihat nama folder. API key disimpan di database lokal dan hanya dikirim ke Base URL itu. Field yang kosong atau URL tanpa http(s) ditolak sebelum disimpan, dan error provider tampil dengan kata-kata provider sendiri. |

### D. Monitoring

| ID | Prio | Requirement | Acceptance criteria |
|---|---|---|---|
| FR-30 | P0 | Overview semua sesi dari app. | Overview menampilkan sesi yang butuh Anda paling atas, lalu yang berjalan, lalu yang selesai hari ini. Setiap baris: CLI, folder, status, lama berjalan, event terakhir. |
| FR-31 | P0 | Deteksi sesi eksternal. | Proses CLI yang berjalan di luar app terdeteksi lewat daftar proses (nama executable, PID, folder kerja bila bisa dibaca, waktu mulai) dan tampil di bagian "Opened outside OpenCompanion" dengan label Read-only. |
| FR-32 | P1 | Isi sesi eksternal dari transcript. | Bila CLI menulis riwayat sesi lokal (bagian 6), detail sesi eksternal menampilkan pesan terakhir dari file itu. Bila tidak bisa dibaca, tampil "Transcript not available for this CLI". Yang dipakai adalah riwayat terbaru untuk folder proses itu yang ditulis sejak proses mulai (Claude Code `~/.claude/projects`, Codex `~/.codex/sessions`, OpenCode database di `~/.local/share/opencode` yang dibuka read-only), dibaca ulang tiap beberapa detik. Isinya pesan Anda, jawaban CLI, dan nama tool yang dipakai; OpenCompanion tidak pernah menulis ke file itu. |
| FR-33 | P1 | Garis aktivitas. | Setiap sesi menampilkan garis horizon dengan titik untuk tiap event nyata (tool call, file diubah, prompt izin, error) dalam rentang waktu yang terlihat. |
| FR-34 | P1 | Penggunaan sumber daya. | Detail sesi menampilkan CPU dan memori proses CLI (dan anak prosesnya). CPU dihitung sebagai bagian dari seluruh mesin seperti Task Manager, diukur tiap 3 detik selama sesi berjalan, bersama jumlah proses anak. |
| FR-35 | P2 | Filter dan pencarian. | Filter berdasarkan CLI, project, status; cari di judul dan prompt. Layar "All sessions" (`/history`, dari tautan di Overview) memuat semua sesi terbaru dulu, 50 per halaman dengan "Show 50 more"; kolom cari judul dan prompt (tanpa beda huruf besar kecil, `%` dan `_` dibaca sebagai teks), select CLI, folder (folder yang pernah dipakai sesi), dan status (Running, Waiting for you, Done, Error, Stopped). |

### E. Notifikasi

| ID | Prio | Requirement | Acceptance criteria |
|---|---|---|---|
| FR-40 | P0 | Notifikasi desktop. | Notifikasi OS muncul saat sesi Waiting for you, Done, atau Error. Isinya menyebut CLI dan folder. Klik membuka detail sesi. |
| FR-41 | P1 | Pengaturan notifikasi. | Pengguna bisa mematikan jenis notifikasi tertentu per CLI atau per project. Settings › Notifications: tiga saklar umum, tabel "By CLI" (Waiting, Done, Error per CLI terpasang), dan tabel "By project folder" (folder dipilih dari folder project yang dikenal atau lewat Browse, bisa dihapus). Notifikasi keluar hanya bila saklar umum, aturan CLI-nya, dan setiap aturan folder yang memuat folder sesi (termasuk subfolder) mengizinkan. HP mengikuti aturan yang sama. |
| FR-42 | P1 | Notifikasi ke HP. | Perangkat yang dipasangkan menerima notifikasi yang sama lewat companion (Web Push bila tersedia di koneksi HTTPS, selain itu tampil saat halaman terbuka). |

### F. Mobile companion (web dulu)

| ID | Prio | Requirement | Acceptance criteria |
|---|---|---|---|
| FR-50 | P0 | Server companion bisa dinyalakan/dimatikan. | Mati secara default. Saat dinyalakan, Settings menampilkan alamat LAN dan port yang dipakai. |
| FR-51 | P0 | Pairing lewat QR. | Desktop menampilkan QR berisi alamat + kode sekali pakai yang kedaluwarsa. HP yang memindai menerima token perangkat. Pairing gagal setelah kode kedaluwarsa. |
| FR-52 | P0 | Daftar sesi di HP. | HP menampilkan sesi yang sama dengan Overview, diperbarui realtime lewat WebSocket. |
| FR-53 | P0 | Detail sesi di HP. | Menampilkan status, event terbaru, dan bagian akhir output (tail). |
| FR-54 | P1 | Approve / Deny dan Stop dari HP. | Tombol yang sama dengan desktop, dengan konfirmasi untuk Stop. |
| FR-55 | P1 | Kelola perangkat. | Desktop menampilkan daftar perangkat yang dipasangkan dengan waktu terakhir terhubung; tiap perangkat bisa dicabut. |
| FR-56 | P1 | State tidak terhubung. | Jika desktop tidak terjangkau, HP menampilkan layar "Can't reach your desktop" dengan penyebab yang mungkin dan tombol coba lagi. |
| FR-57 | P2 | Chat dari HP. | Mengirim pesan ke orchestrator chat dari HP, dengan konfirmasi dispatch yang sama. |
| FR-58 | P2 | Aplikasi native mobile. | Setelah web companion stabil, versi native memakai API companion yang sama. |
| FR-59 | P1 | Mulai sesi dan kirim pesan dari HP. | HP bisa memulai sesi baru (CLI, folder project yang dikenal desktop atau path lain, mode, prompt, mode izin) dan mengirim pesan ke sesi: follow-up untuk headless dengan aturan yang sama seperti desktop, teks plus Enter dan tombol Enter, Esc, panah, Ctrl+C untuk terminal interaktif, serta Resume untuk terminal yang sudah tertutup. |

### G. Riwayat dan Settings

| ID | Prio | Requirement | Acceptance criteria |
|---|---|---|---|
| FR-60 | P0 | Riwayat sesi dan tugas. | Sesi dan dispatch tersimpan di SQLite lokal: prompt, CLI, folder, status akhir, waktu. |
| FR-61 | P0 | Tema Siang / Senja. | Toggle tema berfungsi penuh di kedua mode; default mengikuti tema OS. |
| FR-62 | P1 | Retensi data. | Pengguna menentukan berapa lama output sesi disimpan, dan bisa menghapus riwayat. Settings › History: "Keep finished sessions for" Forever (default), 90, 30, 7 atau 1 hari, dicek saat app dibuka dan tiap jam; sesi selesai yang lebih lama dihapus bersama event, log terminal, dan file hook-nya. "Delete finished sessions" menghapus semua sesi selesai dengan file yang sama. Kartu Board dan kartu Chat yang menjalankannya tetap ada tanpa tautan sesi. File yang diubah CLI di project tidak disentuh. |
| FR-63 | P1 | Bahasa UI. | UI bisa diganti Inggris / Indonesia. Settings › Language memilih English atau Bahasa Indonesia; semua layar desktop, halaman HP (yang membaca pilihan desktop lewat `/api/hello`), notifikasi OS, dan menu tray mengikutinya tanpa restart. Pesan OpenCompanion sendiri dari backend ikut diterjemahkan saat ditampilkan; teks yang ditulis CLI (output, jawaban planner, pesan error CLI) tetap seperti aslinya. |
| FR-64 | P2 | Mulai saat login. | Opsi menjalankan OpenCompanion saat login OS. Windows: Settings › "Window and sign-in" menulis atau menghapus nilai `OpenCompanion` di `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` lewat `reg.exe`; app yang dimulai saat login membawa `--hidden` dan langsung masuk tray. Keadaannya dibaca dari Windows setiap kali Settings dibuka. macOS dan Linux belum. |
| FR-65 | P1 | Mode izin sesi: Ask me, Plan, Auto, Bypass. | Settings menyimpan mode default untuk sesi baru; New session dan Run dari Board bisa memilih mode lain untuk satu sesi. Tiap mode diteruskan sebagai flag resmi masing-masing CLI, dan tabelnya tampil di Settings. Bypass baru tersimpan setelah konfirmasi kedua, dan form sesi memberi peringatan saat Bypass dipilih. Planner Chat tidak terpengaruh dan tetap read-only. |
| FR-66 | P1 | Ukuran teks. | Settings menawarkan 90%, 100% (default), 110%, 125%, 150%. Pilihan langsung berlaku di semua layar desktop sebagai zoom webview, jadi teks, tombol, dan spasi membesar bersama dan layout mengikuti breakpoint yang ada. Tersimpan di Settings dan diterapkan lagi saat aplikasi dibuka. Halaman HP tidak terpengaruh dan mengikuti ukuran teks HP. |

### H. Board (kanban tugas)

Kolom tetap: **Pending** (belum siap: ide, pertanyaan terbuka, pekerjaan terblokir), **Todo** (siap dikirim ke CLI), **In progress** (sesi CLI sedang mengerjakan), **Done** (selesai).

| ID | Prio | Requirement | Acceptance criteria |
|---|---|---|---|
| FR-70 | P1 | Board dengan empat kolom tetap. | Setiap kartu punya judul, catatan, project, dan CLI (opsional). Kartu dan urutannya tersimpan di SQLite lokal dan bertahan setelah app ditutup. |
| FR-71 | P1 | Tambah, edit, hapus kartu. | Judul wajib diisi. Hapus butuh konfirmasi. Kartu baru masuk ke Pending kecuali pengguna memilih kolom lain. |
| FR-72 | P1 | Pindah kartu dengan drag dan keyboard. | Kartu bisa di-drag antar kolom. Tanpa mouse, kartu dipindah dengan mengganti kolomnya di dialog kartu; fokus kembali ke kartu yang dipindah dan perpindahan diumumkan ke screen reader. |
| FR-73 | P1 | Run dari kartu Todo. | Run membuka dialog sesi yang sudah terisi CLI kartu, folder project, dan catatan kartu sebagai prompt. Setelah Start, kartu pindah ke In progress dan terhubung ke sesi itu. |
| FR-74 | P1 | Kartu mengikuti status sesi. | Kartu In progress menampilkan status sesinya (Running, Waiting for you dengan aksen, Error) dan tautan ke sesi. Sesi yang selesai tanpa error memindahkan kartu ke Done otomatis; sesi error membiarkan kartu di In progress dengan chip Error. |
| FR-75 | P1 | Filter per project. | Filter menyembunyikan kartu project lain; jumlah kartu di tiap kolom mengikuti filter. |
| FR-76 | P2 | Kartu dari Chat. | Kartu dispatch di Chat punya aksi "Add to board" yang membuat kartu Todo dengan prompt, CLI, dan folder yang sama. |
| FR-77 | P2 | Board di HP. | Companion menampilkan board per kolom; kartu bisa dipindah kolom dan dijalankan, dan kartu Waiting for you bisa di-Approve dari sana. Kartu baru dan edit isi kartu tetap di desktop. |

## 8. Alur utama

**F1. Pertama kali dibuka**: Onboarding → scan CLI → daftar hasil (terpasang, versi, belum ada) → pilih CLI untuk chat → Overview kosong dengan ajakan menulis tugas pertama.

**F2. Tugas lewat chat**: tulis permintaan → chat (CLI headless) menjawab dengan kartu dispatch → validasi → Anda tekan Run → sesi muncul di Overview → notifikasi saat selesai/butuh Anda.

**F3. Approve dari HP**: CLI minta izin → status Waiting for you → notifikasi desktop + HP → buka detail di HP → baca perintah → Approve → sesi lanjut Running.

**F4. Sesi eksternal**: scan proses berkala → CLI yang tidak dikenal app tampil sebagai Read-only → klik → detail menampilkan folder, lama berjalan, dan pesan terakhir dari transcript bila ada.

**F5. Tugas lewat Board**: tulis kartu di Pending → lengkapi catatannya → pindah ke Todo → Run → sesi berjalan, kartu di In progress → CLI minta izin, kartu diberi aksen → Approve → sesi selesai → kartu pindah ke Done.

## 9. Layar (peta ke desain)

| Kode | Layar | FR terkait |
|---|---|---|
| D1 | Onboarding: scan CLI | FR-01, FR-20 |
| D2 | Overview | FR-30, FR-31, FR-33 |
| D3 | Orchestrator Chat | FR-20 s/d FR-25, FR-27, FR-28, FR-29 |
| D4 | Session detail | FR-10 s/d FR-17, FR-34 |
| D5 | CLI Manager | FR-01 s/d FR-04 |
| D6 | New session (modal) | FR-10, FR-11 |
| D7 | Settings: Mobile access | FR-50, FR-51, FR-55 |
| D8 | State kosong, memuat, error | semua tampilan data |
| D9 | Tema Senja (Overview, Session detail) | FR-61 |
| D10 | Board | FR-70 s/d FR-75 |
| M1 | Pair device | FR-51 |
| M2 | Sessions | FR-52 |
| M3 | Session detail | FR-53, FR-54 |
| M4 | Can't reach your desktop | FR-56 |

## 10. Arsitektur

```
┌────────────────────────── OpenCompanion (satu proses Tauri) ──────────────────────────┐
│  Webview: Svelte 5 SPA (SvelteKit adapter-static)                                     │
│    Overview · Chat · Session (xterm.js) · CLI Manager · Settings                       │
│        │  invoke / events (Tauri IPC)                                                  │
│  Rust core                                                                             │
│    cli_registry ── adapters (claude, codex, opencode, ...)                             │
│    session_manager ── PTY sessions ── headless runner ── event parser per adapter      │
│    orchestrator ── menjalankan CLI chat headless, memvalidasi rencana dispatch         │
│    monitor ── process scanner ── transcript watcher                                    │
│    notifier ── notifikasi OS + fan-out ke companion                                    │
│    store ── SQLite (sesi, event, dispatch, perangkat)                                  │
│    companion_server (mati default) ── HTTP + WebSocket, token per perangkat            │
└──────────────┬──────────────────────────────────────────────┬─────────────────────────┘
               │ spawn / PTY                                   │ LAN (atau VPN milik Anda)
        claude · codex · opencode                        Browser HP: web companion (PWA)
```

Keputusan teknis:

- **Frontend**: Svelte 5 dengan SvelteKit `adapter-static` dalam mode SPA (`fallback: 'index.html'`, `ssr = false` di root layout), sesuai panduan resmi Tauri 2 untuk SvelteKit. Web companion adalah build terpisah dari codebase yang sama (route mobile), dilayani oleh `companion_server`.
- **Plugin Tauri 2 resmi** yang direncanakan: `notification`, `store`, `sql` (SQLite), `single-instance`, `window-state`, `dialog`, `opener`, lalu `autostart` dan `updater` setelah MVP.
- **Crate kandidat** (belum diuji, diputuskan di M0): `portable-pty` untuk PTY lintas platform (ConPTY di Windows), `sysinfo` untuk scan proses, `notify` untuk memantau file transcript, `axum` untuk HTTP + WebSocket companion.
- **Model data inti**: `CliInstall`, `Project`, `Session` (cli, folder, mode, status, pid, started_at, ended_at), `SessionEvent` (tipe, ringkasan, payload), `Dispatch` (asal chat, rencana, keputusan), `Device` (nama, token hash, last_seen), `Task` (judul, catatan, project, cli, kolom, urutan, session_id, dibuat, diubah).

Cara kerja orchestrator chat:

1. Pesan Anda + konteks (CLI terpasang, project, sesi aktif) dikirim ke CLI chat dalam mode headless dengan instruksi sistem yang meminta jawaban JSON berisi rencana dispatch.
2. Rust mem-parse dan memvalidasi JSON. Chat tidak pernah menjalankan perintah shell sendiri; satu-satunya efek sampingnya adalah usulan dispatch.
3. Kartu dispatch tampil. Run memanggil `session_manager` dengan parameter dari kartu yang sudah divalidasi.

## 11. Keamanan dan privasi

- Tidak ada telemetri dan tidak ada panggilan jaringan dari OpenCompanion sendiri, kecuali server companion saat Anda nyalakan.
- OpenCompanion tidak menyimpan kredensial CLI. Login tetap diurus oleh masing-masing CLI.
- Server companion: mati default, hanya menerima perangkat yang dipasangkan, token disimpan dalam bentuk hash, kode pairing sekali pakai dengan masa berlaku singkat, batas percobaan pairing.
- HTTP di LAN tidak terenkripsi. Settings menampilkan peringatan ini dan merekomendasikan VPN dengan HTTPS (misalnya Tailscale) untuk penggunaan di luar jaringan tepercaya.
- Dispatch dan Approve/Deny dari HP tercatat di riwayat dengan nama perangkatnya.
- Output sesi bisa berisi rahasia (env, token). Output disimpan lokal saja dan mengikuti pengaturan retensi (FR-62).

## 12. Non-functional requirements

Angka di bawah adalah target desain, belum diukur.

- Status sesi di UI berubah dalam 1 detik setelah event diterima dari CLI.
- Tanpa sesi aktif, app tidak memakai CPU secara berarti (scan proses berkala dengan interval yang bisa diatur).
- Terminal tetap responsif untuk output panjang (render lewat xterm.js, riwayat output dibatasi dan sisanya disimpan ke disk).
- Windows 11 didukung penuh di MVP. macOS dan Linux dibangun dari codebase yang sama dan diuji setelah MVP.
- Semua kontrol bisa dipakai dengan keyboard, fokus terlihat, kontras teks memenuhi WCAG AA di dua tema.

## 13. Milestone

| Milestone | Isi | Selesai jika |
|---|---|---|
| M0 Spike | PTY di Windows (ConPTY) dengan Claude Code, Codex, OpenCode; parse event headless tiap CLI; deteksi proses eksternal; uji sinyal "menunggu izin". | Tiga CLI bisa dijalankan interaktif dan headless dari prototipe Rust, dan hasil uji deteksi izin tercatat per CLI. |
| M1 Desktop MVP | FR P0 bagian A, B, C, D, E, G. | Alur F1, F2, F4 berjalan end-to-end di Windows 11. |
| M2 Web companion | FR P0 bagian F + FR-54, FR-56. | Alur F3 berjalan dari HP di jaringan yang sama. |
| M3 Setelah MVP | FR P1 sisanya, CLI tambahan, macOS/Linux, lalu native mobile (FR-58). | Ditentukan setelah M2. |

## 14. Risiko

| Risiko | Dampak | Mitigasi |
|---|---|---|
| Flag atau format event CLI berubah saat update. | Parse event rusak, status salah. | Adapter mencatat versi yang diuji, fallback ke tampilan teks mentah, peringatan "Untested version". |
| Deteksi "menunggu izin" di PTY bergantung pada pola teks TUI. | Prompt izin terlewat. | Utamakan event stream dan hooks; pola teks hanya fallback, dan metode yang dipakai ditampilkan ke pengguna. |
| Folder kerja proses eksternal tidak selalu bisa dibaca di Windows. | Sesi eksternal tampil tanpa folder. | Tampilkan "Folder unknown", cocokkan dengan transcript bila memungkinkan. |
| PWA dan Web Push butuh HTTPS; LAN biasa hanya HTTP. | Tidak bisa install PWA / push di LAN. | MVP companion berjalan sebagai halaman web biasa; HTTPS lewat VPN (misalnya Tailscale) sebagai jalur PWA. Diputuskan di M2. |
| Chat headless memakai kuota langganan CLI. | Biaya/kuota habis lebih cepat. | Pilihan CLI chat bisa diganti; konteks yang dikirim dibatasi. |
| Ketentuan penggunaan tiap CLI untuk otomatisasi. | Pola pemakaian tertentu mungkin tidak diizinkan provider. | Tinjau ketentuan tiap CLI sebelum M1; OpenCompanion hanya menjalankan CLI resmi dengan akun pengguna sendiri. |

## 15. Pertanyaan terbuka

1. Apakah Gemini CLI termasuk MVP, atau cukup Claude Code, Codex, OpenCode?
2. Apakah HP boleh mengirim teks bebas ke sesi (bukan hanya Approve/Deny/Stop) di M2?
3. Apakah perlu mode "worktree per dispatch" agar dua CLI tidak mengubah folder yang sama bersamaan?
4. Logo final (nama sudah OpenCompanion; saat ini wordmark teks).

## 16. Istilah

- **Sesi**: satu proses CLI yang berjalan untuk satu folder.
- **Dispatch**: usulan dari chat untuk memulai sesi atau mengirim tugas ke sesi.
- **Headless**: CLI berjalan tanpa TUI, menerima prompt dan mengeluarkan event.
- **Sesi eksternal**: proses CLI yang tidak dijalankan oleh OpenCompanion.
- **Companion**: web app di HP yang terhubung ke desktop lewat jaringan lokal.
