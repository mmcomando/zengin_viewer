use super::bindings::mmcomando;
use super::journal::{JournalAction, JournalChange, JournalSection, JournalStatus};
use super::state::{
    AlignToWpChange, DialogChoice, DialogLine, EquipItemChange, HealthChange, ImportCallState,
    InventoryGain, MobRoutineEntry, NpcLookAtChange, OverlayRequest, RoutineEntry,
    RoutineEntryChange, RoutineExchange, SpawnItem, SpawnNpc, WeaponKind, WeaponMode,
};
use super::string_value::StringCanon;
use super::symbol_id::SymbolId;
use crate::warn_unimplemented;
use crate::zengin::common::to_zengin_units;
use bevy::log::warn;
use bevy::math::Vec3;

macro_rules! fake_impl {
    ($name:ident($($arg:ident),*)) => {
        fn $name(&mut self, $($arg: i32),*) {
            warn_unimplemented!(stringify!($name));
        }
    };
    ($name:ident($($arg:ident),*) -> i32) => {
        fn $name(&mut self, $($arg: i32),*) -> i32 {
            warn_unimplemented!(stringify!($name));
            0
        }
    };
    // String-returning externals must record an entry in the DYN_STRINGS
    // mirror even when stubbed, the mirror stays in sync with the guest ring
    // cursor only if every string-returning call is recorded
    ($name:ident($($arg:ident),*) -> Vec<u8>) => {
        fn $name(&mut self, $($arg: i32),*) -> Vec<u8> {
            warn_unimplemented!(stringify!($name));
            self.record_dynamic_string(Vec::new());
            Vec::new()
        }
    };

}

impl mmcomando::zengine_scripts::scripts::Host for ImportCallState {
    fn wld_insertnpc(&mut self, npc_index: i32, waypoint_string_id: i32) {
        let Some(npc_index) = self.symbol_table.instance(npc_index) else {
            return;
        };
        let Some(waypoint_string_id) = self.symbol_table.string(waypoint_string_id) else {
            return;
        };
        // println!(
        //     "NEW wld_insertnpc npc_index({npc_index}), waypoint_string_id({waypoint_string_id})"
        // );

        self.pending.spawn_npcs.push(SpawnNpc {
            instance_id: npc_index,
            transform: None,
            location_name: Some(waypoint_string_id),
        });
    }

    fn wld_insertitem(&mut self, item_index: i32, waypoint_string_id: i32) {
        // println!("CCC");
        let Some(item_index) = self.symbol_table.instance(item_index) else {
            return;
        };
        let Some(waypoint_string_id) = self.symbol_table.string(waypoint_string_id) else {
            return;
        };
        // println!(
        //     "NEW wld_insertitem item_index({item_index}), item_name({item_name}), waypoint_string_id({waypoint_string_id}), waypoint_name({waypoint_name})"
        // );

        self.pending.spawn_items.push(SpawnItem {
            instance_id: item_index,
            transform: None,
            location_name: Some(waypoint_string_id),
        });
    }

    fn ta_min(
        &mut self,
        npc_index: i32,
        start_h: i32,
        start_m: i32,
        stop_h: i32,
        stop_m: i32,
        func_index: i32,
        way_point: i32,
    ) {
        // println!(
        //     "NEW ta_min npc_index({npc_index}), start_h({start_h}), stop_h({stop_h}), way_point({way_point})"
        // );
        let Some(npc_index) = self.symbol_table.instance(npc_index) else {
            return;
        };
        let Some(way_point) = self.symbol_table.string(way_point) else {
            return;
        };
        let Some(way_point) = self.get_canon_string(way_point) else {
            return;
        };

        let routine_entry = RoutineEntry {
            start_h: start_h.max(0) as u32,
            start_m: start_m.max(0) as u32,
            stop_h: stop_h.max(0) as u32,
            stop_m: stop_m.max(0) as u32,
            way_point,
            // Id 0 means the state has no function
            state_fn: self.symbol_table.function(func_index),
        };

        let entry = self
            .loaded_state
            .npc_instances
            .entry(npc_index)
            .or_default();
        entry.routine_entries.push(routine_entry);
        self.pending.routine_changes.push(RoutineEntryChange {
            instance_id: npc_index,
        });
    }

    fn ai_playani(&mut self, npc_index: i32, ani_id: i32) {
        let Some(npc_index) = self.symbol_table.instance(npc_index) else {
            return;
        };
        let Some(ani_id) = self.symbol_table.string(ani_id) else {
            return;
        };
        let Some(ani) = self.get_show_string(ani_id) else {
            return;
        };
        self.loaded_state
            .npc_instances
            .entry(npc_index)
            .or_default()
            .routine_played_anis
            .push(StringCanon::new(ani.string()));
    }

    fn ai_playanibs(&mut self, npc_index: i32, ani_id: i32, _body_state: i32) {
        let Some(npc_index) = self.symbol_table.instance(npc_index) else {
            return;
        };
        let Some(ani_id) = self.symbol_table.string(ani_id) else {
            return;
        };
        let Some(ani) = self.get_show_string(ani_id) else {
            return;
        };
        self.loaded_state
            .npc_instances
            .entry(npc_index)
            .or_default()
            .routine_played_anis
            .push(StringCanon::new(ani.string()));
    }

    fn ai_output(&mut self, npc_a: i32, npc_b: i32, line_id: i32) {
        let Some(npc_a) = self.symbol_table.instance(npc_a) else {
            return;
        };
        let Some(npc_b) = self.symbol_table.instance(npc_b) else {
            return;
        };
        let Some(line_id) = self.symbol_table.string(line_id) else {
            return;
        };
        let Some(line) = self.get_show_string(line_id) else {
            warn!(
                "ai_output with empty line string, npc_a({npc_a}), npc_b({npc_b}), line_id({line_id})"
            );
            return;
        };
        self.pending.dialog_lines.push(DialogLine {
            npc_a,
            npc_b,
            line_id,
            line_str: StringCanon::new(line.string()),
        });
    }

    fn ai_usemob(&mut self, npc_index: i32, mob_name_id: i32, state: i32) -> i32 {
        // Negative states release the mob, nothing to animate.
        if state <= 0 {
            return 0;
        }
        let Some(npc_index) = self.symbol_table.instance(npc_index) else {
            return 0;
        };
        let Some(mob_name_id) = self.symbol_table.string(mob_name_id) else {
            return 0;
        };
        let Some(mob) = self.get_show_string(mob_name_id) else {
            return 0;
        };
        let ani = format!("S_{}_S{state}", mob.string().to_uppercase());
        self.loaded_state
            .npc_instances
            .entry(npc_index)
            .or_default()
            .routine_played_anis
            .push(StringCanon::new(ani));
        0
    }

    fn info_addchoice(&mut self, dialog_id: i32, description_text_id: i32, function_id: i32) {
        let Some(dialog_id) = self.symbol_table.instance(dialog_id) else {
            return;
        };
        let Some(description_text_id) = self.symbol_table.string(description_text_id) else {
            return;
        };
        let Some(function_id) = self.symbol_table.function(function_id) else {
            return;
        };
        let description = self
            .get_show_string(description_text_id)
            .map(super::string_value::ShowString::into_string)
            .unwrap_or_default();

        self.loaded_state
            .dialog_choices
            .entry(dialog_id)
            .or_default()
            .push(DialogChoice {
                description,
                sequence_function_id: function_id,
            });
    }

    /// Returns whether the two string symbols hold equal text.
    fn hlp_strcmp(&mut self, aa: i32, bb: i32) -> i32 {
        let Some(aa) = self.symbol_table.string(aa) else {
            return 0;
        };
        let Some(bb) = self.symbol_table.string(bb) else {
            return 0;
        };
        let aa = self.get_show_string(aa);
        let bb = self.get_show_string(bb);
        match (aa, bb) {
            // Both empty compares equal, like the previous `Option<String>`
            // equality did
            (None, None) => 1,
            (Some(aa), Some(bb)) => i32::from(aa.string() == bb.string()),
            _ => 0,
        }
    }

    /// Distance from the npc to a named waypoint in engine units. The engine
    /// returns `i32::MAX` when the npc or the waypoint is unknown.
    fn npc_getdisttowp(&mut self, npc: i32, wp_name: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return i32::MAX;
        };
        let Some(wp_name) = self.symbol_table.string(wp_name) else {
            return i32::MAX;
        };
        let Some(wp_name) = self.get_canon_string(wp_name) else {
            return i32::MAX;
        };

        let Some(target) = self.way_points.get(wp_name.as_str()).copied() else {
            return i32::MAX;
        };
        let Some(position) = self
            .loaded_state
            .npc_instances
            .get(&npc)
            .and_then(|npc| npc.position)
        else {
            return i32::MAX;
        };

        // Positions are stored in Bevy meters, the engine works in game units
        let distance = to_zengin_units((position - target).length());
        distance.min(i32::MAX as f32) as i32
    }

    fn info_clearchoices(&mut self, dialog_id: i32) {
        let Some(dialog_id) = self.symbol_table.instance(dialog_id) else {
            return;
        };
        self.loaded_state.dialog_choices.remove(&dialog_id);
    }

    fn mdl_setvisualbody(
        &mut self,
        npc_index: i32,
        body_model: i32,
        body_texture_index: i32,
        _par_4: i32,
        head_model: i32,
        face_texture_index: i32,
        _par_1: i32,
        armor_model: i32,
    ) {
        let Some(npc_index) = self.symbol_table.instance(npc_index) else {
            return;
        };
        let entry = self
            .loaded_state
            .npc_instances
            .entry(npc_index)
            .or_default();
        entry.body_model_id = self.symbol_table.string(body_model);
        entry.head_model_id = self.symbol_table.string(head_model);
        // The armor argument is an item instance symbol, not a string literal
        entry.armor_model_id = SymbolId::new(armor_model);
        entry.face_texture_index = face_texture_index;
        entry.body_texture_index = body_texture_index;
        entry.visual_initialized = true;
    }

    fn mdl_setvisual(&mut self, npc_index: i32, body_model_id: i32) {
        let Some(npc_index) = self.symbol_table.instance(npc_index) else {
            return;
        };
        let entry = self
            .loaded_state
            .npc_instances
            .entry(npc_index)
            .or_default();
        entry.hierarchy_id = self.symbol_table.string(body_model_id);
        entry.visual_initialized = true;
    }

    fn ai_stopprocessinfos(&mut self, _npc: i32) {
        self.close_dialog = true;
    }

    fn inttofloat(&mut self, var: i32) -> i32 {
        (var as f32).to_bits() as i32
    }
    // String-building externals: the returned content is copied into the
    // DYN_STRINGS ring by the guest and must be recorded here so the mirror
    // ring stays in sync with the guest ring cursor
    fn concatstrings(&mut self, aa: i32, bb: i32) -> Vec<u8> {
        let mut content = self.get_string_bytes(aa);
        content.extend(self.get_string_bytes(bb));
        self.record_dynamic_string(content.clone());
        content
    }

    fn inttostring(&mut self, var: i32) -> Vec<u8> {
        let content = var.to_string().into_bytes();
        self.record_dynamic_string(content.clone());
        content
    }

    fn floattostring(&mut self, var: i32) -> Vec<u8> {
        // ZenGin formats external float results like printf `%f`
        let content = format!("{:.6}", f32::from_bits(var as u32)).into_bytes();
        self.record_dynamic_string(content.clone());
        content
    }

    fake_impl!(floattoint(_0) -> i32);
    fake_impl!(print(_0));
    fake_impl!(printdebug(_0));
    /// Shows a message on the screen for `time` seconds. The engine queues the
    /// `ai_` variant into the npc AI queue, the viewer shows both immediately.
    /// The font argument is ignored, the viewer uses its default UI font.
    fn printscreen(&mut self, msg: i32, pos_x: i32, pos_y: i32, _font: i32, time_sec: i32) -> i32 {
        self.push_screen_message(msg, pos_x, pos_y, time_sec);
        0
    }
    fn ai_printscreen(
        &mut self,
        msg: i32,
        pos_x: i32,
        pos_y: i32,
        _font: i32,
        time_sec: i32,
    ) -> i32 {
        self.push_screen_message(msg, pos_x, pos_y, time_sec);
        0
    }
    fake_impl!(printdialog(_0, _1, _2, _3, _4, _5) -> i32);
    fake_impl!(printdebuginst(_0));
    fake_impl!(printdebuginstch(_0, _1));
    fake_impl!(printdebugch(_0, _1));
    fake_impl!(printmulti(_0, _1, _2, _3, _4));
    fake_impl!(exitgame());
    fake_impl!(exitsession());
    fake_impl!(playvideo(_0) -> i32);
    fake_impl!(playvideoex(_0, _1, _2) -> i32);
    fake_impl!(setpercentdone(_0));
    fake_impl!(introducechapter(_0, _1, _2, _3, _4));
    // Menu script (MENU.DAT) externals: options apply callbacks and the
    // choicebox update used by settings menus. Not implemented yet.
    fake_impl!(apply_options_audio());
    fake_impl!(apply_options_controls());
    fake_impl!(apply_options_game());
    fake_impl!(apply_options_performance());
    fake_impl!(apply_options_video());
    fake_impl!(update_choicebox(_0));
    fake_impl!(npc_memoryentry(_0, _1, _2, _3, _4));
    fake_impl!(npc_memoryentryguild(_0, _1, _2, _3, _4));
    fake_impl!(npc_hasnews(_0, _1, _2, _3) -> i32);
    fake_impl!(npc_getnewswitness(_0, _1) -> i32);
    fake_impl!(npc_getnewsoffender(_0, _1) -> i32);
    fake_impl!(npc_getnewsvictim(_0, _1) -> i32);
    fake_impl!(npc_isnewsgossip(_0, _1) -> i32);
    fake_impl!(npc_deletenews(_0, _1) -> i32);
    fake_impl!(npc_getguildattitude(_0, _1) -> i32);
    fake_impl!(npc_getactivespell(_0) -> i32);
    fake_impl!(npc_getlasthitspellid(_0) -> i32);
    fake_impl!(npc_getlasthitspellcat(_0) -> i32);
    fake_impl!(npc_getactivespellcat(_0) -> i32);
    fake_impl!(npc_getactivespelllevel(_0) -> i32);
    fake_impl!(npc_getactivespellisscroll(_0) -> i32);
    fake_impl!(npc_setactivespellinfo(_0, _1) -> i32);
    fake_impl!(wld_detectitem(_0, _1) -> i32);
    fake_impl!(wld_detectplayer(_0) -> i32);
    fake_impl!(wld_detectnpc(_0, _1, _2, _3) -> i32);
    fake_impl!(wld_detectnpcex(_0, _1, _2, _3, _4) -> i32);
    fake_impl!(wld_detectnpcexatt(_0, _1, _2, _3, _4, _5) -> i32);
    fn wld_setguildattitude(&mut self, guild_a: i32, guild_b: i32, attitude: i32) {
        self.guild_attitudes.insert((guild_a, guild_b), attitude);
    }
    fake_impl!(wld_getguildattitude(_0, _1) -> i32);
    /// Minimal mob availability: all mobs count as available so routine states
    /// reach their use-mob branch (real mob objects are not simulated)
    fn wld_ismobavailable(&mut self, _npc: i32, _mob_name: i32) -> i32 {
        warn_unimplemented!("wld_ismobavailable");
        1
    }
    fake_impl!(wld_isfpavailable(_0, _1) -> i32);
    fake_impl!(wld_isnextfpavailable(_0, _1) -> i32);
    fake_impl!(npc_createspell(_0, _1));
    fake_impl!(npc_learnspell(_0, _1));

    fn npc_hasitems(&mut self, npc: i32, item: i32) -> i32 {
        let Some((npc, item)) = self.npc_item_pair(npc, item) else {
            return 0;
        };
        self.loaded_state
            .npc_instances
            .get(&npc)
            .map_or(0, |npc| npc.inventory.count(item))
    }
    fn npc_giveitem(&mut self, npc: i32, item: i32, to: i32) {
        let Some(from) = self.symbol_table.instance(npc) else {
            return;
        };
        let Some(item) = self.symbol_table.instance(item) else {
            return;
        };
        let Some(to) = self.symbol_table.instance(to) else {
            return;
        };
        let count = self
            .loaded_state
            .npc_instances
            .get_mut(&from)
            .map_or(0, |npc| npc.inventory.take(item));
        if count <= 0 {
            return;
        }
        self.loaded_state
            .npc_instances
            .entry(to)
            .or_default()
            .inventory
            .create(item, count);
        if self.player_instance == Some(to) {
            self.pending.inventory_gains.push(InventoryGain {
                npc: to,
                item,
                count,
            });
        }
    }
    fn createinvitem(&mut self, npc: i32, item: i32) {
        self.createinvitems(npc, item, 1);
    }
    fn createinvitems(&mut self, npc: i32, item: i32, count: i32) {
        let Some((npc, item)) = self.npc_item_pair(npc, item) else {
            return;
        };
        self.loaded_state
            .npc_instances
            .entry(npc)
            .or_default()
            .inventory
            .create(item, count);
        if self.player_instance == Some(npc) {
            self.pending
                .inventory_gains
                .push(InventoryGain { npc, item, count });
        }
    }
    /// Records the item equipped on the npc; the mesh is attached by an ECS
    /// system. Works both at instance init and at runtime. The weapon slot
    /// (melee vs ranged) is resolved from the item symbol name; the name comes
    /// from the preloaded string table, so no guest re-entry is needed here.
    fn equipitem(&mut self, npc: i32, item: i32) {
        let item_raw = item;
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        let Some(item) = self.symbol_table.instance(item) else {
            return;
        };
        let kind = self
            .symbol_table
            .symbol_name(item_raw)
            .as_deref()
            .and_then(WeaponKind::from_symbol_name);
        let Some(kind) = kind else {
            warn!(
                "EquipItem: item({}) has no known weapon kind, not equipped",
                item_raw
            );
            return;
        };
        let instance = self.loaded_state.npc_instances.entry(npc).or_default();
        match kind {
            WeaponKind::Melee => instance.equipped_melee = Some(item),
            WeaponKind::Bow | WeaponKind::Crossbow => instance.equipped_ranged = Some(item),
        }
        self.pending
            .equip_item_changes
            .push(EquipItemChange { instance_id: npc });
    }
    fake_impl!(npc_getinvitem(_0, _1) -> i32);
    fake_impl!(npc_getinvitembyslot(_0, _1, _2) -> i32);
    fake_impl!(npc_removeinvitem(_0, _1) -> i32);
    /// Removes up to `amount` of the item from the inventory, returns how many
    /// were actually removed. No effect when `amount < 1`.
    fn npc_removeinvitems(&mut self, npc: i32, item: i32, amount: i32) -> i32 {
        let Some((npc, item)) = self.npc_item_pair(npc, item) else {
            return 0;
        };
        self.loaded_state
            .npc_instances
            .get_mut(&npc)
            .map_or(0, |npc| npc.inventory.remove(item, amount))
    }
    fake_impl!(npc_clearinventory(_0));
    fn npc_isinstate(&mut self, npc: i32, state: i32) -> i32 {
        self.is_in_state(npc, state, false)
    }
    fn npc_wasinstate(&mut self, npc: i32, state: i32) -> i32 {
        self.is_in_state(npc, state, true)
    }
    fake_impl!(npc_isinroutine(_0, _1) -> i32);
    fake_impl!(ai_setnpcstostate(_0, _1, _2));
    fake_impl!(npc_hasdetectednpc(_0, _1) -> i32);
    fn npc_setattitude(&mut self, npc: i32, attitude: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        self.loaded_state
            .npc_instances
            .entry(npc)
            .or_default()
            .attitude = Some(attitude);
    }
    fn npc_settempattitude(&mut self, npc: i32, attitude: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        self.loaded_state
            .npc_instances
            .entry(npc)
            .or_default()
            .temp_attitude = Some(attitude);
    }
    fake_impl!(npc_getattitude(_0, _1) -> i32);
    fake_impl!(npc_getpermattitude(_0, _1) -> i32);
    /// Sets an npc attribute (`Npc_ChangeAttribute`). Only the hitpoints
    /// attributes are tracked, other attributes are ignored.
    fn npc_changeattribute(&mut self, npc: i32, atr: i32, value: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        // ATR_HITPOINTS = 0, ATR_HITPOINTS_MAX = 1
        if atr != 0 && atr != 1 {
            return;
        }
        let instance = self.loaded_state.npc_instances.entry(npc).or_default();
        if atr == 0 {
            instance.hitpoints = Some(value);
        } else {
            instance.hitpoints_max = Some(value);
        }
        self.pending
            .health_changes
            .push(HealthChange { instance_id: npc });
    }
    fake_impl!(npc_getcomrades(_0) -> i32);
    fake_impl!(npc_isnear(_0, _1) -> i32);
    /// Distance between two npcs in engine units, `i32::MAX` when either has no
    /// entity (unknown or dead).
    fn npc_getdisttonpc(&mut self, npc_a: i32, npc_b: i32) -> i32 {
        let Some(npc_a) = self.symbol_table.instance(npc_a) else {
            return i32::MAX;
        };
        let Some(npc_b) = self.symbol_table.instance(npc_b) else {
            return i32::MAX;
        };
        let instances = &self.loaded_state.npc_instances;
        let (Some(aa), Some(bb)) = (
            instances.get(&npc_a).and_then(|npc| npc.position),
            instances.get(&npc_b).and_then(|npc| npc.position),
        ) else {
            return i32::MAX;
        };
        let distance = to_zengin_units((aa - bb).length());
        distance.min(i32::MAX as f32) as i32
    }
    fake_impl!(npc_getheighttonpc(_0, _1) -> i32);
    fake_impl!(npc_getheighttoitem(_0, _1) -> i32);
    fake_impl!(npc_getdisttoplayer(_0) -> i32);
    fake_impl!(npc_getdisttoitem(_0, _1) -> i32);
    fake_impl!(npc_canseenpc(_0, _1) -> i32);
    fake_impl!(npc_canseenpcfreelos(_0, _1) -> i32);
    fake_impl!(npc_canseeitem(_0, _1) -> i32);
    /// True when the npc is the recorded player instance (`PC_Hero`).
    fn npc_isplayer(&mut self, npc: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return 0;
        };
        i32::from(self.player_instance == Some(npc))
    }
    fake_impl!(npc_knowsplayer(_0, _1) -> i32);
    fake_impl!(npc_setknowsplayer(_0, _1));
    fake_impl!(npc_isinfightmode(_0, _1) -> i32);
    fake_impl!(npc_isaiming(_0, _1) -> i32);
    fake_impl!(npc_startitemreactmodules(_0, _1, _2) -> i32);
    fake_impl!(npc_hasoffered(_0, _1, _2) -> i32);
    fn npc_settofistmode(&mut self, npc: i32) {
        self.set_weapon_mode(npc, WeaponMode::Fists);
    }
    fn npc_settofightmode(&mut self, npc: i32, _weapon: i32) {
        self.set_weapon_mode(npc, WeaponMode::Melee);
    }
    /// An npc has no entity (or the id is not an npc) means dead.
    fn npc_isdead(&mut self, npc: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return 1;
        };
        let alive = self
            .loaded_state
            .npc_instances
            .get(&npc)
            .is_some_and(|npc| npc.position.is_some());
        i32::from(!alive)
    }
    fake_impl!(ai_startstate(_0, _1, _2, _3));
    fake_impl!(ai_continueroutine(_0));
    fn npc_getstatetime(&mut self, npc: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return 0;
        };
        self.loaded_state
            .npc_instances
            .get(&npc)
            .map_or(0, |npc| npc.state_time)
    }
    fn npc_setstatetime(&mut self, npc: i32, seconds: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        self.loaded_state
            .npc_instances
            .entry(npc)
            .or_default()
            .state_time = seconds;
    }
    fake_impl!(ai_outputsvm(_0, _1, _2));
    fake_impl!(ai_outputsvm_overlay(_0, _1, _2));
    fake_impl!(ai_playcutscene(_0, _1));
    fake_impl!(ai_canseenpc(_0, _1, _2));
    fake_impl!(ai_wait(_0, _1));
    fake_impl!(ai_waitms(_0, _1));
    fake_impl!(ai_waittillend(_0, _1));
    /// Requests that the npc faces its routine waypoint, applied by the ECS side.
    fn ai_aligntowp(&mut self, npc: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        self.pending
            .align_to_wp_changes
            .push(AlignToWpChange { instance_id: npc });
    }
    fake_impl!(ai_setwalkmode(_0, _1));
    /// Records a `AI_GoToWP` request; the waypoint name is stored as a raw string
    /// id and resolved by the ECS side (see `ai_applyoverlay` for why).
    fn ai_gotowp(&mut self, npc: i32, waypoint: i32) {
        self.set_goto_request(npc, waypoint);
    }
    fake_impl!(ai_teleport(_0, _1));
    fake_impl!(ai_gotoitem(_0, _1));
    fake_impl!(ai_gotonpc(_0, _1));
    fake_impl!(ai_aligntofp(_0));
    fake_impl!(ai_gotofp(_0, _1));
    fake_impl!(npc_isonfp(_0, _1) -> i32);
    fake_impl!(ai_gotonextfp(_0, _1));
    fake_impl!(ai_gotosound(_0));
    fake_impl!(ai_takeitem(_0, _1));
    fake_impl!(ai_dropitem(_0, _1));
    fake_impl!(ai_drawweapon(_0));
    fn ai_removeweapon(&mut self, npc: i32) {
        self.set_weapon_mode(npc, WeaponMode::Fists);
    }
    fake_impl!(npc_hasrangedweaponwithammo(_0) -> i32);
    fake_impl!(npc_hasequippedweapon(_0) -> i32);
    fn npc_hasequippedmeleeweapon(&mut self, npc: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return 0;
        };
        self.loaded_state
            .npc_instances
            .get(&npc)
            .map_or(0, |npc| i32::from(npc.equipped_melee.is_some()))
    }
    fake_impl!(npc_hasequippedrangedweapon(_0) -> i32);
    fake_impl!(npc_hasequippedarmor(_0) -> i32);
    fake_impl!(npc_hasreadiedweapon(_0) -> i32);
    fake_impl!(npc_hasreadiedmeleeweapon(_0) -> i32);
    fake_impl!(npc_hasreadiedrangedweapon(_0) -> i32);
    fake_impl!(npc_gettrueguild(_0) -> i32);
    fake_impl!(npc_settrueguild(_0, _1) -> i32);
    fake_impl!(npc_arewestronger(_0, _1) -> i32);
    fake_impl!(npc_gettarget(_0) -> i32);
    fake_impl!(npc_getnexttarget(_0) -> i32);
    /// Npc currently looked at, `0` when none.
    fn npc_getlookattarget(&mut self, npc: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return 0;
        };
        self.loaded_state
            .npc_instances
            .get(&npc)
            .and_then(|npc| npc.look_at)
            .map_or(0, super::symbol_id::InstanceId::value)
    }
    fake_impl!(npc_isnexttargetavailable(_0) -> i32);
    fake_impl!(npc_settarget(_0, _1));
    fake_impl!(npc_getbodystate(_0) -> i32);
    fake_impl!(npc_hasbodyflag(_0, _1) -> i32);
    fake_impl!(npc_isincutscene(_0) -> i32);
    fake_impl!(npc_isvoiceactive(_0) -> i32);
    fake_impl!(npc_stopani(_0, _1));
    fake_impl!(npc_getdetectedmob(_0) -> i32);
    fake_impl!(npc_playani(_0, _1));
    fake_impl!(npc_isdetectedmobownedbynpc(_0, _1) -> i32);
    fake_impl!(npc_isdetectedmobownedbyguild(_0, _1) -> i32);
    fake_impl!(wld_getmobstate(_0, _1) -> i32);
    fake_impl!(npc_clearaiqueue(_0));
    fake_impl!(infomanager_hasfinished() -> i32);
    fake_impl!(ai_processinfos(_0));

    /// Knowledge is gained when a dialog root is played.
    fn npc_knowsinfo(&mut self, npc: i32, info: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return 0;
        };
        let Some(info) = self.symbol_table.instance(info) else {
            return 0;
        };
        i32::from(
            self.loaded_state
                .npc_instances
                .get(&npc)
                .is_some_and(|npc| npc.known_infos.contains(&info)),
        )
    }
    fake_impl!(npc_checkinfo(_0, _1) -> i32);
    fake_impl!(npc_giveinfo(_0, _1) -> i32);
    fn npc_gettalentskill(&mut self, npc: i32, talent: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return 0;
        };
        self.loaded_state
            .npc_instances
            .get(&npc)
            .map_or(0, |npc| npc.talents.skill(talent))
    }
    fn npc_gettalentvalue(&mut self, npc: i32, talent: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return 0;
        };
        self.loaded_state
            .npc_instances
            .get(&npc)
            .map_or(0, |npc| npc.talents.value(talent))
    }
    fn npc_settalentskill(&mut self, npc: i32, talent: i32, level: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        self.loaded_state
            .npc_instances
            .entry(npc)
            .or_default()
            .talents
            .set_skill(talent, level);
    }
    fn npc_settalentvalue(&mut self, npc: i32, talent: i32, level: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        self.loaded_state
            .npc_instances
            .entry(npc)
            .or_default()
            .talents
            .set_value(talent, level);
    }
    fake_impl!(tal_configure(_0, _1));
    /// Name of the nearest waypoint by lowercased key (the names scripts use
    /// for waypoint lookups are canonical, see `get_canon_string`), empty when
    /// the npc has no known position or the way net has no waypoints
    fn npc_getnearestwp(&mut self, npc: i32) -> Vec<u8> {
        let content = self.nearest_way_point(npc).unwrap_or_default().into_bytes();
        self.record_dynamic_string(content.clone());
        content
    }
    fake_impl!(npc_getnextwp(_0) -> i32);
    fake_impl!(npc_iswayblocked(_0) -> i32);
    fake_impl!(npc_getreadiedweapon(_0) -> i32);
    fake_impl!(npc_getequippedmeleeweapon(_0) -> i32);
    fake_impl!(npc_getequippedrangedweapon(_0) -> i32);
    fake_impl!(npc_getequippedarmor(_0) -> i32);
    fake_impl!(ai_equipbestmeleeweapon(_0) -> i32);
    fake_impl!(ai_equipbestrangedweapon(_0) -> i32);
    fake_impl!(ai_equipbestarmor(_0));
    fake_impl!(ai_unequipweapons(_0));
    fake_impl!(ai_unequiparmor(_0));
    fake_impl!(ai_equiparmor(_0, _1));
    fn ai_readymeleeweapon(&mut self, npc: i32) {
        self.set_weapon_mode(npc, WeaponMode::Melee);
    }
    /// The overlay mds name is stored as a raw string id and resolved later,
    /// because this may run inside the script component (instance init) where
    /// reading the symbol name would trap.
    fn mdl_applyoverlaymds(&mut self, npc: i32, overlay: i32) {
        self.set_overlay_request(npc, OverlayRequest::Apply(overlay));
    }
    fn mdl_applyoverlaymdstimed(&mut self, npc: i32, overlay: i32, _ticks: i32) {
        self.set_overlay_request(npc, OverlayRequest::Apply(overlay));
    }
    fn mdl_removeoverlaymds(&mut self, npc: i32, _overlay: i32) {
        self.set_overlay_request(npc, OverlayRequest::Remove);
    }
    fake_impl!(ai_readyrangedweapon(_0));
    fake_impl!(ai_turnaway(_0, _1));
    fake_impl!(ai_turntonpc(_0, _1));
    fake_impl!(ai_whirlaround(_0, _1));
    fake_impl!(ai_whirlaroundtosource(_0));
    fake_impl!(ai_turntosound(_0));
    fake_impl!(ai_quicklook(_0, _1));
    fake_impl!(ai_lookat(_0, _1));
    fn ai_lookatnpc(&mut self, npc: i32, target: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        let Some(target) = self.symbol_table.instance(target) else {
            return;
        };
        self.loaded_state
            .npc_instances
            .entry(npc)
            .or_default()
            .look_at = Some(target);
        self.pending
            .look_at_changes
            .push(NpcLookAtChange { instance_id: npc });
    }
    fn ai_stoplookat(&mut self, npc: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        let Some(entry) = self.loaded_state.npc_instances.get_mut(&npc) else {
            return;
        };
        entry.look_at = None;
        self.pending
            .look_at_changes
            .push(NpcLookAtChange { instance_id: npc });
    }
    fake_impl!(ai_pointat(_0, _1));
    fake_impl!(ai_pointatnpc(_0, _1));
    fake_impl!(ai_stoppointat(_0));
    fake_impl!(ai_standup(_0));
    fake_impl!(ai_standupquick(_0));
    fake_impl!(ai_flee(_0));
    fake_impl!(ai_aimat(_0, _1));
    fake_impl!(ai_stopaim(_0));
    fake_impl!(ai_shootat(_0, _1));
    fake_impl!(ai_defend(_0));
    fake_impl!(ai_combatreacttodamage(_0));
    fake_impl!(ai_readyspell(_0, _1, _2));
    fake_impl!(ai_unreadyspell(_0));
    fake_impl!(npc_hasspell(_0, _1) -> i32);
    fake_impl!(ai_attack(_0));
    fake_impl!(ai_finishingmove(_0, _1));
    fake_impl!(ai_dodge(_0));
    fake_impl!(ai_useitem(_0, _1));
    fake_impl!(ai_useitemtostate(_0, _1, _2));
    fake_impl!(npc_refusetalk(_0) -> i32);
    fake_impl!(npc_setrefusetalk(_0, _1));
    fake_impl!(ai_ask(_0, _1, _2));
    fake_impl!(ai_asktext(_0, _1, _2, _3, _4));
    fake_impl!(ai_waitforquestion(_0, _1));
    fake_impl!(hlp_isitem(_0, _1) -> i32);
    fake_impl!(hlp_getinstanceid(_0) -> i32);
    /// Random integer in `[0, bound)`, deterministic.
    fn hlp_random(&mut self, bound: i32) -> i32 {
        let bound = bound.max(1) as u32;
        (self.rng.next() % bound) as i32
    }
    /// Resolves an instance symbol to its npc id, scripts use this to reference
    /// other npcs. `0` when the argument is not an instance symbol (the value
    /// downstream `npc_*` host calls treat as "no npc").
    fn hlp_getnpc(&mut self, instance_symbol: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(instance_symbol) else {
            return 0;
        };
        npc.value()
    }
    /// The viewer has no separate npc handle table, an npc argument is valid when
    /// it resolves to an instance symbol (the resolution every other `npc_*` host
    /// call performs).
    fn hlp_isvalidnpc(&mut self, npc: i32) -> i32 {
        i32::from(self.symbol_table.instance(npc).is_some())
    }
    fake_impl!(hlp_isvaliditem(_0) -> i32);
    fake_impl!(ai_snd_play(_0, _1));
    fake_impl!(ai_snd_play3d(_0, _1, _2));
    fake_impl!(snd_play(_0));
    fake_impl!(snd_play3d(_0, _1));
    fake_impl!(snd_getdisttosource(_0) -> i32);
    fake_impl!(snd_issourcenpc(_0) -> i32);
    fake_impl!(snd_issourceitem(_0) -> i32);
    fake_impl!(npc_canseesource(_0) -> i32);
    fake_impl!(mis_getstatus(_0) -> i32);
    fake_impl!(mis_setstatus(_0, _1));
    fake_impl!(mis_ontime(_0) -> i32);
    /// Creates a journal topic in the given section (`Log_CreateTopic`).
    fn log_createtopic(&mut self, topic: i32, section: i32) {
        let Some(topic) = self.get_string_non_0(topic) else {
            return;
        };
        let Some(section) = JournalSection::from_i32(section) else {
            return;
        };
        if self.journal.create_topic(topic.clone(), section) {
            self.pending.journal_changes.push(JournalChange {
                topic,
                action: JournalAction::Created { section },
            });
        }
    }
    /// Appends an entry to a journal topic (`Log_AddEntry`).
    fn log_addentry(&mut self, topic: i32, entry: i32) {
        let Some(topic) = self.get_string_non_0(topic) else {
            return;
        };
        let Some(entry) = self.get_string_non_0(entry) else {
            return;
        };
        if self.journal.add_entry(&topic, entry.clone()) {
            self.pending.journal_changes.push(JournalChange {
                topic,
                action: JournalAction::Entry(entry),
            });
        }
    }
    /// Sets the status of a journal topic (`Log_SetTopicStatus`).
    fn log_settopicstatus(&mut self, topic: i32, status: i32) {
        let Some(topic) = self.get_string_non_0(topic) else {
            return;
        };
        let Some(status) = JournalStatus::from_i32(status) else {
            return;
        };
        if self.journal.set_topic_status(&topic, status) {
            self.pending.journal_changes.push(JournalChange {
                topic,
                action: JournalAction::Status(status),
            });
        }
    }
    fake_impl!(mis_addmissionentry(_0, _1));
    fake_impl!(mis_removemission(_0));
    fake_impl!(npc_checkavailablemission(_0, _1, _2) -> i32);
    fake_impl!(npc_checkrunningmission(_0, _1) -> i32);
    fake_impl!(npc_isdrawingweapon(_0) -> i32);
    fake_impl!(npc_isdrawingspell(_0) -> i32);
    fake_impl!(npc_checkoffermission(_0, _1) -> i32);
    fake_impl!(mdl_applyrandomani(_0, _1, _2));
    fake_impl!(mdl_applyrandomanifreq(_0, _1, _2));
    /// Floats are passed as their bit patterns.
    fn mdl_setmodelscale(&mut self, npc_index: i32, x: i32, y: i32, z: i32) {
        let Some(npc_index) = self.symbol_table.instance(npc_index) else {
            return;
        };
        let scale = Vec3::new(
            f32::from_bits(x as u32),
            f32::from_bits(y as u32),
            f32::from_bits(z as u32),
        );
        self.loaded_state
            .npc_instances
            .entry(npc_index)
            .or_default()
            .model_scale = Some(scale);
    }
    /// Floats are passed as their bit patterns.
    fn mdl_setmodelfatness(&mut self, npc_index: i32, fatness: i32) {
        let Some(npc_index) = self.symbol_table.instance(npc_index) else {
            return;
        };
        self.loaded_state
            .npc_instances
            .entry(npc_index)
            .or_default()
            .model_fatness = Some(f32::from_bits(fatness as u32));
    }
    fake_impl!(mdl_startfaceani(_0, _1, _2, _3));
    fake_impl!(mdl_applyrandomfaceani(_0, _1, _2, _3, _4, _5, _6));
    fake_impl!(wld_istime(_0, _1, _2, _3) -> i32);
    fake_impl!(wld_getday() -> i32);
    fake_impl!(wld_israining() -> i32);
    fake_impl!(wld_settime(_0, _1));
    fake_impl!(wld_spawnnpcrange(_0, _1, _2, _3));
    fake_impl!(wld_playeffect(_0, _1, _2, _3, _4, _5, _6));
    fake_impl!(wld_stopeffect(_0));
    fake_impl!(ai_playfx(_0, _1, _2));
    fake_impl!(ai_stopfx(_0, _1));
    fake_impl!(wld_removenpc(_0));
    fake_impl!(wld_insertnpcandrespawn(_0, _1, _2));
    fake_impl!(wld_insertobject(_0, _1));
    fake_impl!(wld_removeitem(_0) -> i32);
    fake_impl!(wld_exchangeguildattitudes(_0));
    fake_impl!(wld_setobjectroutine(_0, _1, _2, _3));
    /// Records the mob state assignment for later consumers, mobs are not
    /// simulated yet so nothing acts on the stored entries
    fn wld_setmobroutine(&mut self, hour: i32, minute: i32, mob_name: i32, state: i32) {
        let (hour, minute, state) = (hour, minute, state);
        let Some(mob_name_id) = self.symbol_table.string(mob_name) else {
            warn_unimplemented!("wld_setmobroutine with non-string mob name({mob_name})");
            return;
        };
        let Some(mob_name) = self.get_string_non_0(mob_name_id.value()) else {
            warn_unimplemented!("wld_setmobroutine with empty mob name");
            return;
        };
        self.loaded_state.mob_routines.push(MobRoutineEntry {
            hour,
            minute,
            mob_name,
            state,
        });
    }
    fake_impl!(wld_sendtrigger(_0));
    fake_impl!(wld_senduntrigger(_0));
    fake_impl!(npc_ownedbynpc(_0, _1) -> i32);
    fake_impl!(npc_ownedbyguild(_0, _1) -> i32);
    fake_impl!(ai_takemob(_0, _1));
    fake_impl!(ai_dropmob(_0));
    fake_impl!(mob_createitems(_0, _1, _2));
    fake_impl!(mob_hasitems(_0, _1) -> i32);
    fake_impl!(doc_create() -> i32);
    fake_impl!(doc_createmap() -> i32);
    fake_impl!(doc_setpages(_0, _1));
    fake_impl!(doc_setpage(_0, _1, _2, _3));
    fake_impl!(doc_setfont(_0, _1, _2));
    fake_impl!(doc_setlevel(_0, _1));
    fake_impl!(doc_setlevelcoords(_0, _1, _2, _3, _4));
    fake_impl!(doc_setmargins(_0, _1, _2, _3, _4, _5, _6));
    fake_impl!(doc_printline(_0, _1, _2));
    fake_impl!(doc_printlines(_0, _1, _2));
    fake_impl!(doc_show(_0));
    fake_impl!(doc_open(_0));
    fake_impl!(doc_font(_0));
    fake_impl!(doc_print(_0));
    fake_impl!(doc_mapcoordinates(_0, _1, _2, _3, _4, _5, _6, _7, _8));
    fake_impl!(ta(_0, _1, _2, _3, _4));
    fake_impl!(ta_cs(_0, _1, _2));
    fake_impl!(ta_beginoverlay(_0));
    fake_impl!(ta_endoverlay(_0));
    fake_impl!(ta_removeoverlay(_0));
    /// Records a routine exchange request; the `Rtn_{name}_{npcId}` function is
    /// resolved and run outside of the script execution by the script manager.
    fn npc_exchangeroutine(&mut self, npc: i32, routine_name: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        let Some(routine_name) = self.symbol_table.string(routine_name) else {
            return;
        };
        let Some(name) = self.get_string_non_0(routine_name.value()) else {
            return;
        };
        self.pending.routine_exchanges.push(RoutineExchange {
            instance_id: npc,
            routine_name: name,
        });
    }
    fake_impl!(rtn_exchange(_0, _1));
    fake_impl!(perc_setrange(_0, _1));
    fake_impl!(npc_setperctime(_0, _1));
    fake_impl!(npc_percenable(_0, _1, _2));
    fake_impl!(npc_percdisable(_0, _1));
    fake_impl!(npc_sendpassiveperc(_0, _1, _2, _3));
    fake_impl!(npc_sendsingleperc(_0, _1, _2));
    fake_impl!(npc_perceiveall(_0));
    fake_impl!(wld_assignroomtoguild(_0, _1));
    fake_impl!(wld_assignroomtonpc(_0, _1));
    fake_impl!(wld_getplayerportalowner() -> i32);
    fake_impl!(wld_getplayerportalguild() -> i32);
    fake_impl!(wld_getformerplayerportalowner() -> i32);
    fake_impl!(wld_getformerplayerportalguild() -> i32);
    fake_impl!(npc_isplayerinmyroom(_0) -> i32);
    fake_impl!(npc_isinplayersroom(_0) -> i32);
    fake_impl!(npc_wasplayerinmyroom(_0) -> i32);
    fake_impl!(npc_getportalowner(_0) -> i32);
    fake_impl!(npc_getportalguild(_0) -> i32);
    fake_impl!(hlp_cutsceneplayed(_0) -> i32);
    fake_impl!(game_initgerman() -> i32);
    fake_impl!(game_initenglish() -> i32);
    fake_impl!(game_initengintl() -> i32);
}
