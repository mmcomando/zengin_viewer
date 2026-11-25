use super::*;

impl mmcomando::zengine_scripts::mdl_scripts::Host for ImportCallState {
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
        let body_model_id = self.symbol_table.string(body_model);
        let head_model_id = self.symbol_table.string(head_model);
        // The armor argument is an item instance symbol, not a string literal
        self.set_npc_armor(npc_index, InstanceId::new(armor_model));
        let mut entry = self.npc_state_mut(npc_index);
        entry.visual.body_model_id = body_model_id;
        entry.visual.head_model_id = head_model_id;
        entry.visual.face_texture_index = face_texture_index;
        entry.visual.body_texture_index = body_texture_index;
        entry.visual.initialized = true;
    }
    fn mdl_setvisual(&mut self, npc_index: i32, body_model_id: i32) {
        let Some(npc_index) = self.symbol_table.instance(npc_index) else {
            return;
        };
        let hierarchy_id = self.symbol_table.string(body_model_id);
        let mut entry = self.npc_state_mut(npc_index);
        entry.visual.hierarchy_id = hierarchy_id;
        entry.visual.initialized = true;
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
        self.npc_state_mut(npc_index).visual.model_scale = Some(scale);
    }
    /// Floats are passed as their bit patterns.
    fn mdl_setmodelfatness(&mut self, npc_index: i32, fatness: i32) {
        let Some(npc_index) = self.symbol_table.instance(npc_index) else {
            return;
        };
        self.npc_state_mut(npc_index).visual.model_fatness = Some(f32::from_bits(fatness as u32));
    }
    fake_impl!(mdl_startfaceani(_0, _1, _2, _3));
    fake_impl!(mdl_applyrandomfaceani(_0, _1, _2, _3, _4, _5, _6));
}
