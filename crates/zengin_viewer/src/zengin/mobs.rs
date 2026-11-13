//! Interactive mob registry and mesh attachment-point extraction.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::zengin::GameMob;
use crate::zengin::resources::ZenGinModelComponent;
use crate::zengin::script::string_value::StringCanon;
use crate::zengin::visual::model::ZenGinModel;

pub struct MobsPlugin;

impl Plugin for MobsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MobRegistry>();
        app.add_systems(Update, (extract_mob_positions, release_despawned_mob_uses));
    }
}

/// Runtime index of interactive mobs by their `AI_UseMob` scheme.
#[derive(Resource, Default, Debug)]
pub struct MobRegistry {
    by_scheme: HashMap<String, Vec<Entity>>,
}

impl MobRegistry {
    pub fn insert(&mut self, scheme: String, entity: Entity) {
        self.by_scheme.entry(scheme).or_default().push(entity);
    }

    pub fn mobs(&self, scheme: &str) -> &[Entity] {
        self.by_scheme.get(scheme).map_or(&[], Vec::as_slice)
    }

    #[allow(dead_code)] // consumed by the mob use action
    pub fn iter(&self) -> impl Iterator<Item = (&str, &[Entity])> {
        self.by_scheme
            .iter()
            .map(|(scheme, mobs)| (scheme.as_str(), mobs.as_slice()))
    }
}

/// Npc currently attached to a mob interaction point.
#[derive(Component, Debug, Clone)]
pub struct NpcMobUse {
    pub mob: Entity,
    pub scheme: StringCanon,
}

/// Npc entity occupying a mob interaction point.
#[derive(Component, Debug, Clone, Copy)]
pub struct MobOccupant(pub Entity);

/// Releases mob points held by despawned npcs.
fn release_despawned_mob_uses(
    mut commands: Commands,
    mut removed: RemovedComponents<NpcMobUse>,
    mobs: Query<(Entity, &MobOccupant)>,
) {
    for npc in removed.read() {
        for (mob, occupant) in &mobs {
            if occupant.0 == npc {
                commands.entity(mob).remove::<MobOccupant>();
            }
        }
    }
}

/// Fills [`GameMob::positions`] once the mob model asset is loaded.
fn extract_mob_positions(
    mut mobs: Query<(&mut GameMob, &ZenGinModelComponent)>,
    models: Res<Assets<ZenGinModel>>,
) {
    for (mut mob, model_component) in &mut mobs {
        if mob.positions.is_some() {
            continue;
        }
        let Some(model) = models.get(&model_component.model_handle) else {
            continue;
        };
        let positions = mob_positions(model);
        debug!(
            "mob({}) extracted {} attachment point(s)",
            mob.focus_name,
            positions.len()
        );
        mob.positions = Some(positions);
    }
}

/// `ZS_POS*` nodes of a mob model with their model-space transforms.
fn mob_positions(model: &ZenGinModel) -> Vec<crate::zengin::MobPosition> {
    model
        .node_names
        .iter()
        .zip(&model.final_tr)
        .filter(|(name, _)| name.starts_with("ZS_POS"))
        .map(|(name, transform)| crate::zengin::MobPosition {
            name: name.clone(),
            transform: *transform,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mob_positions_keep_zs_pos_nodes_in_model_space() {
        let front = Transform::from_xyz(1.0, 2.0, 3.0);
        let model = ZenGinModel {
            node_names: vec!["BIP01".to_string(), "ZS_POS0_FRONT".to_string()],
            final_tr: vec![Transform::IDENTITY, front],
            ..Default::default()
        };

        let positions = mob_positions(&model);

        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].name, "ZS_POS0_FRONT");
        assert_eq!(positions[0].transform, front);
    }
}
