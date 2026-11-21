use bevy::prelude::*;
use egui_dock::DockState;

use crate::gameplay::objects::{NpcBodyRootOffset, NpcModel, PlayerCharacter};
use crate::world_entity::WorldEntityMarker;
use crate::zengin::{
    GameChest, GameItem, GameMob, GameMover, GameNpc, MobBase, MobPose, NpcEquippedWeapon,
    NpcHealth, NpcWeaponMode,
};

/// Persistent state of the docked debug GUI.
#[derive(Resource)]
pub(crate) struct DebugUiState {
    /// Whether the left dock is expanded.
    pub(crate) left_open: bool,
    pub(crate) dock: DockState<DebugTab>,
    /// Entity selected in the entity browser and shown in the details panel.
    pub(crate) selected: Option<Entity>,
    /// Whether the right details panel is expanded.
    pub(crate) right_open: bool,
    pub(crate) filter: String,
    /// Cached interactive world entities listed in the entity browser.
    pub(crate) rows: Vec<EntityRow>,
    /// Set when the cached rows must be rebuilt.
    pub(crate) rows_dirty: bool,
    pub(crate) refresh_timer: Timer,
}

impl Default for DebugUiState {
    fn default() -> Self {
        Self {
            left_open: false,
            dock: DockState::new(vec![DebugTab::Info, DebugTab::Entities]),
            selected: None,
            right_open: true,
            filter: String::new(),
            rows: Vec::new(),
            rows_dirty: true,
            refresh_timer: Timer::from_seconds(0.5, TimerMode::Repeating),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum DebugTab {
    Info,
    Entities,
}

impl DebugTab {
    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Info => "Info",
            Self::Entities => "Entities",
        }
    }
}

/// Kind of an interactive world entity shown in the entity browser.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum WorldEntityKind {
    Player,
    Npc,
    Item,
    Chest,
    Mob,
    Mover,
}

impl WorldEntityKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Player => "player",
            Self::Npc => "npc",
            Self::Item => "item",
            Self::Chest => "chest",
            Self::Mob => "mob",
            Self::Mover => "mover",
        }
    }

    /// Coarse category used by the text filter, e.g. `vob` matches chests,
    /// interactive mobs and movers.
    pub(crate) fn category(self) -> &'static str {
        match self {
            Self::Player | Self::Npc => "npc",
            Self::Item => "item",
            Self::Chest | Self::Mob | Self::Mover => "vob",
        }
    }
}

/// One row of the entity browser.
pub(crate) struct EntityRow {
    pub(crate) entity: Entity,
    pub(crate) kind: WorldEntityKind,
    /// `Name` component value, or the raw entity id when unnamed.
    pub(crate) name: String,
    /// Lowercase kind, category and name used by the text filter.
    pub(crate) search: String,
}

/// Data read from every world entity. Grouped in nested tuples so one query can
/// serve both the entity browser and the details panel without a read-all
/// `EntityRef` query, which conflicts with mutable resources.
pub(crate) type EntityQueryItem<'a> = (
    Entity,
    Option<&'a Name>,
    (
        Option<&'a Transform>,
        Option<&'a GlobalTransform>,
        Option<&'a Visibility>,
        Option<&'a InheritedVisibility>,
        Option<&'a ViewVisibility>,
    ),
    (
        Option<&'a GameNpc>,
        Option<&'a NpcModel>,
        Option<&'a NpcBodyRootOffset>,
        Option<&'a NpcHealth>,
        Option<&'a NpcWeaponMode>,
        Option<&'a NpcEquippedWeapon>,
    ),
    (
        Option<&'a GameItem>,
        Option<&'a GameChest>,
        Option<&'a GameMob>,
        Option<&'a GameMover>,
        Option<&'a MobBase>,
        Option<&'a MobPose>,
    ),
    Option<&'a PlayerCharacter>,
);

pub(crate) type EntityQueryData = EntityQueryItem<'static>;

/// Restricts the entity query to interactive world content.
pub(crate) type EntityQueryFilter = (
    With<WorldEntityMarker>,
    Or<(
        With<GameNpc>,
        With<GameItem>,
        With<GameChest>,
        With<GameMob>,
        With<GameMover>,
    )>,
);

/// Borrowed view of the data of one world entity.
pub(crate) struct DetailsData<'a> {
    pub(crate) entity: Entity,
    pub(crate) name: Option<&'a Name>,
    pub(crate) transform: Option<&'a Transform>,
    pub(crate) global_transform: Option<&'a GlobalTransform>,
    pub(crate) visibility: Option<&'a Visibility>,
    pub(crate) inherited_visibility: Option<&'a InheritedVisibility>,
    pub(crate) view_visibility: Option<&'a ViewVisibility>,
    pub(crate) npc: Option<&'a GameNpc>,
    pub(crate) npc_model: Option<&'a NpcModel>,
    pub(crate) npc_body_root_offset: Option<&'a NpcBodyRootOffset>,
    pub(crate) npc_health: Option<&'a NpcHealth>,
    pub(crate) npc_weapon_mode: Option<&'a NpcWeaponMode>,
    pub(crate) npc_equipped_weapon: Option<&'a NpcEquippedWeapon>,
    pub(crate) item: Option<&'a GameItem>,
    pub(crate) chest: Option<&'a GameChest>,
    pub(crate) mob: Option<&'a GameMob>,
    pub(crate) mover: Option<&'a GameMover>,
    pub(crate) mob_base: Option<&'a MobBase>,
    pub(crate) mob_pose: Option<&'a MobPose>,
    pub(crate) is_player: bool,
}

impl DetailsData<'_> {
    pub(crate) fn kind(&self) -> Option<WorldEntityKind> {
        if self.mover.is_some() {
            return Some(WorldEntityKind::Mover);
        }
        if self.chest.is_some() {
            return Some(WorldEntityKind::Chest);
        }
        if self.mob.is_some() {
            return Some(WorldEntityKind::Mob);
        }
        if self.item.is_some() {
            return Some(WorldEntityKind::Item);
        }
        if self.is_player {
            return Some(WorldEntityKind::Player);
        }
        if self.npc.is_some() {
            return Some(WorldEntityKind::Npc);
        }
        None
    }

    pub(crate) fn name(&self) -> String {
        self.name
            .map_or_else(|| self.entity.to_string(), |name| name.as_str().to_owned())
    }
}

pub(crate) fn details_data(item: EntityQueryItem<'_>) -> DetailsData<'_> {
    let (
        entity,
        name,
        (transform, global_transform, visibility, inherited_visibility, view_visibility),
        (npc, npc_model, npc_body_root_offset, npc_health, npc_weapon_mode, npc_equipped_weapon),
        (item, chest, mob, mover, mob_base, mob_pose),
        is_player,
    ) = item;
    DetailsData {
        entity,
        name,
        transform,
        global_transform,
        visibility,
        inherited_visibility,
        view_visibility,
        npc,
        npc_model,
        npc_body_root_offset,
        npc_health,
        npc_weapon_mode,
        npc_equipped_weapon,
        item,
        chest,
        mob,
        mover,
        mob_base,
        mob_pose,
        is_player: is_player.is_some(),
    }
}
