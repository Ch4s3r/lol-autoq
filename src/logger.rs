use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use tracing::Level;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

use crate::app_state::{ActivityEntry, ActivityKind};

// ── Dynamic log level ─────────────────────────────────────────────────────

/// 1 = ERROR  2 = WARN  3 = INFO  4 = DEBUG  5 = TRACE  (default: INFO)
pub static LOG_LEVEL: AtomicU8 = AtomicU8::new(3);

pub fn set_level(s: &str) {
    LOG_LEVEL.store(u8_from_str(s), Ordering::Relaxed);
}

pub fn u8_from_str(s: &str) -> u8 {
    match s.to_ascii_lowercase().as_str() {
        "error" => 1,
        "warn" => 2,
        "info" => 3,
        "debug" => 4,
        "trace" => 5,
        _ => 3,
    }
}

// ── Shared globals ────────────────────────────────────────────────────────

static LOG_FILE: OnceLock<Arc<Mutex<BufWriter<std::fs::File>>>> = OnceLock::new();
static LOG_BUFFER: OnceLock<Arc<Mutex<VecDeque<ActivityEntry>>>> = OnceLock::new();

/// Shared UI buffer — tracing events land here; drained into the activity log
/// each poll cycle.
pub fn log_buffer() -> Arc<Mutex<VecDeque<ActivityEntry>>> {
    LOG_BUFFER
        .get_or_init(|| Arc::new(Mutex::new(VecDeque::new())))
        .clone()
}

// ── Init ──────────────────────────────────────────────────────────────────

/// Open (or create) `lol-autoq.log` in **append** mode and write a session
/// separator. Must be called once at startup before any background tasks.
pub fn init(log_level_str: &str) {
    set_level(log_level_str);
    LOG_BUFFER.get_or_init(|| Arc::new(Mutex::new(VecDeque::new())));

    match OpenOptions::new()
        .create(true)
        .append(true)
        .open("lol-autoq.log")
    {
        Ok(file) => {
            let mut w = BufWriter::new(file);
            let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            let _ = writeln!(w, "\n=== Session started {ts} ===");
            let _ = w.flush();
            LOG_FILE.set(Arc::new(Mutex::new(w))).ok();
        }
        Err(e) => eprintln!("lol-autoq: cannot open log file: {e}"),
    }
}

// ── Direct write path (used by push_activity) ─────────────────────────────

/// Write an activity entry synchronously to the log file.  Called from
/// `AppState::push_activity` so every user-visible event is persisted
/// immediately, independent of the tracing level filter.
pub fn write_activity(timestamp: &str, message: &str, kind: &ActivityKind) {
    let tag = match kind {
        ActivityKind::Warning => "WARN",
        _ => "INFO",
    };
    flush_line(&format!("[{timestamp}] [{tag}] {message}\n"));
}

fn flush_line(line: &str) {
    if let Some(f) = LOG_FILE.get() {
        if let Ok(mut w) = f.lock() {
            let _ = w.write_all(line.as_bytes());
            let _ = w.flush();
        }
    }
}

// ── Tracing layer ─────────────────────────────────────────────────────────

/// Captures tracing events from our own crate modules, writes them to the log
/// file, and pushes them into the shared UI buffer so `drain_log_buffer` can
/// surface them in the activity log.
///
/// Events emitted by `app_state` are excluded because `push_activity` already
/// writes them directly (avoiding double-writing).
pub struct UiFileLayer;

impl<S: tracing::Subscriber> Layer<S> for UiFileLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let level = *event.metadata().level();
        let level_num: u8 = match level {
            Level::ERROR => 1,
            Level::WARN => 2,
            Level::INFO => 3,
            Level::DEBUG => 4,
            Level::TRACE => 5,
        };

        if level_num > LOG_LEVEL.load(Ordering::Relaxed) {
            return;
        }

        // Only handle events from our own crate.
        let target = event.metadata().target();
        if !target.starts_with("lol_autoq") {
            return;
        }
        // push_activity writes directly — skip to avoid double-writing.
        if target == "lol_autoq::app_state" {
            return;
        }

        let mut v = MessageVisitor::default();
        event.record(&mut v);
        let message = v.into_string();

        let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();
        let tag = match level {
            Level::ERROR => "ERROR",
            Level::WARN => "WARN",
            Level::INFO => "INFO",
            Level::DEBUG => "DEBUG",
            Level::TRACE => "TRACE",
        };

        flush_line(&format!("[{timestamp}] [{tag}] {message}\n"));

        // Push to UI buffer so drain_log_buffer can surface it in the activity log.
        let kind = match level {
            Level::ERROR | Level::WARN => ActivityKind::Warning,
            _ => ActivityKind::Info,
        };
        let entry = ActivityEntry {
            timestamp,
            message,
            kind,
        };
        if let Some(buf) = LOG_BUFFER.get() {
            if let Ok(mut b) = buf.lock() {
                b.push_front(entry);
                if b.len() > 500 {
                    b.pop_back();
                }
            }
        }
    }
}

// ── Visitor ───────────────────────────────────────────────────────────────

#[derive(Default)]
struct MessageVisitor {
    message: String,
    extras: Vec<String>,
}

impl MessageVisitor {
    fn into_string(self) -> String {
        if self.extras.is_empty() {
            self.message
        } else {
            format!("{} ({})", self.message, self.extras.join(", "))
        }
    }
}

impl tracing::field::Visit for MessageVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_owned();
        } else {
            self.extras.push(format!("{}={}", field.name(), value));
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        } else {
            self.extras.push(format!("{}={:?}", field.name(), value));
        }
    }
}
