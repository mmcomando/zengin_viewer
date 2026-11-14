use super::*;

impl mmcomando::zengine_scripts::misc_scripts::Host for ImportCallState {
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
    /// Shows the chapter splash: background texture, title, subtitle and a
    /// timed close in `time` milliseconds. The texture and sound names are
    /// resolved against the vfs when the ECS shows the screen.
    fn introducechapter(
        &mut self,
        title: i32,
        subtitle: i32,
        texture: i32,
        sound: i32,
        wait_time: i32,
    ) {
        self.push_chapter_intro(title, subtitle, texture, sound, wait_time);
    }
    fake_impl!(update_choicebox(_0));
    fn createinvitem(&mut self, npc: i32, item: i32) {
        self.createinvitems(npc, item, 1);
    }
    fn createinvitems(&mut self, npc: i32, item: i32, count: i32) {
        let Some((npc, item)) = self.npc_item_pair(npc, item) else {
            return;
        };
        self.npc_state_mut(npc).inventory.create(item, count);
        if self.player_instance == Some(npc) {
            self.messages
                .inventory_gains
                .send(InventoryGain { npc, item, count });
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
        let mainflag = self
            .typed
            .item(item.value() as u32)
            .map_or(0, |data| data.mainflag);
        if mainflag & ITEM_MAINFLAG_ARMOR != 0 {
            self.npc_state_mut(npc).equipment.armor = Some(item);
            return;
        }
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
        {
            let mut instance = self.npc_state_mut(npc);
            match kind {
                WeaponKind::Melee => instance.equipment.melee = Some(item),
                WeaponKind::Bow | WeaponKind::Crossbow => instance.equipment.ranged = Some(item),
            }
        }
    }
    fake_impl!(infomanager_hasfinished() -> i32);
    fake_impl!(tal_configure(_0, _1));
    fake_impl!(rtn_exchange(_0, _1));
    fake_impl!(perc_setrange(_0, _1));
}
