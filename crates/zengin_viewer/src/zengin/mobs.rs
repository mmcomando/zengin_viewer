//! Interactive mob registry and mesh attachment-point extraction.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::gameplay::game_time::GameTime;
use crate::zengin::GameMob;
use crate::zengin::GameNpc;
use crate::zengin::resources::ZenGinModelComponent;
use crate::zengin::script::component::ScriptResource;
use crate::zengin::script::state::MobRoutineEntry;
use crate::zengin::script::state::ScriptMob;
use crate::zengin::script::string_value::StringCanon;
use crate::zengin::visual::model::ZenGinModel;

pub struct MobsPlugin;

impl Plugin for MobsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MobRegistry>();
        app.add_systems(
            Update,
            (
                extract_mob_positions,
                release_despawned_mob_uses,
                sync_npc_body_state,
                apply_mob_routines,
                sync_script_mobs,
            ),
        );
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

/// Mirrors the used mob onto the npc body state reported to the scripts.
fn sync_npc_body_state(
    gothic_script: Option<Res<ScriptResource>>,
    added: Query<(&GameNpc, &NpcMobUse), Added<NpcMobUse>>,
    mut removed: RemovedComponents<NpcMobUse>,
    npcs: Query<(&GameNpc, Option<&NpcMobUse>)>,
) {
    let Some(gothic_script) = gothic_script else {
        return;
    };
    let mut script = gothic_script.lock();
    for entity in removed.read() {
        // Despawned npcs keep no script state, and a switched mob still has its
        // component here and is handled below.
        let Ok((npc, current)) = npcs.get(entity) else {
            continue;
        };
        if current.is_some() {
            continue;
        }
        debug!("npc({}) body state cleared", npc.instance_id());
        script.set_npc_body_state(npc.instance_id(), None);
    }
    for (npc, mob_use) in &added {
        let body_state = script.scheme_body_state(mob_use.scheme.as_str());
        debug!(
            "npc({}) body state({body_state}) for mob({})",
            npc.instance_id(),
            mob_use.scheme
        );
        script.set_npc_body_state(npc.instance_id(), Some(body_state));
    }
}

/// Applies the `Wld_SetMobRoutine` states of the current time to the mobs.
fn apply_mob_routines(
    gothic_script: Option<Res<ScriptResource>>,
    game_time: Res<GameTime>,
    mut mobs: Query<&mut GameMob>,
) {
    let Some(gothic_script) = gothic_script else {
        return;
    };
    let routines = {
        let script = gothic_script.lock();
        let routines = &script.get_loaded_state().mob_routines;
        if routines.is_empty() {
            return;
        }
        routines.clone()
    };
    let minutes = game_time.minutes_of_day();
    let mut states: HashMap<String, i32> = HashMap::new();
    for mut mob in &mut mobs {
        let scheme = mob.scheme.to_uppercase();
        let state = states
            .entry(scheme.clone())
            .or_insert_with(|| routine_state_at(&routines, &scheme, minutes).unwrap_or(0));
        if mob.state == *state {
            continue;
        }
        debug!(
            "mob({}) scheme({scheme}) routine state {} -> {state}",
            mob.focus_name, mob.state
        );
        mob.state = *state;
    }
}

/// State of the last schedule entry at or before `minutes`, the last entry when
/// every entry is still ahead, matching the engine `MobStates` semantics.
fn routine_state_at(routines: &[MobRoutineEntry], scheme: &str, minutes: u32) -> Option<i32> {
    let mut active: Option<(u32, i32)> = None;
    let mut latest: Option<(u32, i32)> = None;
    for routine in routines {
        if !routine.mob_name.eq_ignore_ascii_case(scheme) {
            continue;
        }
        let at = (routine.hour.max(0) as u32) * 60 + routine.minute.max(0) as u32;
        if latest.is_none_or(|(latest_at, _)| at >= latest_at) {
            latest = Some((at, routine.state));
        }
        if at <= minutes && active.is_none_or(|(active_at, _)| at >= active_at) {
            active = Some((at, routine.state));
        }
    }
    active.or(latest).map(|(_, state)| state)
}

/// Publishes the mob table for `Wld_IsMobAvailable`/`Wld_GetMobState`.
fn sync_script_mobs(
    gothic_script: Option<Res<ScriptResource>>,
    mobs: Query<(&GameMob, &GlobalTransform, Option<&MobOccupant>)>,
    mut initialized: Local<bool>,
    added_occupancy: Query<(), Added<MobOccupant>>,
    changed_mobs: Query<(), Changed<GameMob>>,
    mut removed_occupancy: RemovedComponents<MobOccupant>,
) {
    let occupancy_removed = removed_occupancy.read().count() > 0;
    let dirty = !*initialized
        || occupancy_removed
        || !added_occupancy.is_empty()
        || !changed_mobs.is_empty();
    if !dirty {
        return;
    }
    let Some(gothic_script) = gothic_script else {
        return;
    };
    let snapshot = mobs
        .iter()
        .map(|(mob, mob_tr, occupant)| {
            (
                mob.scheme.to_lowercase(),
                ScriptMob {
                    position: mob_tr.translation(),
                    occupied: occupant.is_some(),
                    state: mob.state,
                },
            )
        })
        .collect();
    gothic_script.lock().set_mobs(snapshot);
    *initialized = true;
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

    #[test]
    fn routine_state_uses_the_last_entry_at_or_before_the_minute() {
        let routines = [routine(5, 0, 0), routine(0, 0, 1), routine(20, 0, 1)];

        assert_eq!(routine_state_at(&routines, "FIREPLACE", 4 * 60), Some(1));
        assert_eq!(routine_state_at(&routines, "FIREPLACE", 10 * 60), Some(0));
        assert_eq!(routine_state_at(&routines, "fireplace", 21 * 60), Some(1));
    }

    fn routine(hour: i32, minute: i32, state: i32) -> MobRoutineEntry {
        MobRoutineEntry {
            hour,
            minute,
            mob_name: "FIREPLACE".to_string(),
            state,
        }
    }
}
