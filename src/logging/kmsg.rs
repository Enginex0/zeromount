use std::fmt;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;

use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

const KMSG_PATH: &str = "/dev/kmsg";
const TAG: &str = "zeromount";

pub struct KmsgLayer {
    writer: Mutex<Option<std::fs::File>>,
}

impl KmsgLayer {
    pub fn new() -> Self {
        let file = OpenOptions::new().write(true).open(KMSG_PATH).ok();
        Self {
            writer: Mutex::new(file),
        }
    }

    fn kmsg_level(level: &tracing::Level) -> u8 {
        // syslog priority levels for /dev/kmsg
        match *level {
            tracing::Level::ERROR => 3,
            tracing::Level::WARN => 4,
            tracing::Level::INFO => 6,
            tracing::Level::DEBUG => 7,
            tracing::Level::TRACE => 7,
        }
    }
}

struct MessageVisitor {
    message: String,
}

impl MessageVisitor {
    fn new() -> Self {
        Self {
            message: String::new(),
        }
    }
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value);
        }
    }
}

impl<S: Subscriber> Layer<S> for KmsgLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut guard = match self.writer.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let file = match guard.as_mut() {
            Some(f) => f,
            None => return,
        };

        let mut visitor = MessageVisitor::new();
        event.record(&mut visitor);

        let pri = Self::kmsg_level(event.metadata().level());
        // kmsg format: <priority>tag: message
        let _ = writeln!(file, "<{pri}>{TAG}: {}", visitor.message);
    }
}
