use super::*;

impl mmcomando::zengine_scripts::symbols::Host for ImportCallState {
    fn get_symbol(&mut self, symbol: i32, element: i32) -> i32 {
        let (Ok(symbol), Ok(element)) = (u32::try_from(symbol), u32::try_from(element)) else {
            warn!("Script read of an invalid symbol({symbol}), element({element})");
            return -1;
        };
        self.typed.read_symbol(symbol, element)
    }

    fn set_symbol(&mut self, symbol: i32, element: i32, value: i32) {
        let (Ok(symbol), Ok(element)) = (u32::try_from(symbol), u32::try_from(element)) else {
            warn!("Script write of an invalid symbol({symbol}), element({element})");
            return;
        };
        self.typed.write_symbol(symbol, element, value);
        if element > 0 {
            let class = self.typed.class_of(symbol);
            if class == Some(daedalus_typed::ClassKind::Npc)
                && let Some(instance) = InstanceId::new(symbol as i32)
            {
                self.pending.npc_changed.insert(instance);
            } else if class == Some(daedalus_typed::ClassKind::Item)
                && let Some(instance) = InstanceId::new(symbol as i32)
            {
                self.pending.item_changed.insert(instance);
            }
        }
    }

    fn get_member(&mut self, instance: i32, member: i32, element: i32) -> i32 {
        let (Ok(instance), Ok(member), Ok(element)) = (
            u32::try_from(instance),
            u32::try_from(member),
            u32::try_from(element),
        ) else {
            warn!(
                "Script read of an invalid member, instance({instance}), member({member}), element({element})"
            );
            return -1;
        };

        self.typed.read_member(instance, member, element)
    }

    fn set_member(&mut self, instance: i32, member: i32, element: i32, value: i32) {
        let (Ok(instance), Ok(member), Ok(element)) = (
            u32::try_from(instance),
            u32::try_from(member),
            u32::try_from(element),
        ) else {
            warn!(
                "Script write of an invalid member, instance({instance}), member({member}), element({element})"
            );
            return;
        };

        self.typed.write_member(instance, member, element, value);
        let class = self.typed.class_of(instance);
        if class == Some(daedalus_typed::ClassKind::Npc)
            && let Some(instance) = InstanceId::new(instance as i32)
        {
            self.pending.npc_changed.insert(instance);
        } else if class == Some(daedalus_typed::ClassKind::Item)
            && let Some(instance) = InstanceId::new(instance as i32)
        {
            self.pending.item_changed.insert(instance);
        }
    }
}
