use bevy::prelude::*;

use crate::zengin::MenuDefinition;

/// The projected `menu_status` screen, `None` when MENU.DAT could not be read.
#[derive(Resource, Default)]
pub(crate) struct StatsMenuData(pub(super) Option<MenuDefinition>);

impl StatsMenuData {
    /// The loaded status screen definition.
    pub(super) fn definition(&self) -> Option<&MenuDefinition> {
        self.0.as_ref()
    }
}
