use super::super::bindings::exports::mmcomando::zengine_scripts::manage::SymbolKind;
use super::super::state::{DialogChoice, DialogRoot, NpcCheckInfo};
use super::super::string_value::ShowString;
use super::super::symbol_id::{FunctionId, InstanceId, SymbolId};
use bevy::log::warn;
use daedalus_typed::Info;

impl super::ScriptManager {
    /// Newest first, like the engine which inserts each new choice at the front
    /// of the info's list.
    pub fn sub_dialog_choices(&mut self, info_instance_id: InstanceId) -> Vec<DialogChoice> {
        let choices = self
            .loaded_state()
            .dialog_choices
            .get(&info_instance_id)
            .cloned()
            .unwrap_or_default();
        choices.into_iter().rev().collect()
    }

    /// Removes every entry with the function, like the engine does for a picked
    /// sub choice.
    pub fn remove_sub_dialog_choice(
        &mut self,
        info_instance_id: InstanceId,
        sequence_function_id: FunctionId,
    ) {
        let Some(choices) = self
            .store
            .data_mut()
            .loaded_state
            .dialog_choices
            .get_mut(&info_instance_id)
        else {
            return;
        };
        choices.retain(|choice| choice.sequence_function_id != sequence_function_id);
    }

    /// Registers the info instance as known by the npc.
    pub fn mark_info_known(
        &mut self,
        npc_instance_id: InstanceId,
        info_instance_id: Option<InstanceId>,
    ) {
        let Some(info_instance_id) = info_instance_id else {
            return;
        };
        self.npc_state_mut(npc_instance_id)
            .dialog
            .known_infos
            .insert(info_instance_id);
    }

    /// Evaluates the condition function of every root with the npc instance
    /// set, dropping roots whose condition returns false
    pub fn filter_dialog_roots(
        &mut self,
        npc_player: InstanceId,
        npc_talk_to: InstanceId,
        roots: &mut Vec<DialogRoot>,
    ) {
        roots.retain(|root| {
            let Some(condition_fn) = root.condition_function_id else {
                return true;
            };
            let already_known = !root.permanent
                && root.info_instance_id.is_some_and(|info_instance_id| {
                    self.npc_state(npc_player)
                        .is_some_and(|npc| npc.dialog.known_infos.contains(&info_instance_id))
                });
            if already_known {
                return false;
            }

            self.set_instance(npc_talk_to);
            self.set_self(npc_talk_to);
            self.set_other(npc_player);
            self.call_function_bool(condition_fn)
        });
    }

    /// Recomputes the `Npc_CheckInfo` cache of the npc from its root infos and
    /// returns the roots whose conditions passed. Call it before a script
    /// function which may consult `Npc_CheckInfo`, a host import cannot evaluate
    /// the conditions itself.
    pub fn refresh_npc_check_info(
        &mut self,
        npc_instance_id: InstanceId,
        player_instance_id: InstanceId,
    ) -> Option<Vec<DialogRoot>> {
        let mut roots = self
            .npc_state(npc_instance_id)
            .and_then(|npc| npc.dialog.roots.clone())?;

        self.filter_dialog_roots(player_instance_id, npc_instance_id, &mut roots);
        let check_info = NpcCheckInfo {
            important: roots.iter().any(|root| root.important),
            normal: roots.iter().any(|root| !root.important),
        };
        self.store
            .data_mut()
            .check_info
            .insert(npc_instance_id, check_info);
        Some(roots)
    }

    pub fn get_dialog_info(&mut self, symbol_id: SymbolId) -> Option<Info> {
        self.store.data().typed.info(symbol_id.value() as u32)
    }

    pub fn load_dialogs_information(&mut self) {
        let symbols_num = self.get_all_symbols_num();

        let info_class_id = self.get_symbol_id("c_info");

        for index in 0..symbols_num {
            let Some(index) = SymbolId::new(index) else {
                continue;
            };
            let symbol_info = self.get_symbol_info(index);
            if symbol_info.kind != SymbolKind::Instance {
                continue;
            }
            // Kind was just verified, the checked conversion cannot fail
            let index = InstanceId::from_symbol(index);
            if !self.has_ancestor(&symbol_info, info_class_id) {
                continue;
            }

            let Some(dialog_info) = self.get_dialog_info(index.symbol()) else {
                continue;
            };
            let Some(sequence_function_id) = dialog_info
                .information
                .and_then(|id| self.symbol_table().function(id.value()))
            else {
                continue;
            };
            let description = dialog_info
                .description
                .and_then(|id| self.get_show_string(id))
                .map(ShowString::into_string)
                .unwrap_or_default();

            let Some(npc_id) = dialog_info.npc else {
                continue;
            };
            if self.symbol_table().instance(npc_id.value()).is_none() {
                continue;
            }
            let condition_function_id = dialog_info
                .condition
                .and_then(|id| self.symbol_table().function(id.value()));
            let mut npc = self.npc_state_mut(npc_id);
            let roots = npc.dialog.roots.get_or_insert_with(Vec::new);
            roots.push(DialogRoot {
                info_instance_id: Some(index),
                sequence_function_id,
                condition_function_id,
                permanent: dialog_info.permanent,
                description,
                important: dialog_info.important,
                trade: dialog_info.trade,
                order: dialog_info.order,
            });

            // `scriptFn` breaks ties like the original engine does.
            roots.sort_by_key(|el| (el.order, el.sequence_function_id.value()));
        }
    }

    /// Runs `b_assesstalk` with `self` bound to the npc and `other` to the
    /// player, deciding whether the conversation starts, is refused or turns
    /// into a fight. `false` when the script function is missing.
    pub fn call_assesstalk(&mut self, npc_player: InstanceId, npc_talk_to: InstanceId) -> bool {
        let Some(function) = self.try_get_function_id("b_assesstalk") else {
            return false;
        };
        self.set_self(npc_talk_to);
        self.set_other(npc_player);
        self.set_instance(npc_talk_to);
        self.refresh_npc_check_info(npc_talk_to, npc_player);
        match self.try_call_function(function) {
            Ok(()) => true,
            Err(err) => {
                warn!("b_assesstalk npc({npc_talk_to}) error: {err:?}");
                false
            }
        }
    }

    pub fn call_dialog_sequence_function(&mut self, sequence_function_id: FunctionId) {
        self.store.data_mut().close_dialog = false;
        match self.try_call_function(sequence_function_id) {
            Ok(()) => {}
            Err(err) => {
                println!("call_dialog_sequence_function({sequence_function_id:?}) error: {err:?}");
            }
        }
    }
}
