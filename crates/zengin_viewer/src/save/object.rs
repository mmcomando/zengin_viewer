//! Conversion from live ECS components into a generic [`SavedObject`].

use bevy::prelude::*;

use crate::gameplay::objects::PlayerCharacter;
use crate::save::format::{SavedChest, SavedItem, SavedNpc, SavedObject, SavedPlayer};
use crate::zengin::{GameChest, GameItem, GameNpc, MobBase, NpcOverlay};

impl SavedObject {
    /// Collects the saveable components of one entity. Returns `None` when the
    /// entity carries no saveable component besides its transform.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_components(
        transform: &Transform,
        player: Option<&PlayerCharacter>,
        npc: Option<&GameNpc>,
        overlay: Option<&NpcOverlay>,
        item: Option<&GameItem>,
        chest: Option<&GameChest>,
        base: Option<&MobBase>,
    ) -> Option<Self> {
        let npc = npc.map(|npc| SavedNpc {
            instance_id: npc.instance_id(),
            overlay: overlay.and_then(|overlay| overlay.0.clone()),
        });
        let item = item.map(|item| SavedItem {
            instance_id: item.instance_id(),
        });
        let chest = match (chest, base) {
            (Some(chest), Some(base)) => Some(SavedChest {
                vob_id: base.vob_id,
                locked: chest.locked,
                items: chest.items.clone(),
                cracked: chest.cracked,
            }),
            (Some(_), None) => {
                warn!("Chest entity has no MobBase, its state is not saved");
                None
            }
            (None, _) => None,
        };
        let object = Self {
            transform: *transform,
            player: player.map(|_| SavedPlayer),
            npc,
            item,
            chest,
        };
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
    fn chest_saves_its_vob_id_and_state() {
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
        )
        .expect("chest object");

        let saved = object.chest.as_ref().expect("chest payload");
        assert_eq!(saved.vob_id, 3);
        assert!(saved.locked);
        assert!(saved.cracked);
    }

    #[test]
    fn interactive_mob_components_are_not_saved() {
        let base = mob_base();
        let object = SavedObject::from_components(
            &Transform::IDENTITY,
            None,
            None,
            None,
            None,
            None,
            Some(&base),
        );
        assert!(object.is_none());
    }
}
