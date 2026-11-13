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
        self.pending.animation_changes.push(AnimationChange {
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
        self.pending.animation_changes.push(AnimationChange {
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
        self.pending.animation_changes.push(AnimationChange {
            instance_id: npc_index,
            animation: StringCanon::new(ani),
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
    fn ai_startstate(&mut self, npc: i32, state: i32, _finalize: i32, _waypoint: i32) {
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
        let state_name = self.symbol_table.symbol_name(state).unwrap_or_default();
        let other = self
            .npc_state(npc)
            .and_then(|npc| npc.live.target)
            .or_else(|| self.other_instance());
        bevy::log::info!(
            "AI_StartState({state_name}) npc({}) other({other:?})",
            npc.value()
        );
        // The engine drops the ai action queue when a state starts.
        self.pending
            .clear_ai_queue_changes
            .push(ClearAiQueueChange { instance_id: npc });
        self.pending.start_state_changes.push(StartStateChange {
            instance_id: npc,
            state: state_fn,
            other,
        });
    }
    fake_impl!(ai_continueroutine(_0));
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
    fake_impl!(ai_gotonextfp(_0, _1));
    fake_impl!(ai_gotosound(_0));
    fake_impl!(ai_takeitem(_0, _1));
    fake_impl!(ai_dropitem(_0, _1));
    fake_impl!(ai_drawweapon(_0));
    fn ai_removeweapon(&mut self, npc: i32) {
        self.set_weapon_mode(npc, WeaponMode::Fists);
    }
    fake_impl!(ai_processinfos(_0));
    fake_impl!(ai_equipbestmeleeweapon(_0) -> i32);
    fake_impl!(ai_equipbestrangedweapon(_0) -> i32);
    fake_impl!(ai_equipbestarmor(_0));
    fake_impl!(ai_unequipweapons(_0));
    fake_impl!(ai_unequiparmor(_0));
    fake_impl!(ai_equiparmor(_0, _1));
    fn ai_readymeleeweapon(&mut self, npc: i32) {
        self.set_weapon_mode(npc, WeaponMode::Melee);
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
        bevy::log::info!(
            "AI_Attack requested by npc({}) on target({})",
            npc.value(),
            target.value()
        );
        self.pending.attack_changes.push(AttackChange {
            instance_id: npc,
            target,
        });
    }
    fake_impl!(ai_finishingmove(_0, _1));
    fake_impl!(ai_dodge(_0));
    fake_impl!(ai_useitem(_0, _1));
    fake_impl!(ai_useitemtostate(_0, _1, _2));
    fake_impl!(ai_ask(_0, _1, _2));
    fake_impl!(ai_asktext(_0, _1, _2, _3, _4));
    fake_impl!(ai_waitforquestion(_0, _1));
    /// Plays a sound at the npc's world position.
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
