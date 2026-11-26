use super::*;

impl mmcomando::zengine_scripts::info_scripts::Host for ImportCallState {
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
            .map_or_default(crate::script::string_value::ShowString::into_string);

        self.loaded_state
            .dialog_choices
            .entry(dialog_id)
            .or_default()
            .push(DialogChoice {
                description,
                sequence_function_id: function_id,
            });
    }
    fn info_clearchoices(&mut self, dialog_id: i32) {
        let Some(dialog_id) = self.symbol_table.instance(dialog_id) else {
            return;
        };
        self.loaded_state.dialog_choices.remove(&dialog_id);
    }
}
