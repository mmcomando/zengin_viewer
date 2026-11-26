//! Capture of tracing events during scenario runs, backing the `TestLogExists`
//! and `TestClearLogBuffer` replay commands.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use bevy::log::{
    BoxedLayer, LogPlugin,
    tracing::{self, Subscriber},
    tracing_subscriber::{Layer, layer::Context},
};
use bevy::prelude::*;

use input_core::Options;

/// Log lines captured during the current scenario run, each formatted as
/// `<LEVEL> <target>: <message> <extra fields>`.
#[derive(Resource, Clone, Default)]
pub(crate) struct LogBuffer(Arc<Mutex<Vec<String>>>);

impl LogBuffer {
    pub(crate) fn push(&self, line: String) {
        self.lock().push(line);
    }

    pub(crate) fn clear(&self) {
        self.lock().clear();
    }

    pub(crate) fn snapshot(&self) -> Vec<String> {
        self.lock().clone()
    }

    fn lock(&self) -> MutexGuard<'_, Vec<String>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

struct CaptureLayer {
    buffer: LogBuffer,
}

impl<S: Subscriber> Layer<S> for CaptureLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _context: Context<'_, S>) {
        self.buffer.push(format_event(event));
    }
}

#[derive(Default)]
struct FieldVisitor {
    message: Option<String>,
    fields: Vec<String>,
}

impl tracing::field::Visit for FieldVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
            return;
        }
        self.fields.push(format!("{}={value}", field.name()));
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{value:?}"));
            return;
        }
        self.fields.push(format!("{}={value:?}", field.name()));
    }
}

fn format_event(event: &tracing::Event<'_>) -> String {
    let mut visitor = FieldVisitor::default();
    event.record(&mut visitor);
    let metadata = event.metadata();
    let mut line = format!(
        "{} {}: {}",
        metadata.level(),
        metadata.target(),
        visitor.message.unwrap_or_default()
    );
    if !visitor.fields.is_empty() {
        line.push(' ');
        line.push_str(&visitor.fields.join(" "));
    }
    line
}

/// [`LogPlugin`] with scenario log capture added. Capture is enabled only when
/// the run replays a scenario.
pub(crate) fn scenario_log_plugin() -> LogPlugin {
    LogPlugin {
        custom_layer: scenario_capture_layer,
        ..default()
    }
}

fn scenario_capture_layer(app: &mut App) -> Option<BoxedLayer> {
    let scenario_enabled = app
        .world()
        .get_resource::<Options>()
        .is_some_and(|options| options.scenario_enabled());
    if !scenario_enabled {
        return None;
    }
    let buffer = LogBuffer::default();
    app.insert_resource(buffer.clone());
    Some(CaptureLayer { buffer }.boxed())
}

#[cfg(test)]
mod tests {
    use bevy::log::tracing_subscriber::prelude::*;
    use bevy::log::{info, tracing, warn};

    use super::{CaptureLayer, LogBuffer};

    #[test]
    fn capture_layer_records_formatted_lines() {
        let buffer = LogBuffer::default();
        let subscriber = bevy::log::tracing_subscriber::registry().with(CaptureLayer {
            buffer: buffer.clone(),
        });
        tracing::subscriber::with_default(subscriber, || {
            info!("hello {}", 7);
            warn!(target: "zengin_viewer::test", "missing texture");
        });
        let lines = buffer.snapshot();
        assert!(
            lines
                .iter()
                .any(|line| line.contains("INFO") && line.contains("hello 7"))
        );
        assert!(lines.iter().any(|line| line.contains("WARN")
            && line.contains("zengin_viewer::test")
            && line.contains("missing texture")));
    }
}
