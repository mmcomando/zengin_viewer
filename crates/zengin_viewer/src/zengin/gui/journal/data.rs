use bevy::prelude::*;

use crate::zengin::MenuDefinition;

/// The projected `menu_log` screen and the textures it references.
#[derive(Resource, Default)]
pub(crate) struct JournalMenuData {
    pub(super) definition: Option<MenuDefinition>,
    pub(super) viewer_back_pic: Option<String>,
}

impl JournalMenuData {
    /// The loaded journal screen definition.
    pub(crate) fn definition(&self) -> Option<&MenuDefinition> {
        self.definition.as_ref()
    }

    pub(super) fn viewer_back_pic(&self) -> Option<&str> {
        self.viewer_back_pic.as_deref()
    }
}
