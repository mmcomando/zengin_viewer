//! Conversion from live ECS components into a generic [`SavedObject`].

use bevy::prelude::*;

use crate::zengin::PlayerCharacter;
use crate::zengin::StaticMesh;
use crate::zengin::save::format::{SavedChest, SavedObject, SavedObjectBuilder};
use crate::zengin::{GameChest, GameItem, GameNpc, MobBase, NpcOverlay};

impl SavedObject {
    /// Collects the saveable components of one entity. Each component maps to
    /// its own payload. Returns `None` when the entity carries no saveable
    /// component besides its transform.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_components(
        transform: &Transform,
        player: Option<&PlayerCharacter>,
        npc: Option<&GameNpc>,
        overlay: Option<&NpcOverlay>,
        item: Option<&GameItem>,
        chest: Option<&GameChest>,
        base: Option<&MobBase>,
        static_mesh: Option<&StaticMesh>,
    ) -> Option<Self> {
        let mut builder = SavedObjectBuilder::new(*transform);
        if player.is_some() {
            builder = builder.player();
        }
        if let Some(npc) = npc {
            builder = builder.npc(npc.instance_id());
        }
        if let Some(overlay) = overlay {
            builder = builder.overlay(overlay.0.clone());
        }
        if let Some(item) = item {
            builder = builder.item(item.instance_id());
        }
        if let Some(base) = base {
            builder = builder.mob_base(base.vob_id);
        }
        if static_mesh.is_some() {
            builder = builder.static_object();
        }
        if let Some(chest) = chest {
            builder = builder.chest(SavedChest {
                locked: chest.locked,
                items: chest.items.clone(),
                cracked: chest.cracked,
            });
        }
        let object = builder.build();
        (!object.is_empty()).then_some(object)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chest() -> GameChest {
        GameChest {
            locked: true,
            key: "KEY".to_string(),
            pick_string: "LR".to_string(),
            cracked: true,
            items: Vec::new(),
        }
    }

    fn mob_base() -> MobBase {
        MobBase {
            vob_id: 3,
            vob_name: "CHEST".to_string(),
            focus_name: "MOBNAME_CHEST".to_string(),
        }
    }

    #[test]
    fn chest_saves_its_state_and_takes_the_vob_id_from_mob_base() {
        let chest = chest();
        let base = mob_base();
        let object = SavedObject::from_components(
            &Transform::IDENTITY,
            None,
            None,
            None,
            None,
            Some(&chest),
            Some(&base),
            None,
        )
        .expect("chest object");

        assert_eq!(object.mob_base.as_ref().map(|base| base.vob_id), Some(3));
        let saved = object.chest.as_ref().expect("chest payload");
        assert!(saved.locked);
        assert!(saved.cracked);
    }

    #[test]
    fn mob_base_is_saved_with_its_vob_id() {
        let base = mob_base();
        let object = SavedObject::from_components(
            &Transform::IDENTITY,
            None,
            None,
            None,
            None,
            None,
            Some(&base),
            None,
        )
        .expect("mob base object");

        assert_eq!(object.mob_base.as_ref().map(|base| base.vob_id), Some(3));
        assert!(object.static_object.is_none());
        assert!(object.chest.is_none());
    }

    #[test]
    fn static_mesh_is_a_marker_and_takes_the_vob_id_from_mob_base() {
        let static_mesh = StaticMesh;
        let base = MobBase {
            vob_id: 5,
            ..default()
        };
        let object = SavedObject::from_components(
            &Transform::IDENTITY,
            None,
            None,
            None,
            None,
            None,
            Some(&base),
            Some(&static_mesh),
        )
        .expect("static object");

        assert!(object.static_object.is_some());
        assert_eq!(object.mob_base.as_ref().map(|base| base.vob_id), Some(5));
    }

    #[test]
    fn overlay_is_saved_separately_from_the_npc() {
        let overlay = NpcOverlay(Some("HUMANS_SPRINT".to_string()));
        let object = SavedObject::from_components(
            &Transform::IDENTITY,
            None,
            None,
            Some(&overlay),
            None,
            None,
            None,
            None,
        )
        .expect("overlay object");

        assert!(object.npc.is_none());
        assert_eq!(
            object
                .overlay
                .as_ref()
                .and_then(|overlay| overlay.0.as_deref()),
            Some("HUMANS_SPRINT")
        );
    }
}
