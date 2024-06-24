use apalis::layers::tracing::MakeSpan;
use apalis_core::{
    request::Request,
    task::{attempt::Attempt, task_id::TaskId},
};
use chrono::{DateTime, Utc};
use tracing::{Level, Span};

// ? ---------------------------------------------------------------------------
// ? Models
// ? ---------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct Reminder(DateTime<Utc>);

impl From<DateTime<Utc>> for Reminder {
    fn from(t: DateTime<Utc>) -> Self {
        Reminder(t)
    }
}

// ? ---------------------------------------------------------------------------
// ? Span
// ? ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub(crate) struct ReminderSpan {
    level: Level,
}

impl Default for ReminderSpan {
    fn default() -> Self {
        Self::new()
    }
}

impl ReminderSpan {
    pub fn new() -> Self {
        Self {
            level: Level::DEBUG,
        }
    }
}

impl<B> MakeSpan<B> for ReminderSpan {
    fn make_span(&mut self, req: &Request<B>) -> Span {
        let task_id: &TaskId = req.get().unwrap();
        let attempts: Attempt = req.get().cloned().unwrap_or_default();
        let span = Span::current();

        macro_rules! make_span {
            ($level:expr) => {
                tracing::span!(
                    parent: span,
                    $level,
                    "reminder",
                    task_id = task_id.to_string(),
                    attempt = attempts.current().to_string(),
                )
            };
        }

        match self.level {
            Level::ERROR => {
                make_span!(Level::ERROR)
            }
            Level::WARN => {
                make_span!(Level::WARN)
            }
            Level::INFO => {
                make_span!(Level::INFO)
            }
            Level::DEBUG => {
                make_span!(Level::DEBUG)
            }
            Level::TRACE => {
                make_span!(Level::TRACE)
            }
        }
    }
}
