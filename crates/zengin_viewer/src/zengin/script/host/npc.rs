use super::*;

impl mmcomando::zengine_scripts::npc_scripts::Host for ImportCallState {
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
        let Some(position) = self.npc_state(npc).and_then(|npc| npc.live.position) else {
            return i32::MAX;
        };

        // Positions are stored in Bevy meters, the engine works in game units
        let distance = to_zengin_units((position - target).length());
        distance.min(i32::MAX as f32) as i32
    }
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
    fake_impl!(npc_createspell(_0, _1));
    fake_impl!(npc_learnspell(_0, _1));
    fn npc_hasitems(&mut self, npc: i32, item: i32) -> i32 {
        let Some((npc, item)) = self.npc_item_pair(npc, item) else {
            return 0;
        };
        self.npc_state(npc)
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
        let count = self.npc_state_mut(from).inventory.take(item);
        if count <= 0 {
            return;
        }
        self.npc_state_mut(to).inventory.create(item, count);
        if self.player_instance == Some(to) {
            self.messages.inventory_gains.send(InventoryGain {
                npc: to,
                item,
                count,
            });
        }
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
        self.npc_state_mut(npc).inventory.remove(item, amount)
    }
    fake_impl!(npc_clearinventory(_0));
    fn npc_isinstate(&mut self, npc: i32, state: i32) -> i32 {
        self.is_in_state(npc, state, false)
    }
    fn npc_wasinstate(&mut self, npc: i32, state: i32) -> i32 {
        self.is_in_state(npc, state, true)
    }
    fake_impl!(npc_isinroutine(_0, _1) -> i32);
    fake_impl!(npc_hasdetectednpc(_0, _1) -> i32);
    fn npc_setattitude(&mut self, npc: i32, attitude: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        self.npc_state_mut(npc).social.attitude = Some(attitude);
    }
    fn npc_settempattitude(&mut self, npc: i32, attitude: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        self.npc_state_mut(npc).social.temp_attitude = Some(attitude);
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
        let Some(handle) = self
            .typed
            .instance(npc.value() as u32)
            .and_then(daedalus_typed::TypedInstance::npc)
            .cloned()
        else {
            return;
        };
        {
            let mut data = handle
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if atr == 0 {
                data.hitpoints = value;
            } else {
                data.hitpoints_max = value;
            }
        }
        self.pending.npc_changed.insert(npc);
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
        let aa = self.npc_state(npc_a).and_then(|npc| npc.live.position);
        let bb = self.npc_state(npc_b).and_then(|npc| npc.live.position);
        let (Some(aa), Some(bb)) = (aa, bb) else {
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
            .npc_state(npc)
            .is_some_and(|npc| !npc.live.dead && npc.live.position.is_some());
        i32::from(!alive)
    }
    fn npc_getstatetime(&mut self, npc: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return 0;
        };
        self.npc_state(npc).map_or(0, |npc| npc.routine.state_time)
    }
    fn npc_setstatetime(&mut self, npc: i32, seconds: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        self.npc_state_mut(npc).routine.state_time = seconds;
    }
    /// Whether the npc stands on a freepoint whose name contains the value.
    fn npc_isonfp(&mut self, npc: i32, fp: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return 0;
        };
        let Some(query) = self.canon_string_arg(fp) else {
            return 0;
        };
        i32::from(self.npc_is_on_freepoint(npc, query.as_str()))
    }
    fake_impl!(npc_hasrangedweaponwithammo(_0) -> i32);
    fake_impl!(npc_hasequippedweapon(_0) -> i32);
    fn npc_hasequippedmeleeweapon(&mut self, npc: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return 0;
        };
        self.npc_state(npc)
            .map_or(0, |npc| i32::from(npc.equipment.melee.is_some()))
    }
    fake_impl!(npc_hasequippedrangedweapon(_0) -> i32);
    fake_impl!(npc_hasequippedarmor(_0) -> i32);
    fake_impl!(npc_hasreadiedweapon(_0) -> i32);
    fake_impl!(npc_hasreadiedmeleeweapon(_0) -> i32);
    fake_impl!(npc_hasreadiedrangedweapon(_0) -> i32);
    fake_impl!(npc_gettrueguild(_0) -> i32);
    fake_impl!(npc_settrueguild(_0, _1) -> i32);
    fake_impl!(npc_arewestronger(_0, _1) -> i32);
    /// Target the ai currently attacks (`Npc_SetTarget`), `0` when none.
    fn npc_gettarget(&mut self, npc: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return 0;
        };
        self.npc_state(npc)
            .and_then(|npc| npc.live.target)
            .map_or(0, InstanceId::value)
    }
    fake_impl!(npc_getnexttarget(_0) -> i32);
    /// Npc currently looked at, `0` when none.
    fn npc_getlookattarget(&mut self, npc: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return 0;
        };
        self.npc_state(npc)
            .and_then(|npc| npc.live.look_at)
            .map_or(0, InstanceId::value)
    }
    fake_impl!(npc_isnexttargetavailable(_0) -> i32);
    /// Records the npc target later attacked by `AI_Attack`/`AI_StartState`.
    fn npc_settarget(&mut self, npc: i32, target: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            warn!("Npc_SetTarget with invalid npc instance({npc})");
            return;
        };
        let Some(target) = self.symbol_table.instance(target) else {
            warn!(
                "Npc_SetTarget npc({}) with invalid target instance({target})",
                npc.value()
            );
            return;
        };
        self.npc_state_mut(npc).live.target = Some(target);
        bevy::log::debug!(
            "Npc_SetTarget npc({}) target({})",
            npc.value(),
            target.value()
        );
    }
    fn npc_getbodystate(&mut self, npc: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return BS_STAND;
        };
        self.npc_state(npc)
            .map_or(BS_STAND, |npc| current_body_state(&npc.live))
    }
    fn npc_hasbodyflag(&mut self, npc: i32, flag: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return 0;
        };
        let state = self
            .npc_state(npc)
            .map_or(BS_STAND, |npc| current_body_state(&npc.live));
        i32::from(has_body_flag(state, flag))
    }
    fake_impl!(npc_isincutscene(_0) -> i32);
    fake_impl!(npc_isvoiceactive(_0) -> i32);
    fake_impl!(npc_stopani(_0, _1));
    fake_impl!(npc_getdetectedmob(_0) -> i32);
    fake_impl!(npc_playani(_0, _1));
    fake_impl!(npc_isdetectedmobownedbynpc(_0, _1) -> i32);
    fake_impl!(npc_isdetectedmobownedbyguild(_0, _1) -> i32);
    /// Drops all actions queued by the ai (`Npc_ClearAIQueue`), applied by the
    /// ECS side.
    fn npc_clearaiqueue(&mut self, npc: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            warn!("Npc_ClearAIQueue with invalid npc instance({npc})");
            return;
        };
        bevy::log::debug!("Npc_ClearAIQueue npc({})", npc.value());
        self.messages
            .clear_ai_queue
            .send(ClearAiQueueChange { instance_id: npc });
    }
    /// Knowledge is gained when a dialog root is played.
    fn npc_knowsinfo(&mut self, npc: i32, info: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return 0;
        };
        let Some(info) = self.symbol_table.instance(info) else {
            return 0;
        };
        i32::from(
            self.npc_state(npc)
                .is_some_and(|npc| npc.dialog.known_infos.contains(&info)),
        )
    }
    fake_impl!(npc_checkinfo(_0, _1) -> i32);
    fake_impl!(npc_giveinfo(_0, _1) -> i32);
    fn npc_gettalentskill(&mut self, npc: i32, talent: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return 0;
        };
        self.npc_state(npc)
            .map_or(0, |npc| npc.talents.skill(talent))
    }
    fn npc_gettalentvalue(&mut self, npc: i32, talent: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return 0;
        };
        self.npc_state(npc)
            .map_or(0, |npc| npc.talents.value(talent))
    }
    fn npc_settalentskill(&mut self, npc: i32, talent: i32, level: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        self.npc_state_mut(npc).talents.set_skill(talent, level);
    }
    fn npc_settalentvalue(&mut self, npc: i32, talent: i32, level: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        self.npc_state_mut(npc).talents.set_value(talent, level);
    }
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
    fake_impl!(npc_hasspell(_0, _1) -> i32);
    fake_impl!(npc_refusetalk(_0) -> i32);
    fake_impl!(npc_setrefusetalk(_0, _1));
    fake_impl!(npc_canseesource(_0) -> i32);
    fake_impl!(npc_checkavailablemission(_0, _1, _2) -> i32);
    fake_impl!(npc_checkrunningmission(_0, _1) -> i32);
    fake_impl!(npc_isdrawingweapon(_0) -> i32);
    fake_impl!(npc_isdrawingspell(_0) -> i32);
    fake_impl!(npc_checkoffermission(_0, _1) -> i32);
    fake_impl!(npc_ownedbynpc(_0, _1) -> i32);
    fake_impl!(npc_ownedbyguild(_0, _1) -> i32);
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
    fake_impl!(npc_setperctime(_0, _1));
    fake_impl!(npc_percenable(_0, _1, _2));
    fake_impl!(npc_percdisable(_0, _1));
    fake_impl!(npc_sendpassiveperc(_0, _1, _2, _3));
    fake_impl!(npc_sendsingleperc(_0, _1, _2));
    fake_impl!(npc_perceiveall(_0));
    fake_impl!(npc_isplayerinmyroom(_0) -> i32);
    fake_impl!(npc_isinplayersroom(_0) -> i32);
    fake_impl!(npc_wasplayerinmyroom(_0) -> i32);
    fake_impl!(npc_getportalowner(_0) -> i32);
    fake_impl!(npc_getportalguild(_0) -> i32);
}
