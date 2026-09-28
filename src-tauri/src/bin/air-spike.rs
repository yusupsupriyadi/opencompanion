//! M0 spike runner: exercises the Rust core from a terminal, without the webview.
//!
//!   air-spike detect
//!   air-spike scan
//!   air-spike pty <cli> --cwd DIR --out DIR [--secs N] [--arg A]... [--at SEC=KEYS]...
//!   air-spike headless <cli> --cwd DIR --prompt TEXT --out FILE [--answer allow|deny]
//!                      [--timeout N] [--extra FLAG]...
//!   air-spike replay <cli> FILE      (re-parses a file written by `headless`)
//!   air-spike waiting <cli> FILE...  (runs prompt detection on saved screens)
//!   air-spike plan --message TEXT [--cli claude] [--said EARLIER]...  (one planner turn)
//!
//! KEYS understands \r, \n, \t, \e (Escape), \c (Ctrl+C) and \\.

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use opencompanion_lib::cli::{self, CliKind};
use opencompanion_lib::events;
use opencompanion_lib::headless::{self, HeadlessRun, Stream};
use opencompanion_lib::monitor::Monitor;
use opencompanion_lib::pty::{PtySession, PtySpec};
use opencompanion_lib::waiting;

struct Opts {
    positional: Vec<String>,
    flags: Vec<(String, String)>,
}

impl Opts {
    fn parse(args: impl Iterator<Item = String>) -> Self {
        let mut positional = Vec::new();
        let mut flags = Vec::new();
        let mut it = args.peekable();
        while let Some(a) = it.next() {
            if let Some(name) = a.strip_prefix("--") {
                flags.push((name.to_string(), it.next().unwrap_or_default()));
            } else {
                positional.push(a);
            }
        }
        Self { positional, flags }
    }
    fn get(&self, name: &str) -> Option<&str> {
        self.flags.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }
    fn all(&self, name: &str) -> Vec<&str> {
        self.flags.iter().filter(|(k, _)| k == name).map(|(_, v)| v.as_str()).collect()
    }
    fn need(&self, name: &str) -> String {
        self.get(name)
            .unwrap_or_else(|| die(&format!("missing --{name}")))
            .to_string()
    }
}

fn die(msg: &str) -> ! {
    eprintln!("air-spike: {msg}");
    std::process::exit(2);
}

fn keys(spec: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut chars = spec.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            let mut b = [0u8; 4];
            out.extend_from_slice(c.encode_utf8(&mut b).as_bytes());
            continue;
        }
        match chars.next() {
            Some('r') => out.push(b'\r'),
            Some('n') => out.push(b'\n'),
            Some('t') => out.push(b'\t'),
            Some('e') => out.push(0x1b),
            Some('c') => out.push(0x03),
            Some('\\') => out.push(b'\\'),
            Some(other) => out.extend_from_slice(format!("\\{other}").as_bytes()),
            None => out.push(b'\\'),
        }
    }
    out
}

fn kind_arg(opts: &Opts) -> (CliKind, PathBuf) {
    let name = opts.positional.get(1).unwrap_or_else(|| die("missing <cli>"));
    let kind = CliKind::from_bin(name).unwrap_or_else(|| die(&format!("unknown cli {name}")));
    let path = cli::resolve(kind).unwrap_or_else(|| die(&format!("{name} not found")));
    (kind, path)
}

fn cmd_pty(opts: &Opts) {
    let (kind, program) = kind_arg(opts);
    let cwd = PathBuf::from(opts.need("cwd"));
    let out = PathBuf::from(opts.need("out"));
    fs::create_dir_all(&out).expect("create out dir");
    let secs: u64 = opts.get("secs").and_then(|s| s.parse().ok()).unwrap_or(10);
    let mut steps: Vec<(f64, Vec<u8>)> = opts
        .all("at")
        .iter()
        .filter_map(|s| s.split_once('='))
        .filter_map(|(t, k)| Some((t.parse().ok()?, keys(k))))
        .collect();
    steps.sort_by(|a, b| a.0.total_cmp(&b.0));

    let raw_path = out.join(format!("{}-raw.bin", kind.bin()));
    let mut raw = fs::File::create(&raw_path).expect("raw file");
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let spec = PtySpec {
        program: program.clone(),
        args: opts.all("arg").iter().map(|s| s.to_string()).collect(),
        cwd,
        cols: 120,
        rows: 36,
        env: vec![],
        powershell: None,
        stay: None,
    };
    let started = Instant::now();
    let mut session = PtySession::spawn(spec, Box::new(move |b| {
        let _ = tx.send(b.to_vec());
    }))
    .unwrap_or_else(|e| die(&e));
    println!("spawned {} pid={:?} via {}", kind.bin(), session.pid(), program.display());

    let mut last_screen = String::new();
    let mut shot = 0;
    let mut bytes = 0usize;
    let mut first_byte_ms = None;
    while started.elapsed() < Duration::from_secs(secs) {
        while let Ok(chunk) = rx.try_recv() {
            first_byte_ms.get_or_insert(started.elapsed().as_millis());
            bytes += chunk.len();
            let _ = raw.write_all(&chunk);
        }
        let t = started.elapsed().as_secs_f64();
        while steps.first().is_some_and(|(at, _)| *at <= t) {
            let (at, k) = steps.remove(0);
            println!("t={t:.1}s send {:?} (scheduled {at}s)", String::from_utf8_lossy(&k));
            let _ = session.write(&k);
        }
        let screen = session.screen_text();
        if screen != last_screen && !screen.trim().is_empty() {
            shot += 1;
            let file = out.join(format!("{}-screen-{shot:02}.txt", kind.bin()));
            let _ = fs::write(&file, format!("t={t:.1}s\n{screen}"));
            last_screen = screen;
        }
        if let Some(code) = session.exit_code() {
            println!("t={t:.1}s exited on its own, code {code}");
            break;
        }
        thread::sleep(Duration::from_millis(250));
    }
    let resize_ok = session.resize(100, 30).is_ok();
    let stop_code = session.stop(Duration::from_secs(4));
    println!(
        "bytes={bytes} first_output_ms={first_byte_ms:?} screens={shot} resize_ok={resize_ok} \
         stop={} output_closed={}",
        match stop_code {
            Some(c) => format!("exited code {c} after Ctrl+C"),
            None => "killed after grace".into(),
        },
        session.output_closed()
    );
}

fn cmd_headless(opts: &Opts) {
    let (kind, program) = kind_arg(opts);
    let cwd = PathBuf::from(opts.need("cwd"));
    let prompt = opts.need("prompt");
    let out = PathBuf::from(opts.need("out"));
    let answer_allow = opts.get("answer").unwrap_or("allow") == "allow";
    let timeout = Duration::from_secs(opts.get("timeout").and_then(|s| s.parse().ok()).unwrap_or(240));
    let mut inv = headless::invocation(kind, &cwd, &prompt, &headless::TurnOptions::default())
        .unwrap_or_else(|| die("no headless adapter for this cli yet"));
    // Extra flags go before the last argument, which is the prompt or `-`.
    for extra in opts.all("extra") {
        let at = inv.args.len().saturating_sub(1);
        inv.args.insert(at, extra.to_string());
    }
    println!("{} {}", program.display(), inv.args.join(" "));

    let (tx, rx) = mpsc::channel::<(Stream, String)>();
    let started = Instant::now();
    let mut run = HeadlessRun::spawn(&program, &cwd, inv, Box::new(move |s, l| {
        let _ = tx.send((s, l.to_string()));
    }))
    .unwrap_or_else(|e| die(&e));
    let mut file = fs::File::create(&out).expect("out file");
    let mut lines = 0;
    let mut exited_at: Option<Instant> = None;

    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok((stream, line)) => {
                lines += 1;
                let tag = if stream == Stream::Stdout { "O" } else { "E" };
                let ms = started.elapsed().as_millis();
                let _ = writeln!(file, "{ms:>7} {tag} {line}");
                let events = match stream {
                    Stream::Stdout => events::parse_line(kind, &line),
                    Stream::Stderr if kind == CliKind::Opencode => {
                        events::opencode_stderr(&line).into_iter().collect()
                    }
                    Stream::Stderr => vec![],
                };
                for ev in events {
                    let mut text = serde_json::to_string(&ev).unwrap_or_default();
                    if text.len() > 220 {
                        text = format!("{}…", &text[..text.floor_char_boundary(220)]);
                    }
                    println!("{ms:>7} {text}");
                }
                if kind == CliKind::Claude && stream == Stream::Stdout {
                    handle_claude_line(&mut run, &line, answer_allow, started);
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if exited_at.is_none() {
            if let Some(code) = run.try_exit_code() {
                println!("exit code {code} after {:.1}s", started.elapsed().as_secs_f64());
                exited_at = Some(Instant::now());
            }
        }
        // Readers can outlive the process when a grandchild keeps a pipe open.
        if exited_at.is_some_and(|t| t.elapsed() > Duration::from_secs(2)) {
            break;
        }
        if started.elapsed() > timeout {
            println!("timeout after {}s, killing", timeout.as_secs());
            run.kill();
            break;
        }
    }
    if exited_at.is_none() {
        thread::sleep(Duration::from_millis(300));
        match run.try_exit_code() {
            Some(code) => println!("exit code {code} after {:.1}s", started.elapsed().as_secs_f64()),
            None => println!("output closed, process still running"),
        }
    }
    println!("lines={lines} written to {}", out.display());
}

fn handle_claude_line(run: &mut HeadlessRun, line: &str, allow: bool, started: Instant) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { return };
    match v["type"].as_str() {
        Some("control_request") if v["request"]["subtype"] == "can_use_tool" => {
            let id = v["request_id"].as_str().unwrap_or_default();
            println!(
                "t={:.1}s permission request {id}: tool={} -> {}",
                started.elapsed().as_secs_f64(),
                v["request"]["tool_name"],
                if allow { "allow" } else { "deny" }
            );
            let reply = headless::claude_permission_answer(id, allow, &v["request"]["input"]);
            if let Err(e) = run.send_line(&reply) {
                println!("could not answer: {e}");
            }
        }
        Some("result") => run.close_stdin(),
        _ => {}
    }
}

fn main() {
    let opts = Opts::parse(std::env::args().skip(1));
    match opts.positional.first().map(String::as_str) {
        Some("detect") => {
            let t = Instant::now();
            let found = cli::detect_all();
            println!("{}", serde_json::to_string_pretty(&found).unwrap());
            println!("detect took {} ms", t.elapsed().as_millis());
        }
        Some("scan") => {
            let mut m = Monitor::new();
            let t = Instant::now();
            m.scan(&HashSet::new());
            let first_ms = t.elapsed().as_millis();
            thread::sleep(Duration::from_millis(1000));
            let t = Instant::now();
            let found = m.scan(&HashSet::new());
            println!("{}", serde_json::to_string_pretty(&found).unwrap());
            println!("scan took {first_ms} ms cold, {} ms warm", t.elapsed().as_millis());
        }
        Some("replay") => {
            let name = opts.positional.get(1).unwrap_or_else(|| die("missing <cli>"));
            let kind = CliKind::from_bin(name).unwrap_or_else(|| die("unknown cli"));
            let file = opts.positional.get(2).unwrap_or_else(|| die("missing FILE"));
            let text = fs::read_to_string(file).unwrap_or_else(|e| die(&e.to_string()));
            for raw in text.lines() {
                let mut parts = raw.trim_start().splitn(3, ' ');
                let (ms, tag, line) = (parts.next(), parts.next(), parts.next().unwrap_or(""));
                let evs = match tag {
                    Some("O") => events::parse_line(kind, line),
                    Some("E") if kind == CliKind::Opencode => {
                        events::opencode_stderr(line).into_iter().collect()
                    }
                    _ => vec![],
                };
                for ev in evs {
                    let mut t = serde_json::to_string(&ev).unwrap_or_default();
                    if t.len() > 200 {
                        t = format!("{}…", &t[..t.floor_char_boundary(200)]);
                    }
                    println!("{:>7} {t}", ms.unwrap_or(""));
                }
            }
        }
        Some("waiting") => {
            let name = opts.positional.get(1).unwrap_or_else(|| die("missing <cli>"));
            let kind = CliKind::from_bin(name).unwrap_or_else(|| die("unknown cli"));
            for file in &opts.positional[2..] {
                let screen = fs::read_to_string(file).unwrap_or_default();
                let found = waiting::detect(kind, &screen);
                println!("{file}: {}", serde_json::to_string(&found).unwrap_or_default());
            }
        }
        Some("plan") => {
            // One planner turn with the app's real context (folders from disk, open CLIs).
            let message = opts.need("message");
            let clis = cli::detect_all();
            let kind = CliKind::from_bin(opts.get("cli").unwrap_or("claude")).unwrap_or(CliKind::Claude);
            let exe = cli::resolve(kind).unwrap_or_else(|| die("planner cli not found"));
            let db = opencompanion_lib::db::Db::open_in_memory().unwrap_or_else(|e| die(&e));
            let outside = Monitor::new().scan(&HashSet::from([std::process::id()]));
            let ctx = opencompanion_lib::orchestrator::gather(&db, &outside).unwrap_or_else(|e| die(&e));
            let history: Vec<opencompanion_lib::db::ChatMessage> = opts
                .all("said")
                .iter()
                .enumerate()
                .map(|(i, t)| opencompanion_lib::db::ChatMessage {
                    id: i.to_string(),
                    thread_id: "spike".into(),
                    role: if i % 2 == 0 { "user".into() } else { "planner".into() },
                    text: t.to_string(),
                    cards: vec![],
                    created_at: i as i64,
                })
                .collect();
            println!("known folders: {}, readable dirs: {:?}", ctx.folders.len(), ctx.read_dirs);
            if opts.get("dry").is_some() {
                for f in &ctx.folders {
                    println!("  {:<7} {} ({})", f.source, f.path, f.markers.join(", "));
                }
                return;
            }
            let input = opencompanion_lib::orchestrator::PlanInput {
                message: &message,
                history: &history,
                clis: &clis,
                folders: &ctx.folders,
                sessions: &ctx.sessions,
                outside: &outside,
                read_dirs: &ctx.read_dirs,
            };
            let work = std::env::temp_dir().join("air-spike-planner");
            let t = Instant::now();
            match opencompanion_lib::orchestrator::run(kind, &exe, &work, &[], &input) {
                Ok(plan) => {
                    println!("reply: {}", plan.reply);
                    for c in plan.cards {
                        println!(
                            "card: {} {:?} in {} problem={:?}\n  prompt: {}\n  reason: {}",
                            c.cli.bin(),
                            c.mode,
                            c.folder,
                            c.problem,
                            c.prompt.replace('\n', " "),
                            c.reason
                        );
                    }
                }
                Err(e) => println!("error: {e}"),
            }
            println!("took {:.1}s", t.elapsed().as_secs_f64());
        }
        Some("pty") => cmd_pty(&opts),
        Some("headless") => cmd_headless(&opts),
        _ => die("usage: air-spike detect | scan | pty <cli> ... | headless <cli> ..."),
    }
}
