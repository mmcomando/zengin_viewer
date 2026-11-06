use super::super::bindings::exports::mmcomando::zengine_scripts::manage::SymbolKind;
use super::super::state::{DialogChoice, DialogRoot};
use super::super::string_value::ShowString;
use super::super::symbol_id::{FunctionId, InstanceId, SymbolId};
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
                        .is_some_and(|npc| npc.known_infos.contains(&info_instance_id))
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

    /// Whether the npc has important roots which could still run: either not
    /// played yet, or permanent. Cheap, it does not evaluate condition functions.
    pub fn npc_has_pending_important_dialog(
        &self,
        npc_player: InstanceId,
        npc_talk_to: InstanceId,
    ) -> bool {
        let roots = self
            .npc_state(npc_talk_to)
            .and_then(|state| state.dialog_roots.clone());
        let Some(roots) = roots else {
            return false;
        };
        let known_infos = self
            .npc_state(npc_player)
            .map(|state| state.known_infos.clone());

        roots.iter().any(|root| {
            root.important
                && (root.permanent
                    || root.info_instance_id.is_none_or(|info| {
                        !known_infos
                            .as_ref()
                            .is_some_and(|known| known.contains(&info))
                    }))
        })
    }

    /// Evaluates the conditions of the important roots and returns the first
    /// matching root in order. Played non-permanent roots are filtered out.
    pub fn npc_ready_important_dialog(
        &mut self,
        npc_player: InstanceId,
        npc_talk_to: InstanceId,
    ) -> Option<DialogRoot> {
        let mut roots: Vec<DialogRoot> = {
            let state = self.npc_state(npc_talk_to)?;
            let roots = state.dialog_roots.as_ref()?;
            roots
                .iter()
                .filter(|root| root.important)
                .cloned()
                .collect()
        };
        if roots.is_empty() {
            return None;
        }

        self.filter_dialog_roots(npc_player, npc_talk_to, &mut roots);
        roots.into_iter().next()
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
            let roots = npc.dialog_roots.get_or_insert_with(Vec::new);
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

            roots.sort_by_key(|el| el.order);
        }
    }

    pub fn call_dialog_sequence_function(&mut self, sequence_function_id: FunctionId) {
        self.store.data_mut().close_dialog = false;
        match self.try_call_function_discard(sequence_function_id) {
            Ok(()) => {}
            Err(err) => {
                println!("call_dialog_sequence_function({sequence_function_id:?}) error: {err:?}");
            }
        }
    }
}
