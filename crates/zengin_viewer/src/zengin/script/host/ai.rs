use super::*;

impl mmcomando::zengine_scripts::ai_scripts::Host for ImportCallState {
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
        self.messages.animation.send(AnimationChange {
            instance_id: npc_index,
            animation: StringCanon::new(ani.string()),
        });
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
        self.messages.animation.send(AnimationChange {
            instance_id: npc_index,
            animation: StringCanon::new(ani.string()),
        });
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
        self.messages.dialog_lines.send(DialogLine {
            npc_a,
            npc_b,
            line_id,
            line_str: StringCanon::new(line.string()),
        });
    }
    fn ai_usemob(&mut self, npc_index: i32, mob_name_id: i32, state: i32) -> i32 {
        let Some(npc_index) = self.symbol_table.instance(npc_index) else {
            return 0;
        };
        let Some(mob_name_id) = self.symbol_table.string(mob_name_id) else {
            return 0;
        };
        let Some(mob) = self.get_show_string(mob_name_id) else {
            return 0;
        };
        self.messages.use_mob.send(UseMobChange {
            instance_id: npc_index,
            mob_name: StringCanon::new(mob.string()),
            state,
        });
        0
    }
    fn ai_stopprocessinfos(&mut self, _npc: i32) {
        self.close_dialog = true;
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
    fake_impl!(ai_setnpcstostate(_0, _1, _2));
    /// Starts an AI state through the routine executor (`AI_StartState`).
    fn ai_startstate(&mut self, npc: i32, state: i32, _finalize: i32, waypoint: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            warn!("AI_StartState with invalid npc instance({npc})");
            return;
        };
        let Some(state_fn) = self.symbol_table.function(state) else {
            warn!(
                "AI_StartState npc({}) with invalid state function({state})",
                npc.value()
            );
            return;
        };
        let waypoint = self.symbol_table.string(waypoint).filter(|_| {
            self.symbol_table
                .string_content(waypoint)
                .is_some_and(|wp| !wp.is_empty())
        });
        if let Some(waypoint) = waypoint {
            self.typed
                .set_npc_waypoint(npc.value() as u32, Some(waypoint));
        }
        // The state's partner is the script `other` (the engine's state
        // functions read it), the target is only a fallback.
        let other = self.other_instance().or_else(|| {
            let npc = self.npc_state(npc)?;
            npc.live.target
        });
        // bevy::log::info!(
        //     "AI_StartState npc({}) other({other:?})",
        //     npc.value()
        // );
        self.messages.start_state.send(StartStateChange {
            instance_id: npc,
            state: state_fn,
            other,
        });
    }
    fake_impl!(ai_continueroutine(_0));
    /// Plays the NPC voice line named by the `$`-key (`b_say`). `target` has no
    /// consumer, the speech is positional at the speaking npc.
    fn ai_outputsvm(&mut self, npc: i32, _target: i32, svm_name: i32) {
        self.push_svm_line(npc, svm_name, false);
    }
    /// Same as [`Self::ai_outputsvm`], but overlay lines are skipped while the
    /// previous overlay line is still playing.
    fn ai_outputsvm_overlay(&mut self, npc: i32, _target: i32, svm_name: i32) {
        self.push_svm_line(npc, svm_name, true);
    }
    fake_impl!(ai_playcutscene(_0, _1));
    fake_impl!(ai_canseenpc(_0, _1, _2));
    /// Queues an idle pause of `seconds` before the next ai action. The float
    /// is passed as its bit pattern; non-positive or non-finite values are
    /// ignored.
    fn ai_wait(&mut self, npc: i32, seconds: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            warn!("AI_Wait with invalid npc instance({npc})");
            return;
        };
        let seconds = f32::from_bits(seconds as u32);
        if !seconds.is_finite() || seconds <= 0.0 {
            return;
        }
        let milliseconds = (seconds * 1000.0).min(i32::MAX as f32) as i32;
        self.messages.wait.send(WaitChange {
            instance_id: npc,
            milliseconds,
        });
    }
    /// Queues an idle pause of `milliseconds` before the next ai action.
    fn ai_waitms(&mut self, npc: i32, milliseconds: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            warn!("AI_WaitMS with invalid npc instance({npc})");
            return;
        };
        if milliseconds <= 0 {
            return;
        }
        self.messages.wait.send(WaitChange {
            instance_id: npc,
            milliseconds,
        });
    }
    fake_impl!(ai_waittillend(_0, _1));
    /// Requests that the npc faces its routine waypoint, applied by the ECS side.
    fn ai_aligntowp(&mut self, npc: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        self.messages
            .align_to_wp
            .send(AlignToWpChange { instance_id: npc });
    }
    /// Stores the walk mode (`0` run, `1` walk, `2` sneak). Bit `0x80` is the
    /// weapon flag in the script constants and is masked out; other modes are
    /// ignored, like the engine.
    fn ai_setwalkmode(&mut self, npc: i32, walk_mode: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            warn!("AI_SetWalkMode with invalid npc instance({npc})");
            return;
        };
        let mode = walk_mode & !0x80;
        if !(0..=3).contains(&mode) {
            warn!(
                "AI_SetWalkMode npc({}) with unsupported mode({walk_mode})",
                npc.value()
            );
            return;
        }
        self.npc_state_mut(npc).live.walk_mode = mode;
    }
    /// Records a `AI_GoToWP` request; the waypoint name is stored as a raw string
    /// id and resolved by the ECS side (see `ai_applyoverlay` for why).
    fn ai_gotowp(&mut self, npc: i32, waypoint: i32) {
        self.set_goto_request(npc, waypoint);
    }
    fake_impl!(ai_teleport(_0, _1));
    fake_impl!(ai_gotoitem(_0, _1));
    /// Walks to another npc; the ECS side resolves the target instance to its
    /// entity and follows it until in reach.
    fn ai_gotonpc(&mut self, npc: i32, target: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        let Some(target) = self.symbol_table.instance(target) else {
            return;
        };
        if npc == target {
            return;
        }
        self.messages.goto_npc.send(GotoNpcChange {
            instance_id: npc,
            target,
        });
    }
    /// The walk to the freepoint already orients the npc, nothing to align.
    fn ai_aligntofp(&mut self, _npc: i32) {}
    /// Picks the nearest free freepoint matching the name and orders a walk to
    /// it; the point is held by the npc until its routine state changes.
    fn ai_gotofp(&mut self, npc: i32, waypoint: i32) {
        self.goto_freepoint(npc, waypoint, false);
    }
    /// Like `AI_GotoFP`, skipping the freepoint the npc already uses.
    fn ai_gotonextfp(&mut self, npc: i32, waypoint: i32) {
        self.goto_freepoint(npc, waypoint, true);
    }
    fake_impl!(ai_gotosound(_0));
    fake_impl!(ai_takeitem(_0, _1));
    fake_impl!(ai_dropitem(_0, _1));
    fake_impl!(ai_drawweapon(_0));
    fn ai_removeweapon(&mut self, npc: i32) {
        self.set_weapon_mode(npc, WeaponMode::Fists);
    }
    /// Opens the npc's dialog through the dialog systems, `other` is the talk
    /// partner bound by the calling script state.
    fn ai_processinfos(&mut self, npc: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        let Some(other) = self.other_instance().or(self.player_instance) else {
            warn!(
                "AI_ProcessInfos npc({}) without a talk partner",
                npc.value()
            );
            return;
        };
        self.messages
            .process_infos
            .send(ProcessInfosChange { npc, other });
        bevy::log::debug!(
            "AI_ProcessInfos npc({}) other({})",
            npc.value(),
            other.value()
        );
    }
    fn ai_equipbestmeleeweapon(&mut self, npc: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            warn!("AI_EquipBestMeleeWeapon with invalid npc instance({npc})");
            return 0;
        };
        let Some(item) = self.best_melee_weapon(npc) else {
            bevy::log::debug!(
                "AI_EquipBestMeleeWeapon npc({}) has no melee weapon in its inventory",
                npc.value()
            );
            return 0;
        };
        bevy::log::info!(
            "AI_EquipBestMeleeWeapon npc({}) equips item({})",
            npc.value(),
            item.value()
        );
        self.npc_state_mut(npc).equipment.melee = Some(item);
        0
    }
    fake_impl!(ai_equipbestrangedweapon(_0) -> i32);
    fake_impl!(ai_equipbestarmor(_0));
    fn ai_unequipweapons(&mut self, npc: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            warn!("AI_UnEquipWeapons with invalid npc instance({npc})");
            return;
        };
        crate::test_log!(
            "AI_UnEquipWeapons npc({}) unequips its weapons",
            npc.value()
        );
        self.npc_state_mut(npc).equipment.unequip_weapons();
    }
    fake_impl!(ai_unequiparmor(_0));
    fake_impl!(ai_equiparmor(_0, _1));
    fn ai_readymeleeweapon(&mut self, npc: i32) {
        self.set_weapon_mode(npc, WeaponMode::Melee);
    }
    fake_impl!(ai_readyrangedweapon(_0));
    fake_impl!(ai_turnaway(_0, _1));
    /// Rotates the npc to face the target, applied by the ECS side.
    fn ai_turntonpc(&mut self, npc: i32, target: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        let Some(target) = self.symbol_table.instance(target) else {
            return;
        };
        self.messages.turn_to_npc.send(TurnToNpcChange {
            instance_id: npc,
            target,
        });
    }
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
        self.npc_state_mut(npc).live.look_at = Some(target);
    }
    fn ai_stoplookat(&mut self, npc: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        if self.npc_state(npc).is_none() {
            return;
        }
        self.npc_state_mut(npc).live.look_at = None;
    }
    fake_impl!(ai_pointat(_0, _1));
    fake_impl!(ai_pointatnpc(_0, _1));
    fake_impl!(ai_stoppointat(_0));
    fn ai_standup(&mut self, npc: i32) {
        self.request_standup(npc);
    }
    fn ai_standupquick(&mut self, npc: i32) {
        self.request_standup(npc);
    }
    fake_impl!(ai_flee(_0));
    fake_impl!(ai_aimat(_0, _1));
    fake_impl!(ai_stopaim(_0));
    fake_impl!(ai_shootat(_0, _1));
    fake_impl!(ai_defend(_0));
    fake_impl!(ai_combatreacttodamage(_0));
    fake_impl!(ai_readyspell(_0, _1, _2));
    fake_impl!(ai_unreadyspell(_0));
    /// Requests that the npc attacks its `Npc_SetTarget` target, applied by the
    /// ECS side.
    fn ai_attack(&mut self, npc: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            warn!("AI_Attack with invalid npc instance({npc})");
            return;
        };
        let Some(target) = self.npc_state(npc).and_then(|npc| npc.live.target) else {
            warn!(
                "AI_Attack npc({}) ignored: no target set (Npc_SetTarget)",
                npc.value()
            );
            return;
        };
        crate::test_log!(
            "AI_Attack requested by npc({}) on target({})",
            npc.value(),
            target.value()
        );
        self.messages.attack.send(AttackChange {
            instance_id: npc,
            target,
        });
    }
    fake_impl!(ai_finishingmove(_0, _1));
    fake_impl!(ai_dodge(_0));
    fake_impl!(ai_useitem(_0, _1));
    /// Requests an item-state animation on the npc. The item symbol resolves
    /// the animation scheme (`scemename`), a negative state releases the pose.
    fn ai_useitemtostate(&mut self, npc: i32, item: i32, state: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            warn!("AI_UseItemToState with invalid npc instance({npc})");
            return;
        };
        // Item arguments are instance symbols, not string ids.
        let Some(item) = self.symbol_table.instance(item) else {
            warn!(
                "AI_UseItemToState npc({}) with invalid item instance({item})",
                npc.value()
            );
            return;
        };
        let Some(scheme) = self
            .typed
            .item(item.value() as u32)
            .and_then(|data| data.scemename)
            .and_then(|id| self.get_show_string(id))
        else {
            bevy::log::debug!(
                "AI_UseItemToState npc({}) item({}) has no scemename",
                npc.value(),
                item.value()
            );
            return;
        };
        self.messages.use_item_state.send(UseItemStateChange {
            instance_id: npc,
            item,
            scheme: StringCanon::new(scheme.string()),
            state,
        });
    }
    fake_impl!(ai_ask(_0, _1, _2));
    fake_impl!(ai_asktext(_0, _1, _2, _3, _4));
    fake_impl!(ai_waitforquestion(_0, _1));
    /// Plays a sound at the npc's world position.
    ///  Only the base definition plays: `C_SFX` volume/loop and `<name>_A1..` variants are ignored.
    fn ai_snd_play(&mut self, npc: i32, sound: i32) {
        self.push_sound_at_npc(npc, sound);
    }
    /// Plays a sound at the npc's world position, `v2` has no consumer in the
    /// viewer and no script in G2MDK calls this variant.
    fn ai_snd_play3d(&mut self, npc: i32, sound: i32, _v2: i32) {
        self.push_sound_at_npc(npc, sound);
    }
    fake_impl!(ai_playfx(_0, _1, _2));
    fake_impl!(ai_stopfx(_0, _1));
    fake_impl!(ai_takemob(_0, _1));
    fake_impl!(ai_dropmob(_0));
}

impl ImportCallState {
    /// The queued action releases any mob use and restores the standing pose;
    /// the body state is cleared right away so the same script call sees the
    /// npc as standing.
    fn request_standup(&mut self, npc: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        if self.npc_state(npc).is_none() {
            return;
        }
        self.npc_state_mut(npc).live.body_state = None;
        self.messages
            .stand_up
            .send(StandUpChange { instance_id: npc });
    }

    /// Highest total damage melee weapon in the npc's inventory. Ties are
    /// broken by value, then by the lower instance id so the pick does not
    /// depend on the unordered inventory map.
    ///
    /// Item conditions (`cond_atr`/`cond_value`) are not evaluated because the
    /// runtime state does not track the required attributes.
    fn best_melee_weapon(&self, npc: InstanceId) -> Option<InstanceId> {
        let items = self.npc_state(npc)?.inventory.items();
        items
            .into_iter()
            .filter(|(item, _)| {
                self.symbol_table
                    .symbol_name(item.value())
                    .is_some_and(|name| {
                        WeaponKind::from_symbol_name(&name) == Some(WeaponKind::Melee)
                    })
            })
            .max_by_key(|(item, _)| {
                let data = self.typed.item(item.value() as u32);
                let damage = data.as_ref().map_or(0, |data| data.damage_total);
                let value = data.as_ref().map_or(0, |data| data.value);
                (damage, value, std::cmp::Reverse(item.value()))
            })
            .map(|(item, _)| item)
    }
}
