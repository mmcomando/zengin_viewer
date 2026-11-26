use bevy::prelude::*;

use crate::MenuDefinition;

/// The projected `menu_log` screen and the textures it references.
#[derive(Resource, Default)]
pub struct JournalMenuData {
    pub definition: Option<MenuDefinition>,
    pub viewer_back_pic: Option<String>,
}

impl JournalMenuData {
    /// The loaded journal screen definition.
    pub fn definition(&self) -> Option<&MenuDefinition> {
        self.definition.as_ref()
    }

    pub fn viewer_back_pic(&self) -> Option<&str> {
        self.viewer_back_pic.as_deref()
    }
}
