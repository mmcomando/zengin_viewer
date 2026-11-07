use bevy::prelude::*;

use super::data::StatsSnapshot;

/// State of the stats screen: closed by default, values snapshot on open.
#[derive(Resource, Default)]
pub(crate) struct StatsUiState {
    open: bool,
    snapshot: Option<StatsSnapshot>,
    rebuild: bool,
    last_size: Vec2,
    last_scale: f32,
}

impl StatsUiState {
    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    pub(crate) fn close(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn open(&mut self, snapshot: StatsSnapshot) {
        *self = Self {
            open: true,
            snapshot: Some(snapshot),
            rebuild: true,
            last_size: Vec2::ZERO,
            last_scale: 0.0,
        };
    }

    pub(super) fn snapshot(&self) -> Option<&StatsSnapshot> {
        self.snapshot.as_ref()
    }

    pub(super) fn take_rebuild(&mut self, size: Vec2, scale: f32) -> bool {
        let rebuild = self.rebuild
            || self.last_size != size
            || (self.last_scale - scale).abs() > f32::EPSILON;
        if rebuild {
            self.last_size = size;
            self.last_scale = scale;
            self.rebuild = false;
        }
        rebuild
    }
}
