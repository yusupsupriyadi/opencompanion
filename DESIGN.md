# DESIGN.md: OpenCompanion

Arah desain ini ditranskripsi dari gambar referensi yang diberikan pemilik produk (lukisan padang rumput sore hari, langit krem-mentega, pendaki berjaket biru duduk di batang kayu, garis tanah pixel di tengah). Dokumen ini adalah data desain untuk membangun layar di Pencil dan nanti di Svelte. Kebutuhan fitur ada di `docs/PRD.md`.

## 1. Identitas

- **Produk**: OpenCompanion, aplikasi desktop lokal untuk menjalankan, memberi tugas, dan memonitor AI coding CLI.
- **Kepribadian**: tenang, hangat, bisa diandalkan. Seperti duduk di bukit sambil mengawasi ladang: Anda tidak perlu menatap terminal terus, aplikasi yang memberi tahu saat ada yang butuh Anda.
- **Design Read**: control room lokal untuk developer yang menjalankan beberapa AI CLI sekaligus, dengan bahasa visual pastoral painterly plus aksen garis pixel.
- **Dial**: ENERGY 2 / RHYTHM 2 / MOTION 1.
  - ENERGY 2: ilustrasi dan warna hangat memberi karakter, layar kerja harian tetap tenang.
  - RHYTHM 2: struktur aplikasi konsisten (sidebar + area kerja), dipecah oleh momen berilustrasi (onboarding, empty state, offline mobile).
  - MOTION 1: hanya hover, focus, dan transisi saat status berubah. Tidak ada animasi berulang. Pengecualian atas permintaan owner (2026-09-25): judul sesi Running di sidebar bergradasi pelangi `--rainbow` yang bergeser pelan (4 detik per putaran), berhenti di `prefers-reduced-motion`.
- **Satu fokus per layar**: setiap layar punya satu elemen utama (lihat bagian 9 dan 10). Di Overview itu panel "Needs you".

## 2. Palet

Maksimal 3 warna inti + 1 aksen. Warna status hanya muncul di chip status, titik status, dan terminal.

| Token | Siang (light) | Senja (dark) | Asal di referensi | Dipakai untuk |
|---|---|---|---|---|
| `bg` | `#FBF3CF` | `#1B1F1A` | langit krem | latar jendela |
| `surface` | `#FFFAE6` | `#242A22` | awan terang | panel, kartu, input |
| `surface-2` | `#F4E7B0` | `#2E3529` | cahaya di awan | hover, item aktif, header tabel |
| `line` | `#E4D69E` | `#343C30` | batas awan | pemisah dan border panel |
| `line-strong` | `#8C8A5E` | `#6E7862` | bayangan rumput | border input, tombol sekunder |
| `ink` | `#33361F` | `#F3EBC8` | teks "No Internet" | teks utama |
| `ink-2` | `#5B5D3E` | `#BDB895` | teks daftar | teks sekunder, ikon diam |
| `forest` | `#2F5A36` | `#9CC45A` | pinus di lereng | tombol utama, ikon nav aktif, focus ring |
| `on-forest` | `#FBF3CF` | `#1B1F1A` | | teks di atas `forest` |
| `accent` | `#2C5A8C` | `#7FAEDD` | jaket pendaki | hanya momen "butuh Anda": border panel Needs you, tombol Approve, titik sesi menunggu |
| `on-accent` | `#FFF8E1` | `#1B1F1A` | | teks di atas `accent` |

Warna status (selalu berpasangan dengan label teks):

| Token | Siang (isi / teks) | Senja (isi / teks) | Label | Asal |
|---|---|---|---|---|
| `st-run` / `st-run-ink` | `#B5D07A` / `#33361F` | `#9CC45A` / `#1B1F1A` | Running | rumput padang |
| `st-wait` / `st-wait-ink` | `#F2C94C` / `#33361F` | `#E9B92E` / `#1B1F1A` | Waiting for you | bunga kuning |
| `st-err` / `st-err-ink` | `#A8472E` / `#FFF8E1` | `#E0876A` / `#1B1F1A` | Error | kulit kayu |
| `st-idle` / `st-idle-ink` | `#EFE3AE` / `#5B5D3E` | `#2E3529` / `#BDB895` | Done, Idle, Stopped, Off | langit pucat |
| read-only | tanpa isi, border `line-strong` / `ink-2` | sama | Read-only | |

Terminal (panel gelap di kedua tema, "teduh di bawah pohon"):

| Token | Siang | Senja |
|---|---|---|
| `term-bg` | `#1E2B1F` | `#121710` |
| `term-text` | `#EDE6C4` | `#EDE6C4` |
| `term-dim` | `#A9B38F` | `#A9B38F` |
| `term-green` | `#A6CF6A` | `#A6CF6A` |
| `term-yellow` | `#F0C23A` | `#F0C23A` |
| `term-red` | `#E88B6B` | `#E88B6B` |
| `rainbow` (daftar warna gradasi, hanya judul sesi Running di sidebar) | `#A3262A` `#8C4300` `#276127` `#2C5A8C` `#6D3FA8` | `#F4A3A3` `#F2BE7E` `#E8DA7A` `#A8D66E` `#8FBCEB` `#C9AEF5` |

Scrim modal: `#33361F73` (ink 45%). Panel di atas ilustrasi memakai glass (di bawah).

Latar aplikasi: `meadow-day` (`static/meadow-day.png`, 1672 × 941) di belakang aplikasi (fixed, fit: utuh tanpa terpotong atau ditarik, posisi 70% bawah; pita sisa di tepi jendela diisi salinan lukisan yang sama versi cover dengan blur 32px), terlihat langsung di area kerja. Di Siang lukisan tampil utuh tanpa lapisan warna (keputusan pemilik produk, 26 September 2026); di Senja ada `haze`:

| Token | Siang | Senja |
|---|---|---|
| `haze` | tidak ada | `bg` 38% rata (`#1B1F1A61`), dan seluruh lapisan diberi `filter: brightness(.58)` |
| `glass` (tint `surface`) | `#FFFAE6D1` (82%) | `#242A2273` (45%) |
| `glass-thin` (riwayat chat dan live rail di Chat) | `#FFFAE68F` (56%) | `#242A224D` (30%) |
| `glass-2` (tint `surface-2`) | `#F4E7B0E6` (90%) | `#2E35298C` (55%) |
| `glass-bg` (tint `bg`) | `#FBF3CFD9` (85%) | `#1B1F1A66` (40%) |
| `glass-pop` (dialog dan pop-up) | `#FFFAE6E0` (88%) | `#242A22A6` (65%) |
| `glass-toast` | `#33361FD9` (ink 85%) | `#F3EBC8D9` (ink 85%) |
| `glass-blur` | `blur(24px) saturate(1.4)` | sama |
| `glass-rim` (kilap tepi) | `inset 0 1px 0 #FFFFFFB3, inset 0 0 0 1px #FFFFFF4D` | `inset 0 1px 0 #FFFFFF2E, inset 0 0 0 1px #FFFFFF14` |
| `solid` (alas teks error) | `#FFFAE6` | `#242A22` |

- Tanpa haze, teks `ink` di atas pohon, batang kayu, dan rumput gelap turun sampai sekitar 1:1. Karena itu setiap blok teks di atas lukisan duduk di **alas** glass radius 10: kepala halaman, toolbar Skills, balasan Planner, panel samping sesi, dan "Session not found" dengan padding 12/16; kepala section, kepala kolom Board, baris Activity, teks bantu, dan catatan dengan padding 8/12. Kepala halaman, kepala kolom, baris Activity, dan panel selebar barisnya; kepala section, teks bantu, catatan, dan balasan Planner memeluk teksnya.
- Teks di alas tetap memakai `ink` untuk teks sekunder juga, jadi hierarki dibawa ukuran dan ketebalan.
- **Liquid glass** (keputusan pemilik produk, 26 September 2026, menggantikan aturan glass hanya di sidebar dan batas antislop R-10 satu sampai dua elemen): sidebar, tombol jendela, semua alas teks, kartu, dan panel (baris sesi, Needs you, state box, riwayat chat, composer, live rail, gelembung pesan, badan kolom Board, kartu Settings, tabel, panel Onboarding, dialog, toast) memakai `glass-blur` dan `glass-rim`. Di dalamnya `surface`, `surface-2`, dan `bg` diganti `glass`, `glass-2`, dan `glass-bg`, jadi tombol secondary, input, select, segmented, opsi, dan baris ikut transparan; input dan tombol secondary juga diberi `glass-rim`. Tombol primary, chip status, dan terminal tetap solid.
- Tiap tint adalah yang paling tipis yang masih menjaga `ink-2` 4.5:1 di atas lukisan yang diburamkan. Pop-up (dialog, live rail versi pop-up, daftar folder @) mengambang di luar glass induknya sehingga tidak ada yang diburamkan di belakangnya, jadi memakai `glass-pop` yang lulus tanpa blur. Merah `st-err` butuh 89% di belakangnya, jadi teks error (`.err-text`, pesan field, tombol Delete kartu) memakai alas kecil `solid`.
- Riwayat chat dan live rail yang menempel di Chat setinggi sidebar, jadi keduanya memakai `glass-thin` agar tidak terbaca sebagai sidebar kedua (permintaan pemilik produk, 26 September 2026). Tint setipis ini hanya lulus dengan `ink`, jadi teks sekunder di dalamnya juga `ink` dan hierarkinya dibawa ukuran dan ketebalan.
- Senja: brightness .58 dengan haze 38% menjaga `ink` di 5.07:1 bahkan langsung di atas lukisan, jadi warna lukisan terlihat tanpa melepas cadangan kontras itu.
- Kartu, chip, terminal, tabel, dan badan kolom Board tetap solid.

### Kontras yang sudah dicek

Dicek dengan `contrast-check.py` (WCAG AA, 4.5:1 teks normal, 3:1 teks besar dan non-teks).

| Pasangan | Siang | Senja |
|---|---|---|
| `ink` di `bg` | 11.15 | 13.94 |
| `ink-2` di `bg` | 6.11 | 8.31 |
| `ink-2` di `surface` | 6.52 | 7.31 |
| `ink-2` di `surface-2` | 5.49 | 6.31 |
| `on-forest` di `forest` | 7.14 | 8.32 |
| `accent` di `bg` (teks) | 6.38 | 7.16 |
| `on-accent` di `accent` | 6.70 | 7.16 |
| teks chip running | 7.27 | 8.32 |
| teks chip waiting | 7.84 | 9.10 |
| teks chip error | 5.48 | 6.24 |
| teks chip idle | 5.28 | 6.31 |
| `forest` (focus ring) di `bg` | 7.14 | 8.32 |
| `line-strong` di `bg` (non-teks) | 3.18 | 3.60 |
| `term-text` di `term-bg` | 11.77 | 14.46 |
| `ink-2` di `glass` kartu dan alas di atas lukisan yang diburamkan (titik terburuk, 1100 sampai 2560 lebar) | 4.67 | 4.66 |
| `ink-2` di `glass` sidebar (titik terburuk) | 4.69 | 4.81 |
| `ink-2` di `glass-2` (badan kolom Board, gelembung pesan) | 4.63 | 4.63 |
| `ink-2` di `glass-bg` (baris CLI di luar OpenCompanion) | 4.67 | 4.76 |
| `ink-2` di `glass-pop` tanpa blur (pop-up) | 4.94 | 5.45 |
| `ink` di `glass-thin` (riwayat chat dan live rail, titik terburuk, 360 sampai 2560 lebar) | 4.98 | 6.86 |
| `forest` (focus ring) di `glass-thin` (titik terburuk) | 3.19 | 4.09 |
| `ink` langsung di atas lukisan + `haze` Senja (titik terburuk) | tidak dipakai | 5.07 |
| `term-dim` / `term-green` / `term-yellow` / `term-red` di `term-bg` (Siang) | 6.71 / 8.27 / 8.79 / 5.87 | |
| `st-err` sebagai teks di `surface` (pesan error di baris CLI) | 5.57 | 5.49 |
| `st-err` sebagai teks di `surface-2` (tombol Delete, pesan error di kolom) | 4.69 | 4.73 |
| `accent` sebagai ikon/teks di `surface` (status Waiting di kartu Board) | 6.81 | 6.30 |
| `forest` sebagai ikon/teks di `surface` (Run, Tested, Chat planner) | 7.61 | 7.32 |

Pasangan baru di luar tabel ini wajib dicek dulu sebelum dipakai.

## 3. Tipografi

| Peran | Font | Alasan |
|---|---|---|
| UI | Nunito (600, 700, 800) | terminal huruf membulat paling dekat dengan judul "No Internet" di referensi; dipilih setelah membandingkan Nunito, Andika, Figtree, Lexend di canvas |
| Terminal, path, perintah, versi | IBM Plex Mono (400, 600) | monospace hanya di tempat yang memang fungsional; Plex punya bentuk humanis yang serasi dengan Nunito |
| Label status kecil | Pixelify Sans (400) | menggemakan garis tanah pixel di referensi; hanya di chip status, label "Activity", dan label "Planner" |

Skala:

| Ukuran | Berat | Dipakai untuk |
|---|---|---|
| 30 | 800 | judul onboarding |
| 28 | 800 | H1 halaman (Overview, Chat, CLIs, Settings) |
| 24 | 800 | judul sesi di Session detail |
| 22 | 800 | judul modal, H1 mobile |
| 18 | 800 | judul panel Needs you, judul section di Settings |
| 16 | 800 | judul section (Running in OpenCompanion) |
| 15 | 700 | judul tugas di baris sesi |
| 14 | 600/700 | body, label field, tombol, nav |
| 13 | 400/600 | meta, teks bantuan, isi tabel |
| 12 | 400 | caption, path mono, chip pixel |

Tidak ada label uppercase dengan tracking lebar. Line height paragraf 1.4 sampai 1.45, output terminal 1.45.

## 4. Spasi, grid, bentuk

- Skala spasi: 2, 4, 6, 8, 10, 12, 14, 16, 20, 22, 24, 28, 32, 40.
- Jendela desktop acuan 1440 × 900. Minimum yang harus tetap rapi: 1100 × 700 (kolom kanan Session detail dan Live rail di Chat disembunyikan di bawah 1280, bisa dibuka lewat tombol).
- Kerangka: jendela tanpa frame native; baris atas setinggi 36 adalah Title bar (bagian 8), lalu padding 12 di kiri, kanan, bawah dan gap 12. Sidebar adalah panel glass radius 14; area kerja tidak berpanel sehingga lukisan terlihat, dan isinya berupa kartu glass atau teks di alas glass (bagian 2). Live rail di Chat adalah panel glass tersendiri.
- Sidebar 240 lebar, padding 22/16, gap antar blok 22.
- Area utama: padding 28 atas-bawah, 40 kiri-kanan, gap antar blok 24 sampai 28.
- Mobile acuan 390 × 844. Padding sisi 16, gap 12 sampai 16, target sentuh minimal 44 × 44, tombol aksi utama tinggi 48.
- Radius: `r-sm` 6 (chip, input, kotak kode), `r-btn` 8 (tombol, item nav, option card), `r-md` 10 (kartu, tabel, terminal), `r-lg` 14 (panel Needs you, modal, panel onboarding). Hanya switch yang berbentuk pill.
- Border: 1px `line` untuk panel; 1px `line-strong` untuk input dan tombol sekunder; 1.5px `accent` hanya untuk panel Needs you; 2px `forest` untuk option card terpilih.
- Shadow hanya pada modal: offset 0/12, blur 40, `#33361F40`. Elemen lain datar.
- Glass memakai liquid glass di semua kartu, panel, dialog, dan toast (bagian 2). Chip dan terminal tidak. Tidak ada glow, gradient dekoratif, atau background grid.

## 5. Ikon

- Library: Phosphor (regular), ukuran 16 di tombol, 18 di nav dan field, 20 di brand.
- Alasan: garis Phosphor regular sedikit lebih tebal dan membulat, serasi dengan Nunito.
- Ikon yang dipakai: `house` (Overview), `chats-circle` (Chat), `terminal-window` (CLIs, Open session), `books` (Skills), `magnifying-glass` (cari skill), `kanban` (Board), `gear-six` (Settings), `trash` (hapus kartu), `arrow-square-out` (Open session di kartu), `spinner-gap`/`hand-palm`/`warning-circle` (status kartu), `note` (kartu punya catatan), `link-break` (kartu tanpa sesi), `cloud` (brand), `device-mobile` (Phone access), `sun`/`moon` (tema), `plus`, `play`, `stop`, `check`, `x`, `arrow-clockwise` (Rescan, Restart), `swap`, `pencil-simple`, `paper-plane-tilt`, `folder-simple`, `folder-simple-plus`, `push-pin` (pin folder dan sesi di sidebar; terisi saat aktif), `sidebar-simple` (buka/tutup Live rail di Chat; terisi saat terbuka), `copy`, `check-circle`, `caret-down`, `brain` (pemilih Thinking di composer), `radio-button`/`circle`, `warning`, `qr-code`, `camera`, `caret-left`, `minus`/`square`/`browsers`/`x` (tombol jendela: Minimize, Maximize, Restore, Close).
- Setiap CLI ditandai **CliMark** berisi logo brand dari worldvectorlogo.com (atas permintaan pemilik produk), disimpan di `design/logos/`:
  - Claude Code: `claude.svg` (slug `claude-logo`, starburst oranye, warna asli).
  - Codex CLI: `openai.svg` (slug `openai-2`, ikon OpenAI). Dipasang sebagai SVG inline dengan `fill: currentColor` (warna `ink`) supaya terlihat di Siang dan Senja.
  - Gemini CLI: `gemini.svg`, bintang Gemini yang diambil utuh dari wordmark resmi (slug `gemini-ai`) tanpa digambar ulang. Slug `gemini` sampai `gemini-4` adalah perusahaan lain.
  - OpenCode: tidak ada di worldvectorlogo, jadi tetap monogram `OC` sampai ada logo resmi.
- Logo selalu didampingi nama CLI dalam teks, karena logo tidak dihitung sebagai label.

## 6. Motif identitas: garis horizon

Track aktivitas sesi, diambil dari garis tanah pixel di referensi.

- Ukuran 220 × 16 (frame bebas posisi).
- Ground: garis 1px `line-strong` di y 11 selebar penuh.
- Specks: 12 kotak 2 × 1 `line-strong` di bawah garis (y 14 atau 15 bergantian), jarak tidak rata seperti butir tanah.
- Event: kotak 4 × 4 di atas garis (y 7). Warna `ink-2` untuk command dan file edit, `st-wait` untuk prompt izin, `st-err` untuk error. Posisi x mengikuti waktu event dalam rentang yang terlihat.
- Now: kotak 3 × 8 `forest` di ujung kanan, hanya tampil saat sesi Running. Tidak berkedip; bergeser saat event baru masuk.
- Dipakai di: baris sesi (Overview), header Session detail (dengan label pixel "Activity"), Live rail di Chat, kartu sesi mobile.

## 7. Ilustrasi

Aturan: tampil penuh di momen tanpa data (onboarding, empty state, offline mobile), dan sebagai latar aplikasi desktop (utuh di Siang, di bawah `haze` di Senja; bagian 2). Teks tidak pernah jatuh langsung di atas lukisan: teks berada di alas glass, kartu glass, atau panel glass.

| File | Status | Dipakai di | Prompt |
|---|---|---|---|
| `design/meadow-day.png` (1408 × 768) | sudah dibuat | D1 Onboarding, D8 empty state | "Soft painterly anime-inspired landscape painting, gouache and watercolor texture, warm late-afternoon light. Pale butter-yellow sky (#FBF3CF) with large soft cream and pale teal clouds filling the upper 60 percent of the image, calm and mostly empty so text can sit on it. A rolling green meadow hill across the lower third, sloping gently from left to right, a dark green pine forest along the ridge, tiny yellow wildflowers in the foreground grass. A tiny distant figure in a blue jacket sits on a fallen log in the lower right, seen from behind. No text, no animals, no buildings." |
| `static/meadow-day.png` (1672 × 941) | dipakai aplikasi | latar desktop, D1 Onboarding, header ponsel | versi baru dari pemilik produk: langit biru pekat dengan awan sore, padang dan tokoh yang sama; prompt tidak tercatat |
| `design/meadow-portrait.png` | belum | M1 Pair device, M4 offline | "Same painterly gouache and watercolor style and palette as a warm late-afternoon meadow painting. Tall vertical composition. Upper half: pale butter-yellow sky (#FBF3CF) with soft cream clouds, calm and empty. Lower half: a green meadow slope with a dark pine ridge, small yellow wildflowers in the foreground, a tiny figure in a blue jacket sitting on a fallen log, seen from behind. No text, no animals, no buildings." |
| `design/meadow-dusk.png` | belum | D8 empty state versi Senja, M4 versi Senja | "The same meadow painting at dusk, gouache and watercolor texture. Deep blue-green night sky (#1B1F1A to #2E3529) with a few first stars and a thin band of warm light on the horizon. The meadow and pine ridge in deep greens, a tiny figure in a blue jacket on a fallen log with a small warm lantern beside them. Upper 60 percent calm and mostly empty. No text, no animals, no buildings." |

## 8. Komponen

Semua ukuran dalam px. State umum: hover = isi `surface-2`; focus = ring 2px `forest` dengan jarak 2px; disabled = teks `ink-2` tanpa opacity (kontras tetap AA) dan tanpa hover.

| Komponen | Spesifikasi |
|---|---|
| **Title bar** | pengganti frame native (`decorations: false`), fixed di atas selebar jendela, tinggi 36, tanpa isi sehingga langit lukisan terlihat. Seluruh baris adalah area drag; klik ganda memaksimalkan. Tidak ada wordmark karena brand sudah di Sidebar. Kanan: tiga tombol 46 × 36 yang menempel ke tepi jendela (Close tepat di sudut), di satu alas glass dengan radius 10 di sudut kiri bawah (tanpa alas, langit Siang menurunkan ikon ke 1.9:1), ikon 16 `ink` (`square` 14): Minimize, Maximize/Restore (ikon `browsers` saat maximized), Close. Isi hover berupa kotak dalam 34 × 28 radius 8: `surface-2`, dan untuk Close `st-err` dengan ikon `st-err-ink` (Day 5.48:1, Dusk 6.24:1). Focus ring di kotak dalam. Di DOM ditaruh paling akhir agar Tab mencapai isi halaman dulu. Tampil juga di Onboarding. |
| **NavItem** | lebar 208, padding 9/12, gap 10, radius 8. Ikon 18 `ink-2` + label 14/600 `ink-2`. Aktif: isi `surface-2`, ikon `forest`, label `ink` 700. |
| **Sidebar** | 240 × tinggi penuh, border kanan 1px `line`. Urutan: Brand (wordmark "OpenCompanion" 18/800 + ikon `cloud` 20), Nav (Overview, Board, Chat, CLIs, Skills, Settings), grup "Sessions" (baris label 12/700 `ink-2` + tombol ikon `plus` 28 × 28 "New session" di kanan, selalu tampil, juga saat belum ada sesi; lalu sesi yang di-pin ditambah enam sesi lain paling mendesak, dikelompokkan per folder. Folder yang di-pin paling atas (urutan pin, tetap tampil walau kosong dengan teks "No recent sessions" 12 `ink-2`), lalu folder lain menurut sesinya yang paling mendesak; di dalam folder, sesi yang di-pin paling atas. Pin disimpan di aplikasi desktop dan diingat antar restart. Kepala folder adalah tombol lipat (tinggi 30, padding 4/12/4/8, hover atau fokus `surface-2` untuk seluruh baris). Tombol baris 26 × 26 ikon 14 (`plus` "New session in {folder}", `push-pin` "Pin folder"; di item sesi: `trash` untuk sesi selesai, `push-pin` "Pin session") muncul saat hover atau fokus keyboard, menimpa ujung baris dengan latar memudar ke warna baris. Pin yang aktif (`push-pin` terisi, `aria-pressed`) selalu terlihat di kanan dan baris itu memberi ruang kanan 36: `caret-down` 12 `ink-2` (berputar ke kanan saat terlipat) + `folder-simple` 16 `ink-2` + nama folder mono 12/600 `ink`, path lengkap di tooltip. Saat terlipat, kanan kepala menampilkan `hand-palm` `accent` bila ada sesi Waiting for you, lalu jumlah sesi 12 `ink-2`; kepala diberi isi `surface` bila sesi yang dibuka ada di dalamnya. Folder yang dilipat diingat antar restart. Item mini satu baris, menjorok 12 sehingga logo sejajar ikon folder dan judul sejajar nama folder: CliMark `bare` 16 (logo tanpa kotak, `OC` untuk OpenCode) + judul tugas 13/700, padding 7/12, tooltip "judul · CLI · status"; jarak antar folder 6, antar item 2), spacer, Footer: baris Phone access (ikon, label, chip On/Off) dan segmented tema Day/Dusk. Item mini sesi yang sedang dibuka diberi isi `surface`. Judul sesi Running memakai gradasi `--rainbow` yang bergeser (lihat MOTION 1); tiap titik gradasi minimal sekontras `ink-2` terhadap latar (Day ≥ 6.38:1, Dusk ≥ 8.38:1), jadi tetap lulus di atas `glass`. |
| **StatusChip** | padding 3/8, radius 6, label Pixelify 12. Varian dari tabel status di bagian 2. |
| **CliMark** | 30 × 30 (24 × 24 di list padat), radius 6, isi `surface-2`, border 1px `line`. Isi: logo brand selebar 62% kotak (lihat bagian 5), atau monogram IBM Plex Mono 11/600 `ink` bila logo tidak tersedia. |
| **Button Primary** | padding 9/14, gap 8, radius 8, isi `forest`, ikon 16 + label 14/700 `on-forest`. Satu per layar atau per kartu. |
| **Button Secondary** | padding 8/13, isi `surface`, border 1px `line-strong`, ikon dan label `ink`. |
| **Button Danger** | seperti Primary dengan isi `st-err`, teks `st-err-ink`. Untuk Stop. Stop di mobile selalu lewat konfirmasi. |
| **Button Accent** | seperti Primary dengan isi `accent`, teks `on-accent`. Hanya untuk Approve. |
| **Button Ghost** | tanpa isi dan border, label `ink-2`. Untuk Discard. |
| **SessionRow** | padding 14/16, gap 14, radius 10, isi `surface`, border `line`. Kolom: CliMark · Main (judul tugas 15/700, meta "CLI · path mono") · Track 220 (Horizon + "event terakhir · durasi" 12) · StatusCol lebar tetap 124 rata kanan berisi StatusChip. Lebar tetap StatusCol menjaga garis horizon sejajar antar baris. Varian eksternal: isi `bg`, chip Read-only, tanpa Now. |
| **Needs you panel** | padding 20, gap 14, radius 14, isi `surface`, border 1.5px `accent`. Baris atas: CliMark, judul 18/800 ("Claude Code wants to run a command"), meta 13, chip Waiting for you. Blok perintah `term-bg` (label jenis 12 `term-dim`, perintah mono 14 `term-text`). Aksi: Approve (accent), Deny (secondary, ikon `x`), spacer, Open session (secondary). |
| **Terminal** | radius 10, isi `term-bg`, output padding 18/20, baris mono 13 line height 1.45. Prompt pengguna `term-green` diawali "›", output biasa `term-text`, hasil gagal `term-red`, catatan `term-dim`, proses berjalan `term-yellow`. Input bar di bawah: padding 12/14, border atas `#EDE6C433`, isi `#FFFFFF10`, placeholder "Send input to {CLI}", hint "Enter to send · Ctrl+C to interrupt". |
| **Dispatch card** | padding 16, gap 12, radius 10, isi `surface`, border `line`. Atas: CliMark + "CLI · mode" 14/800 + path mono 12. Prompt di kotak `bg` radius 6, mono 13. Baris alasan 13 `ink-2`. Aksi belum jalan: Run "Run in {folder}" (primary, ikon `play`), Edit (secondary), Discard (ghost). Sudah jalan: chip status di kanan atas, Open session + "Started 12 min ago". Tidak valid: alasan dalam teks `st-err` di atas aksi, Run nonaktif. Varian follow-up (pesan untuk sesi yang sudah ada): judul "Follow-up for {judul sesi}", chip idle "Follow-up", aksi utama "Send to session" (ikon `paper-plane-tilt`), Edit hanya berisi "Message for the session", tanpa Add to board; setelah dikirim chip "Sent" lalu status sesi, dan "Sent to this session." |
| **Chat bubble** | pesan Anda rata kanan, lebar maks 560, padding 12/16, radius 14/14/4/14, isi `surface-2`. Pesan planner tanpa bubble, label pixel "Planner" 12 di atasnya, lebar maks 760. |
| **Folder mention** | muncul di atas composer saat `@` diketik: padding 6, radius 10, isi `surface`, border 1px `line-strong`, lebar maks 560, tanpa shadow. Opsi: ikon `folder-simple` 16 `ink-2` + nama 13/700 + path mono 12 `ink-2` (terpotong), tinggi 38 (44 di bawah 720), aktif/hover isi `surface-2`. Opsi terakhir `folder-simple-plus` "Browse for a folder…". Kaki 12 `ink-2`: "Up and Down to choose · Enter writes the path · Esc closes" (disembunyikan di bawah 720). State: "Finding your project folders…", error `st-err`, "No known folder matches …". Fokus tetap di textarea (`aria-activedescendant`). |
| **Composer** | padding 14, radius 10, isi `surface`, border `line-strong`. Placeholder "Describe a task. Name a CLI, type @ for a folder, or let the planner pick one." Baris bawah: pemilih Model dan Thinking tanpa kotak (teks 13/600 `ink-2` + `caret-down` 12 bold di kanan, Thinking diawali ikon `brain` 14; tinggi 28, padding 4/24/4/8, radius 6, lebar mengikuti pilihan sampai maks 240; tinggi 44 di bawah 720; hover isi `surface-2` dan teks `ink`). Label "Model" dan "Thinking" hanya untuk screen reader; tooltip menyebut nama pemilih dan pilihan lengkapnya. Lalu tombol Send ikon saja rata kanan (primary 36 × 36, ikon `paper-plane-tilt` 16 fill, nama "Send" untuk screen reader, tooltip "Send. Shift+Enter adds a line."; 44 × 44 di bawah 720). Tidak ada hint tetap di bawah composer; baris 12 "The planner is still answering in another chat. Send works again when it is done." hanya muncul selama planner masih menjawab di chat lain. Thinking nonaktif (caret disembunyikan) bila model tidak punya tingkat. Saat daftar dimuat: "Listing {CLI} models…"; bila gagal: pesan `st-err` + tombol ghost "List models again". |
| **Input / Select / Textarea** | padding 10/12 (select 9/12), radius 6, isi `surface`, border 1px `line-strong`, teks 14 atau mono 13. Label field 14/800 di atas dengan gap 8. Error: border 2px `st-err` + pesan teks 13 di bawah (bukan warna saja). |
| **Option card** | padding 10/12, radius 8, border 1px `line-strong`. Terpilih: isi `surface-2`, border 2px `forest`, ikon `check-circle` atau `radio-button` `forest`. Tidak tersedia: isi `bg`, teks `ink-2`, keterangan "Not installed". |
| **Switch** | 44 × 26 pill, padding 3. On: isi `forest`, knob 20 `on-forest` di kanan. Off: isi `surface-2`, border `line-strong`, knob `ink-2` di kiri. Selalu disertai label teks. |
| **Modal** | lebar 580, padding 28, gap 20, radius 14, isi `surface`, border `line`, shadow modal, di atas scrim. Tombol tutup 36 × 36 (ikon `x`). Esc menutup, fokus terkunci di dalam modal. |
| **Board column** | kepala: nama 15/800 + jumlah kartu 13/600 `ink-2` + satu baris penjelas 12 `ink-2`, langsung di atas panel `glass`. Badan: isi `surface-2`, radius 10, padding 10, gap 10, scroll vertikal sendiri. Saat drag di atasnya: garis putus 2px `forest`. Kosong: satu kalimat yang bilang cara mengisinya. |
| **Task card** | ringkas: padding 10, gap 6, radius 10, isi `surface`, border `line`. Baris atas: CliMark 24 (bila CLI dipilih), judul 14/700 maksimal 2 baris sebagai tombol edit, lalu satu ikon aksi 28 × 28 di kanan atas: `play` warna `forest` (Run, hanya Todo) atau `arrow-square-out` (Open session, bila ada sesi). Baris bawah 12 `ink-2`, ikon 14: `folder-simple` + project mono, lalu satu info: `note` (ada catatan, Pending/Todo), status In progress (`spinner-gap` `forest` Running, `hand-palm` `accent` Waiting for you, `warning-circle` `st-err` Error) + waktu, atau `check-circle` `forest` + durasi di Done. Tanpa chip teks, pratinjau catatan, atau horizon; teks lengkap ada di dialog kartu. Setiap ikon punya tooltip dan label tersembunyi untuk screen reader. Kartu Waiting for you: border 1.5px `accent`. Pindah kolom: drag, atau ganti Column di dialog kartu (jalur keyboard). Di bawah 720, ikon aksi 40 × 40. |
| **Table (CLIs)** | radius 10, isi `surface`, header isi `surface-2` label 13/700 `ink-2`. Baris padding 14/16, border atas `line`. |
| **Horizon** | lihat bagian 6. |

## 9. Layar desktop

Status di file `design/ai-remote.pen`: ✓ = sudah ada, ✗ = belum dibuat.

### D1 Onboarding ✓
- Tujuan: menunjukkan CLI yang ditemukan dan memilih planner chat. Fokus: daftar CLI.
- Latar: `meadow-day.png` penuh. Panel 540 lebar di kiri atas (x 112, y 84), isi `#FFFAE6F2`, radius 14, padding 32, gap 22.
- Isi: brand kecil; H1 "These coding CLIs are on your computer"; body "OpenCompanion looked through your PATH. Nothing was installed or changed."; daftar 4 baris (CliMark, nama, path mono, versi mono, ikon `check-circle`; Gemini CLI dengan chip "Not found" dan teks "You can add it later from CLIs"); label "Planner for Chat" + select "Claude Code 2.1.282" + bantuan "Chat runs this CLI headless to turn your requests into session suggestions. You can change it later in Settings."; tombol "Continue with Claude Code" dan "Rescan".
- Loading: baris CLI tampil satu per satu dengan teks "Checking claude…", "Checking codex…".
- Error: jika tidak ada CLI sama sekali, daftar diganti teks "No supported CLI found on PATH." + perintah instal (seperti baris Gemini di D5).

### D2 Overview ✓
- Tujuan: tahu sesi mana yang butuh Anda. Fokus: panel Needs you (hanya tampil bila ada sesi Waiting for you; jika lebih dari satu, panel menumpuk dengan yang paling lama di atas).
- Header: H1 "Overview", ringkasan "4 sessions in OpenCompanion, 1 opened outside", tombol "New session".
- Section "Running in OpenCompanion" (hitungan "2 running, 1 done today") berisi SessionRow; urutan: running, lalu done hari ini.
- Section "Opened outside OpenCompanion" berisi SessionRow varian eksternal + hint "Found in the running process list. You can read its transcript here, but only the terminal it was opened in can send it input."
- Perbaikan yang perlu dilakukan di file: pada baris `~/Project/psikotes` chip harus "Done" (isi `st-idle`), pada baris `~/Project/sepulangkerja.id` chip harus "Read-only" (outline). Override ini hilang saat chip dipindah ke StatusCol, jadi sekarang tampil "Running". Hal yang sama berlaku di latar D6.

### D3 Chat ✓
- Tujuan: menyusun tugas menjadi sesi. Fokus: kartu dispatch.
- Header: tanpa judul dan baris planner yang terlihat (H1 "Chat" hanya untuk screen reader), isinya hanya tombol aksi, rata kanan. Chat baru tidak memakai pesan pembuka planner; placeholder composer yang menjelaskan cara menulis tugas. Bila planner belum bisa dipakai, Send nonaktif dan baris 12 di bawah composer menyebut sebabnya: "The custom provider has no base URL or model yet. Add them in Settings." (custom provider tanpa URL atau model, FR-29), "Checking which CLI can plan…", atau "No CLI that can plan is installed."
- Thread: pesan Anda, pesan planner "Two separate folders, so two sessions. Check the prompts, then press Run on each one.", dua Dispatch card (satu sudah jalan, satu menunggu Run).
- Composer di bawah, dengan pemilih Model dan Thinking untuk CLI planner (FR-28); opsi pertama selalu "CLI default", model OpenCode dikelompokkan per provider. Live rail kanan 300 lebar (isi `surface`, border kiri) dengan kartu sesi kecil: CliMark 24, nama folder, chip, Horizon. Tombol ikon `sidebar-simple` (dicerminkan, 36 × 36, paling kanan di header; terisi saat rail terbuka, `aria-expanded`, tooltip "Hide live sessions"/"Show live sessions") menyembunyikan dan menampilkan rail. Mulai 1280 rail menjadi kolom dan pilihan sembunyi diingat antar restart; di bawah 1280 rail tertutup dan tombol membukanya sebagai panel 300 di bawah tombol, di atas percakapan (`shadow-modal`, tinggi maks 480), tertutup lagi lewat tombol atau Escape.
- Panel "Chats" kiri 240 lebar (208 di bawah 900), panel solid `surface` radius 14 seperti Live rail: judul 14/800 + tombol "New chat" (secondary sm, ikon `plus`), lalu daftar chat berupa link (judul 13/700 satu baris, "12 min ago" 12 `ink-2`, tinggi minimal 44). Chat yang dibuka diberi isi `surface-2`, seperti NavItem aktif. Header thread mendapat tombol ghost "Delete chat" (ikon `trash`) dengan tekanan kedua "Press again to delete". Di bawah 720 daftar dilipat menjadi satu tombol (judul chat, "{n} saved", ikon `caret-down`) di atas percakapan. Chat yang sudah dihapus menampilkan state "This chat is gone" + "Start a new chat".
- Loading: pesan planner diganti baris pixel "Planner" + teks "Thinking with Claude Code…" dan tombol Stop kecil.
- Error: "The planner could not answer: {pesan dari CLI}." + tombol "Try again"; bila output bukan JSON valid, jawaban tampil sebagai teks biasa tanpa kartu.

### D4 Session detail ✓
- Tujuan: melihat dan mengendalikan satu sesi. Fokus: terminal.
- Header: breadcrumb "Overview / ai-remote", CliMark + judul 24/800 + chip, meta "Codex CLI 0.153.4 · interactive · ~/Project/ai-remote · started 12 min ago", tombol Restart dan Stop.
- Baris Activity: label pixel + Horizon + "Commands, file edits and approvals in the last 12 min".
- Body: terminal (lebar sisa) + kolom kanan 300: "Files changed" (path mono + diff mono), "Process" (PID, CPU, Memory, Child processes; diukur tiap 3 detik selama sesi berjalan, "Measuring CPU and memory…" sebelum hasil pertama), "Needs-you signal" (metode deteksi: event stream, hooks, atau pola teks, sesuai FR-16).
- State Waiting for you: panel Needs you versi ringkas muncul di atas terminal, input bar dinonaktifkan dengan teks "Answer the approval above first".
- State eksternal (`/outside?pid=`, dibuka dari baris "Opened outside OpenCompanion" di Overview): tombol Stop dan Restart tidak ada, judul "{CLI} in {folder}" (atau nama CLI saja bila folder tidak terbaca) dengan chip Read-only, meta "Opened outside OpenCompanion · {mode} · path · started 09:12". Panel terminal berisi transcript dari riwayat CLI itu sendiri (pesan Anda `term-green` diawali "›", jawaban CLI `term-text`, tool `term-dim` diawali "•"), dibaca ulang tiap 5 detik dan hanya mengikuti baris baru bila pembaca sudah di ujung; tanpa transcript tampil alasannya dalam `term-dim`. Input bar diganti "This session was opened outside OpenCompanion. Showing its transcript, read-only." Kolom kanan: "Process" (PID, Running for, Memory, CPU dari scan berkala), "Transcript" (path file mono + "Last entry 3 min ago"), "Input" (hanya terminal asalnya yang bisa menjawab). Proses yang sudah berhenti: "This session has ended" + "Back to Overview".

### D5 CLIs ✓
- Tujuan: tahu CLI apa yang siap dipakai. Fokus: tabel.
- Header: H1 "CLIs", sub "Found on your PATH. Versions come from each CLI's own --version output.", tombol "Add from path" dan "Rescan".
- Kolom tabel (lebar): CLI 330 (CliMark, nama, path mono) · Version 110 · Headless command 300 (mono) · Adapter 150 ("Tested" dengan ikon atau chip "Untested version") · aksi (teks "Chat planner" `forest` atau tombol "Use as planner").
- Baris Gemini CLI (isi `bg`): "Not found on PATH" + kotak perintah `npm install -g @google/gemini-cli` + "Copy command".
- Catatan: "OpenCompanion never installs a CLI for you. Copy the command and run it in your own terminal, then press Rescan. Aider, Qwen Code and other CLIs are planned after the first release."

### D6 New session ✓
- Modal di atas layar mana pun (satu dialog dirender oleh shell). Dibuka dari tombol New session di Overview, tombol `plus` di label Sessions (folder kosong, dipilih sendiri), atau tombol `plus` di kepala folder sidebar (folder sudah terisi). Fokus: tombol Start.
- Field: CLI (option card 2 × 2, Gemini tidak tersedia), Project folder (input mono + Browse + chip "Recent"), Mode (Interactive terpilih / Headless dengan penjelasan), First message (optional).
- Footer: "Esc to close", Cancel, "Start Claude Code in uninote" (label berubah mengikuti CLI dan folder).
- Validasi: folder tidak ada = border error + "This folder does not exist." dan Start nonaktif.

### D7 Settings › Phone access ✗
- Sidebar: Settings aktif; chip Phone access "On" (`st-run`).
- H1 "Settings". Fokus: kartu Phone access (padding 24, radius 14, isi `surface`, border `line`).
  - Baris atas: judul "Phone access" 18/800, deskripsi "Lets a phone on the same network watch sessions and answer approval prompts. Off until you turn it on.", Switch On.
  - Kiri (220): kotak QR 180 × 180 (isi `#FFFFFF`, border `line`, QR asli dibuat runtime), "Pairing code", kode mono 20/700 contoh `482 913`, "Expires in 4:32", tombol "New code".
  - Kanan: "Address on this network" + kotak mono contoh `http://192.168.1.24:8765` + tombol copy; teks "On your phone, join the same Wi-Fi, open the camera and point it at the code. The phone asks for a name, then shows your sessions."; kotak peringatan (`surface-2`, ikon `warning`): "This connection uses plain HTTP. On a network you do not trust, or away from home, reach your computer through a VPN with HTTPS, such as Tailscale."
  - "Paired devices": baris ikon `device-mobile`, nama perangkat, "Last seen …", tombol "Remove". Kosong: "No phones paired yet."
- Di bawahnya dua kartu berdampingan: "Chat planner" (select CLI, opsi terakhir "Custom provider (OpenAI-compatible API)" yang membuka field Base URL, Model, API key dengan tombol Show/Hide, lalu "Save provider"; error per field memakai pola Input; centang baca folder nonaktif untuk provider; lalu field "Run cards without asking" dengan daftar folder (path mono + Remove), select + Add + Browse…, dan kotak peringatan border `st-err` "Let cards for {folder} start without Run?" dengan tombol danger "Start them without asking" dan secondary "Keep asking") dan "Notifications" (tiga pilihan centang: Waiting for you, Done, Error; lalu tabel ringkas "By CLI" dengan baris per CLI terpasang dan kolom centang Waiting, Done, Error, tabel "By project folder" dengan nama folder mono, tiga centang dan tombol ghost Remove, select "Choose a project folder…" + Add + Browse…, dan satu baris 13 `ink-2` "A notification goes out only when the switch above, its CLI and its folder all allow it. The phone follows the same rules.") Kartu "Window and sign-in": centang "Keep running in the tray" (default aktif) dengan satu baris 13 `ink-2` yang menjelaskan akibat pilihan itu, lalu (hanya di Windows) centang "Start in the tray when I sign in to Windows". Kartu "History": lokasi data + jumlah sesi, select "Keep finished sessions for" (Forever, 90 days, 30 days, 7 days, 1 day; default Forever) dengan satu baris 13 `ink-2` "Older ones go with their events and terminal logs, checked every hour. Files the CLIs changed in your projects are never touched.", lalu tombol secondary "Delete finished sessions" dengan tekanan kedua "Press again to delete finished sessions".
- State server gagal jalan: kartu menampilkan "Port 8765 is in use by another app." + tombol "Use another port".

### D8 States ✗
Satu frame berisi tiga contoh berdampingan:
- **Empty Overview**: `meadow-day.png` sebagai pita atas, lalu "No sessions yet", teks "Start one from Chat or press New session. CLIs you open in a terminal show up here too.", tombol "Open chat" (secondary) dan "New session" (primary).
- **Loading**: judul "Looking for coding CLIs", daftar per CLI dengan status "Checking claude…" (spinner kecil + teks), "Found codex 0.153.4", "gemini not found". Tanpa skeleton.
- **Error sesi**: chip Error, judul "Codex CLI stopped", meta "exited with code 1 after 40 s", blok terminal berisi output stderr asli dari CLI, teks "OpenCompanion shows the CLI's own message. If it asks you to sign in, run the sign-in command in your own terminal.", tombol "Restart session".

### D10 Board ✗ (ada di prototipe OpenDesign)
- Tujuan: mengatur tugas dan mengirimnya ke CLI. Fokus: kolom In progress, terutama kartu yang menunggu Anda.
- Header: H1 "Board", sub "Tasks for your coding CLIs. Press Run on a Todo card to start a session; the card follows the session and moves to Done when it finishes.", filter project, tombol "New card".
- Empat kolom sama lebar (minimal 240): Pending "Not ready yet", Todo "Ready to hand to a CLI", In progress "A CLI session is on it", Done "Finished". Di bawah 1100 lebar kolom tetap 260 dan board bergeser horizontal di dalam kontainernya sendiri; di bawah 720 kolom bertumpuk.
- Dialog kartu: Title (wajib), Notes (dipakai sebagai prompt), Project, CLI ("Decide later"), Column; Delete dua langkah ("Press again to delete").
- Dialog Run: ringkasan kartu + folder, pilihan CLI, Mode, Prompt terisi dari catatan, tombol "Start {CLI} in {project}".
- Empty per kolom: Pending "Nothing pending. New cards start here.", Todo "No ready tasks. Move a pending card here once it is clear enough to run.", In progress "Nothing in progress. Press Run on a Todo card.", Done "Finished cards land here."

### D11 Skills ✗ (hanya di Svelte)
- Tujuan: tahu skill mana yang hilang atau berbeda isi antar-CLI. Fokus: matriks.
- Header: H1 "Skills", sub "Skill folders in your home directory, compared by content. OpenCompanion reads these folders and never changes them.", tombol "Rescan".
- Toolbar di alas glass (teks `ink`): ringkasan yang dihitung dari hasil pindaian (bentuknya, angka contoh: "95 skills in 5 folders · 3 differ · 13 have a problem"), segmented All / Different / Problems dengan jumlah, input cari dengan ikon `magnifying-glass`.
- Matriks memakai Table (CLIs), lebar minimal 900 dan bergeser horizontal di kontainernya sendiri. Kolom pertama: nama skill 14/700 sebagai tombol yang membuka dialog, deskripsi satu baris 12 `ink-2`, "2 versions" bila isinya berbeda. Satu kolom per folder: CliMark 24 (Shared memakai `folder-simple` di kotak CliMark) + label, path lengkap hanya di tooltip; folder yang tidak ada diberi chip idle "Folder not found".
- Sel: `check-circle` `forest` + "Same" (atau "Installed" bila hanya satu salinan), chip Read-only "Version A" + "3 h ago" 12 `ink-2` bila isinya berbeda (A selalu yang terbaru), "Missing" 13 `ink-2`, atau chip Error ("No SKILL.md", "Broken link", "Unreadable", "Too large").
- Dialog lebar 720: nama skill, deskripsi, daftar folder dengan status, select "Copy from", lalu satu langkah per folder: "Add to Codex CLI" atau "Replace in Shared" (dengan peringatan file yang hanya ada di salinan itu terhapus) berisi blok `.cmd` berlabel "PowerShell" + "Copy command"; folder berupa link hanya mendapat "This folder links to … Update it there." tanpa perintah, dan folder yang berisi link di dalamnya mendapat penjelasan untuk mengganti secara manual, juga tanpa perintah (Remove-Item di PowerShell 5.1 mengikuti junction).
- State: loading "Reading skill folders…", error "Skill folders could not be read" + "Try again", kosong "No skills in these folders yet" + daftar lima path, filter kosong "No skill differs between folders." / "No folder has a problem." / `No skill matches "…".`

### D12 All sessions ✗ (hanya di Svelte)
- Tujuan: menemukan sesi lama. Fokus: daftar hasil.
- Dibuka dari tautan "All sessions" di kepala section "Running in OpenCompanion" di Overview (dan dari teks bantu saat tidak ada yang berjalan); tidak ada item nav baru.
- Header: H1 "All sessions", sub "Every session OpenCompanion ran, newest first. Finished ones stay until you delete them or retention in Settings removes them."
- Toolbar di alas glass: input cari dengan ikon `magnifying-glass` ("Search titles and prompts"), select "Every CLI", "Every folder" (nama folder + path pendek), "Any status". Pencarian menunggu 200 ms setelah ketikan atau pilihan terakhir; hasil lama tetap tampil selama mencari.
- Hasil: SessionRow seperti Overview, 50 per halaman, tombol secondary "Show 50 more". Jumlah hasil diumumkan lewat status tersembunyi.
- State: "Looking through your sessions…", error + "Try again", kosong "Nothing has run in OpenCompanion yet. Sessions you start show up here.", filter kosong "No session matches this search. Clear a filter or try other words."

### D9 Tema Senja ✗
- Salinan D2 dan D4 dengan tema `mode: dark` (semua token berganti otomatis).
- Segmented tema di sidebar: Dusk aktif (isi `surface`, ikon `moon` `ink`), Day diam.
- Terminal memakai `term-bg` Senja `#121710`. Empty state memakai `meadow-dusk.png`.

## 10. Layar mobile (web companion)

Bottom tab bar di tiga layar utama (Sessions, Chat, Board): tinggi 56 + safe area, isi `bg`, border atas `line`; tiap tab ikon 22 (`house`, `chats-circle`, `kanban`; terisi dan `forest` saat aktif) + label 12/700, `aria-current="page"`. Tab Sessions membawa jumlah sesi Waiting for you (isi `accent`, teks `on-accent` 11, tinggi 18). Layar detail dan form (M1, M3, M5, percakapan Chat) tidak menampilkan tab bar karena punya bar aksi bawah sendiri. Semua target sentuh minimal 44, tombol aksi utama 48, jarak antar tombol minimal 8; field teks 16 agar browser HP tidak memperbesar halaman.

### M1 Pair device ✗
- Atas: `meadow-portrait.png` setinggi 280 (radius bawah 14). Di bawahnya di atas `bg`: H1 22/800 "Pair this phone", teks "On your computer, open OpenCompanion, go to Settings, turn on Phone access and scan the code shown there."
- Tombol "Scan pairing code" (primary, ikon `camera`, lebar penuh, 48), tombol "Enter the 6-digit code" (secondary, lebar penuh).
- Mode kode: 6 kotak input mono 44 × 52. Error: "That code has expired. Make a new one on your computer."
- Catatan bawah 12: "This phone talks only to your computer, over your own network."

### M2 Sessions ✗
- Bar atas 56: wordmark 18/800 + chip "Connected" (`st-run`); baris kecil "To your desktop · 192.168.1.24".
- Header: H1 "Sessions" + tombol secondary "New session" (ikon `plus`) menuju M5.
- Fokus: kartu Needs you (versi mobile: judul 16/800, meta, blok perintah mono 13 yang boleh membungkus baris, Approve dan Deny berdampingan lebar sama, tinggi 48).
- Section "Running" dan "Done today": kartu sesi (padding 14, CliMark 28, judul tugas 15/700 maksimal 2 baris, "CLI · folder", chip, Horizon lebar penuh). Seluruh kartu bisa diketuk menuju M3.
- Empty: "Nothing is running on your computer right now."

### M3 Session detail ✗
- Bar atas: tombol kembali "Sessions" (ikon `caret-left`, target 44), chip status di kanan.
- Judul 20/800, meta "Claude Code · ~/Project/uninote · 14 min".
- Activity: Horizon lebar penuh.
- "Latest output": blok terminal, 12 baris terakhir, mono 12, membungkus baris (tidak ada scroll horizontal).
- Bar aksi bawah tetap (dengan safe area; tingginya diukur dan konten diberi padding bawah setinggi bar): saat Waiting for you yang bisa dijawab = Approve (accent) + Deny; terminal berjalan = baris tombol Enter, Esc, ↑, ↓, Ctrl+C (mono 12, tinggi 44) + field teks dan Send (teks diketik, lalu Enter dikirim terpisah); terminal tertutup = "Resume terminal"; headless = textarea follow-up + Send dengan aturan yang sama seperti desktop, dan alasannya bila belum bisa.
- Stop (danger) di kanan bar atas selama sesi berjalan, membuka sheet konfirmasi "Stop Claude Code in uninote? It gets an interrupt first, then a forced stop if it does not exit." dengan "Stop session" dan "Keep running".

### M5 New session ✗
- Bar atas: kembali "Sessions" (atau "Board" saat menjalankan kartu, `?task=`). H1 "New session" atau judul kartu.
- Form satu kolom: CLI (option card; yang belum terpasang nonaktif), Project folder (select dari folder yang ditemukan desktop, terbaru dulu, plus "Another folder…" yang membuka field path mono), Mode (Headless terpilih karena langkahnya terbaca di HP dan menerima follow-up; Interactive), Prompt, Permission mode (default dari Settings, peringatan Bypass sama dengan desktop). Tombol primary lebar penuh "Start {CLI} in {folder}" atau "Run in {folder}".
- State: "Checking the CLIs and project folders on your computer…", error + "Try again", folder yang tidak ada ditandai di field folder.

### M6 Chats dan percakapan ✗
- Chats: H1 "Chat" + "New chat"; baris planner "Planner: {nama}. It answers with a card for each session, and nothing starts until you press Run."; daftar chat (judul 15/700, waktu 12 `ink-2`, tinggi minimal 56). Kosong: "No chats yet. Your first message starts one, and every chat keeps its own history on your computer."
- Percakapan: kembali "Chats", judul 20/800, pesan Anda (isi `surface-2`, radius 14/14/4/14, rata kanan), jawaban dengan label pixel "Planner" dan kartu dispatch versi HP: "Run in {folder}" lebar penuh, "Add to board", "Discard", "Undo". Edit kartu tetap di desktop. Composer di bar bawah: textarea + Send, catatan "Nothing starts until you press Run on a card." Selama planner menjawab: spinner + "The planner is reading your message. This usually takes 10 to 30 seconds."

### M7 Board ✗
- H1 "Board", segmented empat kolom (Pending, Todo, In progress, Done + jumlah; tinggi 48), satu kolom tampil sekaligus dan pilihannya disimpan di alamat (`?col=`). Kartu: CliMark, judul 15/700, chip sesi, "folder · CLI"; kartu yang sesinya Waiting for you diberi border `accent`.
- Ketuk kartu membuka sheet: judul, kolom, path, catatan, Approve/Deny bila sesinya menunggu (FR-77), "Run in {folder}" (Pending atau Todo tanpa sesi berjalan, menuju M5), "Open session", "Move to" tiga kolom lain, "Close". Kartu baru dan edit tetap di desktop.

### M4 Can't reach your desktop ✗
- Gema langsung dari gambar referensi "No Internet / Try:".
- Atas: `meadow-portrait.png` setinggi 360. Di bawahnya: H1 "Can't reach your desktop", label "Try:" 14/800, tiga butir 14: "Wake your computer and open OpenCompanion", "Join the same Wi-Fi as your computer", "Check that Phone access is still on in Settings".
- Tombol "Try again" (primary, lebar penuh, 48), teks kecil "Last connected 18 min ago".
- Versi Senja memakai `meadow-dusk.png`.

## 11. Gerak

- Hover dan focus: transisi warna 120 ms ease-out.
- Perubahan status chip: crossfade 200 ms. Baris yang pindah ke panel Needs you: geser 200 ms, tanpa bounce.
- Modal: fade scrim 150 ms + modal naik 8px dalam 180 ms.
- Tidak ada animasi berulang. Spinner hanya di teks loading yang menjelaskan apa yang dimuat.
- `prefers-reduced-motion`: semua transisi menjadi instan.

## 12. Aksesibilitas

- Kontras mengikuti tabel bagian 2. Pasangan baru dicek dulu.
- Status tidak pernah warna saja: chip selalu berlabel. Item sesi di sidebar hanya menampilkan logo CLI dan judul; CLI dan status tetap dibacakan screen reader dan ada di tooltip.
- Semua kontrol bisa dicapai dengan Tab sesuai urutan visual; Enter/Space mengaktifkan; Esc menutup modal dan sheet.
- Focus ring 2px `forest` + jarak 2px di kedua tema.
- Aksi yang menghapus tombolnya sendiri (Approve, Run, Discard, dialog yang pembukanya sudah hilang) memindah fokus ke kontrol pertama yang menggantikannya, atau ke area utama halaman; fokus tidak pernah jatuh ke awal dokumen. Hitung mundur kode pairing tidak dibacakan tiap detik; kedaluwarsanya diumumkan sekali.
- Terminal: xterm dengan mode screen reader aktif; ringkasan status sesi juga diumumkan lewat live region ("Codex CLI in ai-remote is waiting for you"). Selama CLI berjalan, Tab milik CLI, jadi Ctrl+Tab dan Ctrl+Shift+Tab memindah fokus keluar dari terminal (disebut di catatan bawah terminal); terminal yang sudah tertutup tidak menangkap Tab sama sekali.
- Mobile: teks bisa diperbesar 200% tanpa terpotong; input fokus tidak tertutup keyboard.
- Desktop: Settings › Text size (90, 100, 110, 125, 150%) memakai zoom webview, sehingga skala di bagian 3 dan spasi di bagian 4 membesar bersama. Di jendela sempit layout turun ke breakpoint 1100 dan 720 yang sudah ada.

## 13. Bahasa dan suara

- UI bahasa Inggris untuk MVP. i18n ID/EN direncanakan (PRD FR-63).
- Sebut CLI dan folder secara spesifik: "Codex CLI is waiting for you in ai-remote", bukan "Something needs your attention".
- Tombol menyebut aksinya: "Start Claude Code in uninote", "Run in psikotes", "Copy command".
- Tanpa em dash, tanpa buzzword, tanpa emoji dekoratif.
- Data contoh di mockup: nama CLI, versi, path executable, dan nama folder project diambil dari mesin pengembang pada 2026-09-25. Judul tugas, durasi, PID, CPU, memori, kode pairing, alamat IP, dan nama perangkat adalah contoh untuk layout; di aplikasi semuanya berasal dari data nyata.

## 14. File desain

- `design/ai-remote.pen`: variabel (32 token, tema `mode` light/dark), komponen (NavItem, StatusChip, CliMark, Button Primary/Secondary/Danger/Accent, Horizon, SessionRow, Sidebar), layar D1 sampai D6.
- `design/meadow-day.png`: ilustrasi siang (dirujuk oleh D1).
- Yang tersisa untuk dibangun di Pencil: D7, D8, D9, M1 sampai M4, dua ilustrasi (`meadow-portrait.png`, `meadow-dusk.png`), dan perbaikan chip di D2/D6 (bagian 9).
- Prototipe HTML yang bisa diklik (semua layar D1 sampai D9 dan M1 sampai M4) ada di project OpenDesign "AI Remote": `index.html` (Overview + New session), `board.html` (Board), `session.html?s={folder}`, `chat.html`, `clis.html`, `settings.html`, `onboarding.html`, `states.html`, `phone.html` (web companion, rute `#pair`, `#sessions`, `#session/{folder}`, `#offline`), dan `mobile.html` (empat bingkai HP). Token warna ada di `styles/app.css`, data contoh sesi ada di `scripts/data.js`. Tema Senja dibuka lewat toggle Day/Dusk di sidebar.

## 15. Keputusan (satu baris per keputusan)

- Krem + zaitun: diambil langsung dari langit dan teks referensi, memberi rasa hangat yang membedakan dari tool developer gelap pada umumnya.
- Hijau hutan sebagai primer: warna paling gelap dan stabil di lukisan, cukup kontras untuk tombol.
- Biru jaket sebagai satu-satunya aksen: di lukisan pendaki adalah titik fokus; di aplikasi momen fokusnya adalah sesi yang menunggu Anda.
- Terang sebagai default + Senja: referensinya siang hari; Senja ada karena developer sering kerja malam, dan toggle wajib berfungsi di dua tema.
- Terminal selalu gelap: output CLI dan warna ANSI butuh latar gelap agar terbaca.
- Nunito: terminal hurufnya membulat seperti judul referensi, tetap jelas di ukuran 13.
- Pixelify Sans hanya untuk label kecil: satu gema pixel dari garis tanah referensi, cukup satu tempat agar tidak jadi gimmick.
- Sidebar + area kerja: aplikasi dipakai berjam-jam dengan lima tujuan utama, sidebar menjaga semuanya satu klik.
- Baris sesi, bukan kartu grid: sesi dibandingkan satu sama lain (status, durasi, aktivitas), dan baris dengan kolom sejajar lebih mudah dipindai.
- Logo brand di CliMark: pemilik produk memintanya, dan logo membuat CLI langsung dikenali di daftar sesi. Monogram hanya cadangan saat logo belum ada.
- Ilustrasi hanya di momen tanpa data: di situ tidak ada yang perlu dibaca, jadi lukisan membawa identitas tanpa mengganggu kerja.
- Skills sebagai matriks folder × skill, bukan daftar per CLI: tujuan layar adalah membandingkan, dan kolom sejajar membuat celah langsung terlihat. Kolomnya folder karena CLI mana membaca folder mana bergantung versi CLI.
- Skills hanya memberi perintah salin, tidak menyalin sendiri: sama dengan aturan instal CLI, OpenCompanion tidak menulis ke folder konfigurasi CLI.
- Mobile dengan bottom tab bar (Sessions, Chat, Board): owner meminta Chat, Board dan New session di HP (2026-09-25), jadi ada tiga tujuan setara yang harus satu ketukan; layar detail menyembunyikannya karena memakai tepi bawah untuk bar aksi.
