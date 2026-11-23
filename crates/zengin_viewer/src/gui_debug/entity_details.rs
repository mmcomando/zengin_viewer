use std::sync::PoisonError;

use bevy::prelude::*;
use bevy::reflect::TypeRegistry;
use bevy_egui::egui;
use bevy_inspector_egui::reflect_inspector::{Context, InspectorUi};

use crate::gui_debug::state::{DetailsData, WorldEntityKind};
use crate::zengin::{ScriptResource, WeaponMode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DetailsAction {
    Close,
    Collapse,
}

/// Summary of `C_Npc` members shown at the top of the npc panel.
#[derive(Reflect, Debug)]
struct NpcCoreView {
    instance_id: String,
    symbol: Option<String>,
    display_name: Option<String>,
    level: i32,
    guild: i32,
    npctype: i32,
    flags: i32,
    hitpoints: i32,
    hitpoints_max: i32,
    mana: i32,
    mana_max: i32,
    strength: i32,
    dexterity: i32,
    voice: i32,
    bodymass: i32,
}

#[derive(Reflect, Debug)]
struct NpcRuntimeView {
    state_time: i32,
    dead: bool,
    walk_mode: i32,
    position: Option<Vec3>,
    routine_function: Option<String>,
    routine_function_source: Option<String>,
    routine_entries: Vec<String>,
    current_state: Option<String>,
    previous_state: Option<String>,
    target: Option<String>,
    look_at: Option<String>,
    body_state: Option<i32>,
    weapon_mode: Option<WeaponMode>,
    last_weapon: Option<WeaponMode>,
    equipped_melee: Option<String>,
    equipped_ranged: Option<String>,
    equipped_armor: Option<String>,
    inventory: Vec<String>,
    dialog_roots: usize,
    known_infos: usize,
    perception_callbacks: usize,
    perception_interval: Option<f32>,
    attitude: Option<i32>,
    temp_attitude: Option<i32>,
    refuse_talk_until: Option<f64>,
}

#[derive(Reflect, Debug)]
struct EquippedView {
    melee: Option<String>,
    ranged: Option<String>,
}

#[derive(Reflect, Debug)]
struct ItemCoreView {
    instance_id: String,
    symbol: Option<String>,
    display_name: Option<String>,
    id: i32,
    mainflag: i32,
    flags: i32,
    value: i32,
    weight: i32,
    damagetype: i32,
    damage_total: i32,
    wear: i32,
    material: i32,
    visual_skin: i32,
}

#[derive(Reflect, Debug)]
struct ItemRuntimeView {
    name: String,
    visual: String,
    visual_change: Option<String>,
}

#[derive(Reflect, Debug)]
struct ChestView {
    locked: bool,
    items: Vec<String>,
}

/// Draws the details of `details`. Returns the requested panel action.
pub(crate) fn entity_details_ui(
    ui: &mut egui::Ui,
    details: &DetailsData,
    type_registry: &TypeRegistry,
    scripts: Option<&ScriptResource>,
) -> Option<DetailsAction> {
    let kind = details.kind();

    let mut action = None;
    ui.horizontal(|ui| {
        ui.heading(details.name());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Close").clicked() {
                action = Some(DetailsAction::Close);
            }
            if ui.button("Collapse").clicked() {
                action = Some(DetailsAction::Collapse);
            }
        });
    });
    ui.label(kind.map_or("unknown", WorldEntityKind::label));
    ui.monospace(details.entity.to_string());
    if let Some(tr) = details.global_transform {
        let position = tr.translation();
        ui.monospace(format!(
            "pos: {:.2}, {:.2}, {:.2}",
            position.x, position.y, position.z
        ));
    }

    ui.separator();
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            match kind {
                Some(WorldEntityKind::Player | WorldEntityKind::Npc) => {
                    npc_details(ui, details, scripts, type_registry);
                }
                Some(WorldEntityKind::Item) => item_details(ui, details, scripts, type_registry),
                Some(WorldEntityKind::Chest) => chest_details(ui, details, type_registry),
                Some(WorldEntityKind::Mob) => mob_details(ui, details, type_registry),
                Some(WorldEntityKind::Mover) => {
                    reflect_component(ui, details.mover, type_registry, "Mover");
                }
                None => {}
            }

            ui.separator();
            ui.label("Components");
            reflect_component(ui, details.transform, type_registry, "Transform");
            reflect_component(
                ui,
                details.global_transform,
                type_registry,
                "GlobalTransform",
            );
            reflect_component(ui, details.visibility, type_registry, "Visibility");
            reflect_component(
                ui,
                details.inherited_visibility,
                type_registry,
                "InheritedVisibility",
            );
            reflect_component(ui, details.view_visibility, type_registry, "ViewVisibility");
        });

    action
}

fn reflect_component<T: Reflect>(
    ui: &mut egui::Ui,
    value: Option<&T>,
    type_registry: &TypeRegistry,
    label: &str,
) {
    let Some(value) = value else {
        return;
    };
    reflect_view(ui, label, label, value, type_registry, false);
}

fn reflect_view<T: Reflect>(
    ui: &mut egui::Ui,
    label: &str,
    id: &str,
    value: &T,
    type_registry: &TypeRegistry,
    default_open: bool,
) {
    egui::CollapsingHeader::new(label)
        .id_salt(id)
        .default_open(default_open)
        .show(ui, |ui| {
            let id = ui.make_persistent_id(id);
            let mut context = Context::default();
            let mut inspector = InspectorUi::new_no_short_circuit(type_registry, &mut context);
            inspector.ui_for_reflect_readonly_with_options(value.as_partial_reflect(), ui, id, &());
        });
}

fn npc_details(
    ui: &mut egui::Ui,
    details: &DetailsData,
    scripts: Option<&ScriptResource>,
    type_registry: &TypeRegistry,
) {
    let Some(npc) = details.npc else {
        return;
    };

    let instance_id = npc.instance_id();
    let mut script_guard = scripts.map(|scripts| scripts.lock());
    let (symbol, display_name) = match script_guard.as_mut() {
        Some(script) => (
            script.symbol_table().symbol_name(instance_id.value()),
            script.npc_display_name(instance_id),
        ),
        None => (None, None),
    };

    let handle = npc.handle();
    let (core, raw_typed) = {
        let typed = handle
            .typed()
            .read()
            .unwrap_or_else(PoisonError::into_inner);
        (
            NpcCoreView {
                instance_id: instance_id.to_string(),
                symbol,
                display_name,
                level: typed.level,
                guild: typed.guild,
                npctype: typed.npctype,
                flags: typed.flags,
                hitpoints: typed.hitpoints,
                hitpoints_max: typed.hitpoints_max,
                mana: typed.mana,
                mana_max: typed.mana_max,
                strength: typed.strength,
                dexterity: typed.dexterity,
                voice: typed.voice,
                bodymass: typed.bodymass,
            },
            format!("{typed:#?}"),
        )
    };
    reflect_view(ui, "NPC", "npc_core", &core, type_registry, true);
    reflect_component(ui, details.npc_health, type_registry, "Health");
    reflect_component(ui, details.npc_weapon_mode, type_registry, "Weapon mode");
    if let Some(equipped) = details.npc_equipped_weapon {
        let view = EquippedView {
            melee: equipped.melee.map(|id| id.to_string()),
            ranged: equipped.ranged.map(|id| id.to_string()),
        };
        reflect_view(ui, "Equipped", "npc_equipped", &view, type_registry, true);
    }
    reflect_component(
        ui,
        details.npc_body_root_offset,
        type_registry,
        "Body root offset",
    );
    reflect_component(ui, details.npc_model, type_registry, "Model");

    let (runtime, raw_runtime) = {
        let runtime = handle
            .runtime()
            .read()
            .unwrap_or_else(PoisonError::into_inner);
        let mut resolve = |id: i32| {
            script_guard
                .as_mut()
                .and_then(|script| script.symbol_table().symbol_name(id))
                .unwrap_or_else(|| id.to_string())
        };
        let executor = details.npc_routine_executor;
        let routine_function = executor.map(|executor| resolve(executor.functions().start.value()));
        let routine_function_source = executor.map(|executor| {
            if executor.is_ai_state() {
                "ai state".to_owned()
            } else {
                "routine".to_owned()
            }
        });
        let routine_entries = runtime
            .routine
            .entries
            .iter()
            .map(|entry| {
                let state = entry
                    .state_fn
                    .map(|id| resolve(id.value()))
                    .unwrap_or_else(|| "-".to_owned());
                format!(
                    "{:02}:{:02}-{:02}:{:02} {state} @ {}",
                    entry.start_h, entry.start_m, entry.stop_h, entry.stop_m, entry.way_point
                )
            })
            .collect();
        (
            NpcRuntimeView {
                state_time: runtime.routine.state_time,
                dead: runtime.live.dead,
                walk_mode: runtime.live.walk_mode,
                position: runtime.live.position,
                routine_function,
                routine_function_source,
                routine_entries,
                current_state: runtime.routine.current_state.map(|id| resolve(id.value())),
                previous_state: runtime.routine.previous_state.map(|id| resolve(id.value())),
                target: runtime.live.target.map(|id| resolve(id.value())),
                look_at: runtime.live.look_at.map(|id| resolve(id.value())),
                body_state: runtime.live.body_state,
                weapon_mode: runtime.equipment.weapon_mode,
                last_weapon: runtime.equipment.last_weapon,
                equipped_melee: runtime.equipment.melee.map(|id| resolve(id.value())),
                equipped_ranged: runtime.equipment.ranged.map(|id| resolve(id.value())),
                equipped_armor: runtime.equipment.armor.map(|id| resolve(id.value())),
                inventory: runtime
                    .inventory
                    .items()
                    .into_iter()
                    .map(|(item, count)| format!("{} x{count}", resolve(item.value())))
                    .collect(),
                dialog_roots: runtime.dialog.roots.as_ref().map_or(0, Vec::len),
                known_infos: runtime.dialog.known_infos.len(),
                perception_callbacks: runtime.perception.callbacks.len(),
                perception_interval: runtime.perception.interval,
                attitude: runtime.social.attitude,
                temp_attitude: runtime.social.temp_attitude,
                refuse_talk_until: runtime.social.refuse_talk_until,
            },
            format!("{runtime:#?}"),
        )
    };
    reflect_view(ui, "Runtime", "npc_runtime", &runtime, type_registry, false);
    ui.collapsing("Raw C_Npc", |ui| {
        ui.monospace(raw_typed);
    });
    ui.collapsing("Raw runtime", |ui| {
        ui.monospace(raw_runtime);
    });
}

fn item_details(
    ui: &mut egui::Ui,
    details: &DetailsData,
    scripts: Option<&ScriptResource>,
    type_registry: &TypeRegistry,
) {
    let Some(item) = details.item else {
        return;
    };

    let instance_id = item.instance_id();
    let mut symbol = None;
    let mut display_name = None;
    if let Some(scripts) = scripts {
        let mut script = scripts.lock();
        symbol = script.symbol_table().symbol_name(instance_id.value());
        display_name = script.item_display_name(instance_id);
    }

    let handle = item.handle();
    let (core, raw_typed) = {
        let typed = handle
            .typed()
            .read()
            .unwrap_or_else(PoisonError::into_inner);
        (
            ItemCoreView {
                instance_id: instance_id.to_string(),
                symbol,
                display_name,
                id: typed.id,
                mainflag: typed.mainflag,
                flags: typed.flags,
                value: typed.value,
                weight: typed.weight,
                damagetype: typed.damagetype,
                damage_total: typed.damage_total,
                wear: typed.wear,
                material: typed.material,
                visual_skin: typed.visual_skin,
            },
            format!("{typed:#?}"),
        )
    };
    reflect_view(ui, "C_Item", "item_core", &core, type_registry, true);

    let runtime_view = {
        let runtime = handle
            .runtime()
            .read()
            .unwrap_or_else(PoisonError::into_inner);
        ItemRuntimeView {
            name: runtime.name.clone(),
            visual: runtime.visual.clone(),
            visual_change: runtime.visual_change.clone(),
        }
    };
    reflect_view(
        ui,
        "Runtime",
        "item_runtime",
        &runtime_view,
        type_registry,
        false,
    );
    ui.collapsing("Raw C_Item", |ui| {
        ui.monospace(raw_typed);
    });
}

fn chest_details(ui: &mut egui::Ui, details: &DetailsData, type_registry: &TypeRegistry) {
    reflect_component(ui, details.mob_base, type_registry, "Vob");
    reflect_component(ui, details.mob_pose, type_registry, "Pose");
    if let Some(chest) = details.chest {
        let view = ChestView {
            locked: chest.locked,
            items: chest
                .items
                .iter()
                .map(|(item, count)| format!("{item} x{count}"))
                .collect(),
        };
        reflect_view(ui, "Chest", "chest", &view, type_registry, true);
    }
}

fn mob_details(ui: &mut egui::Ui, details: &DetailsData, type_registry: &TypeRegistry) {
    reflect_component(ui, details.mob_base, type_registry, "Vob");
    reflect_component(ui, details.mob_pose, type_registry, "Pose");
    reflect_component(ui, details.mob, type_registry, "Mob");
}
