//! Typed script-class registry and VM access dispatch.

use std::sync::{Arc, PoisonError, RwLock};

use daedalus_wasm::daedalus::SymbolKind;
use daedalus_wasm::symbol_store::SymbolStore;

use super::access::ScriptVmAccess;
use super::info::Info;
use super::item::Item;
use super::menu::Menu;
use super::menu_item::MenuItem;
use super::npc::Npc;

/// One modeled field slot used for load-time validation and initial seeding.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FieldSlot {
    /// Daedalus member name inside the class (without the `c_npc.` prefix).
    pub member: &'static str,
    /// First modeled element.
    pub element: u32,
    /// Byte offset of the member inside the instance record.
    pub offset: u32,
    /// Number of modeled elements.
    pub count: u32,
}

/// Script classes with a typed host model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassKind {
    /// `C_Npc`.
    Npc,
    /// `C_Item`.
    Item,
    /// `C_Menu` (MENU.DAT).
    Menu,
    /// `C_MenuItem` (MENU.DAT).
    MenuItem,
    /// `C_Info` dialog information.
    Info,
}

impl ClassKind {
    /// Daedalus class name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Npc => "c_npc",
            Self::Item => "c_item",
            Self::Menu => "c_menu",
            Self::MenuItem => "c_menu_item",
            Self::Info => "c_info",
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Npc => 0,
            Self::Item => 1,
            Self::Menu => 2,
            Self::MenuItem => 3,
            Self::Info => 4,
        }
    }
}

/// Typed instance storage, shared with the ECS through the `Arc` handles.
#[derive(Debug, Clone)]
pub enum TypedInstance {
    /// `C_Npc` instance.
    Npc(Arc<RwLock<Npc>>),
    /// `C_Item` instance.
    Item(Arc<RwLock<Item>>),
    /// `C_Menu` instance.
    Menu(Arc<RwLock<Menu>>),
    /// `C_MenuItem` instance.
    MenuItem(Arc<RwLock<MenuItem>>),
    /// `C_Info` instance.
    Info(Arc<RwLock<Info>>),
}

impl TypedInstance {
    /// Class of the stored instance.
    #[must_use]
    pub fn class(&self) -> ClassKind {
        match self {
            Self::Npc(_) => ClassKind::Npc,
            Self::Item(_) => ClassKind::Item,
            Self::Menu(_) => ClassKind::Menu,
            Self::MenuItem(_) => ClassKind::MenuItem,
            Self::Info(_) => ClassKind::Info,
        }
    }

    /// Npc handle, `None` for other classes.
    #[must_use]
    pub fn npc(&self) -> Option<&Arc<RwLock<Npc>>> {
        match self {
            Self::Npc(handle) => Some(handle),
            _ => None,
        }
    }

    /// Item handle, `None` for other classes.
    #[must_use]
    pub fn item(&self) -> Option<&Arc<RwLock<Item>>> {
        match self {
            Self::Item(handle) => Some(handle),
            _ => None,
        }
    }

    /// Menu handle, `None` for other classes.
    #[must_use]
    pub fn menu(&self) -> Option<&Arc<RwLock<Menu>>> {
        match self {
            Self::Menu(handle) => Some(handle),
            _ => None,
        }
    }

    /// Menu item handle, `None` for other classes.
    #[must_use]
    pub fn menu_item(&self) -> Option<&Arc<RwLock<MenuItem>>> {
        match self {
            Self::MenuItem(handle) => Some(handle),
            _ => None,
        }
    }

    /// Dialog info handle, `None` for other classes.
    #[must_use]
    pub fn info(&self) -> Option<&Arc<RwLock<Info>>> {
        match self {
            Self::Info(handle) => Some(handle),
            _ => None,
        }
    }

    fn read(&self, offset: u32, element: u32) -> Option<i32> {
        match self {
            Self::Npc(handle) => read_lock(handle).read(offset, element),
            Self::Item(handle) => read_lock(handle).read(offset, element),
            Self::Menu(handle) => read_lock(handle).read(offset, element),
            Self::MenuItem(handle) => read_lock(handle).read(offset, element),
            Self::Info(handle) => read_lock(handle).read(offset, element),
        }
    }

    fn write(&self, offset: u32, element: u32, value: i32) -> bool {
        match self {
            Self::Npc(handle) => write_lock(handle).write(offset, element, value),
            Self::Item(handle) => write_lock(handle).write(offset, element, value),
            Self::Menu(handle) => write_lock(handle).write(offset, element, value),
            Self::MenuItem(handle) => write_lock(handle).write(offset, element, value),
            Self::Info(handle) => write_lock(handle).write(offset, element, value),
        }
    }
}

/// Result of a typed member access attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberAccess<T> {
    /// The typed class handled the access.
    Handled(T),
    /// The instance is typed, but the member/element is not modeled; the caller
    /// should fall back to the raw word store and report the gap.
    NotModeled,
    /// The instance has no typed model (untyped class, validation disabled).
    NotTyped,
}

/// A hardcoded class field that did not match the loaded G2 script layout.
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    /// Class name.
    pub class: &'static str,
    /// Member name.
    pub member: &'static str,
    /// Offset expected by the host model.
    pub expected_offset: u32,
    /// Member offset found in the loaded script.
    pub found_offset: u32,
    /// Elements expected to be available.
    pub expected_elements: u32,
    /// Elements available in the loaded script.
    pub found_elements: u32,
}

/// Per-instance typed class storage, indexed by instance symbol id.
#[derive(Debug, Default, Clone)]
pub struct TypedRegistry {
    entries: Vec<Option<TypedInstance>>,
    classes: [Option<u32>; 5],
    issues: Vec<ValidationIssue>,
}

impl TypedRegistry {
    /// Builds the registry from the loaded symbol store: validates the hardcoded
    /// G2 offsets by member name, creates typed handles and moves modeled values
    /// out of the raw word space.
    #[must_use]
    pub fn build(store: &mut SymbolStore) -> Self {
        let mut registry = Self {
            entries: vec![None; store.symbol_count()],
            ..Self::default()
        };

        registry.build_class::<Npc>(
            store,
            ClassKind::Npc,
            super::npc::FIELD_SLOTS,
            TypedInstance::Npc,
        );
        registry.build_class::<Item>(
            store,
            ClassKind::Item,
            super::item::FIELD_SLOTS,
            TypedInstance::Item,
        );
        registry.build_class::<Menu>(
            store,
            ClassKind::Menu,
            super::menu::FIELD_SLOTS,
            TypedInstance::Menu,
        );
        registry.build_class::<MenuItem>(
            store,
            ClassKind::MenuItem,
            super::menu_item::FIELD_SLOTS,
            TypedInstance::MenuItem,
        );
        registry.build_class::<Info>(
            store,
            ClassKind::Info,
            super::info::FIELD_SLOTS,
            TypedInstance::Info,
        );

        registry
    }

    fn build_class<T: ScriptVmAccess + Default>(
        &mut self,
        store: &mut SymbolStore,
        class: ClassKind,
        slots: &[FieldSlot],
        wrap: fn(Arc<RwLock<T>>) -> TypedInstance,
    ) {
        let class_name = class.name();
        let Some(class_symbol) = store.class_symbol(class_name) else {
            return;
        };

        let mut valid = true;
        for slot in slots {
            let Some(member) = store.find_member(class_name, slot.member) else {
                self.issues.push(ValidationIssue {
                    class: class_name,
                    member: slot.member,
                    expected_offset: slot.offset,
                    found_offset: 0,
                    expected_elements: slot.element + slot.count,
                    found_elements: 0,
                });
                valid = false;
                continue;
            };
            let Some(meta) = store.meta(member) else {
                valid = false;
                continue;
            };
            if meta.address_or_offset != slot.offset
                || meta.element_count < slot.element + slot.count
            {
                self.issues.push(ValidationIssue {
                    class: class_name,
                    member: slot.member,
                    expected_offset: slot.offset,
                    found_offset: meta.address_or_offset,
                    expected_elements: slot.element + slot.count,
                    found_elements: meta.element_count,
                });
                valid = false;
            }
        }
        if !valid {
            return;
        }

        self.classes[class.index()] = Some(class_symbol);
        for instance in instances_of(store, class_symbol) {
            let handle = seed::<T>(store, instance, slots);
            self.entries[usize::try_from(instance).unwrap_or_default()] = Some(wrap(handle));
        }
    }

    /// Number of instances with a typed model.
    #[must_use]
    pub fn typed_count(&self) -> usize {
        self.entries.iter().filter(|entry| entry.is_some()).count()
    }

    /// Typed handle of an instance, `None` when the instance has no typed model.
    #[must_use]
    pub fn instance(&self, instance: u32) -> Option<&TypedInstance> {
        self.entries.get(usize::try_from(instance).ok()?)?.as_ref()
    }

    /// Typed class of an instance, `None` when untyped.
    #[must_use]
    pub fn class_of(&self, instance: u32) -> Option<ClassKind> {
        self.instance(instance).map(TypedInstance::class)
    }

    /// Class fields which did not match the loaded script layout.
    #[must_use]
    pub fn issues(&self) -> &[ValidationIssue] {
        &self.issues
    }

    /// Whether the class was validated and has typed instances in this store.
    #[must_use]
    pub fn is_typed_class(&self, class: ClassKind) -> bool {
        self.classes[class.index()].is_some()
    }

    /// Reads a class member, falling back to the raw word store only when the
    /// registry does not model the instance.
    #[must_use]
    pub fn read_member(
        &self,
        store: &SymbolStore,
        instance: u32,
        member: u32,
        element: u32,
    ) -> MemberAccess<Option<i32>> {
        let Some(entry) = self.instance(instance) else {
            return MemberAccess::NotTyped;
        };
        let Some(meta) = store.meta(member) else {
            return MemberAccess::NotModeled;
        };
        if !meta.is_class_member || !self.member_belongs_to(store, entry.class(), member) {
            return MemberAccess::NotModeled;
        }
        match entry.read(meta.address_or_offset, element) {
            Some(value) => MemberAccess::Handled(Some(value)),
            None => MemberAccess::NotModeled,
        }
    }

    /// Writes a class member, falling back to the raw word store only when the
    /// registry does not model the instance.
    pub fn write_member(
        &self,
        store: &SymbolStore,
        instance: u32,
        member: u32,
        element: u32,
        value: i32,
    ) -> MemberAccess<()> {
        let Some(entry) = self.instance(instance) else {
            return MemberAccess::NotTyped;
        };
        let Some(meta) = store.meta(member) else {
            return MemberAccess::NotModeled;
        };
        if !meta.is_class_member || !self.member_belongs_to(store, entry.class(), member) {
            return MemberAccess::NotModeled;
        }
        if entry.write(meta.address_or_offset, element, value) {
            MemberAccess::Handled(())
        } else {
            MemberAccess::NotModeled
        }
    }

    /// Snapshot of an npc instance, from the typed handle or the raw store.
    #[must_use]
    pub fn npc(&self, store: &SymbolStore, instance: u32) -> Option<Npc> {
        if let Some(TypedInstance::Npc(handle)) = self.instance(instance) {
            return Some(read_lock(handle).clone());
        }
        project_raw::<Npc>(store, instance, ClassKind::Npc, super::npc::FIELD_SLOTS)
    }

    /// Snapshot of an item instance, from the typed handle or the raw store.
    #[must_use]
    pub fn item(&self, store: &SymbolStore, instance: u32) -> Option<Item> {
        if let Some(TypedInstance::Item(handle)) = self.instance(instance) {
            return Some(read_lock(handle).clone());
        }
        project_raw::<Item>(store, instance, ClassKind::Item, super::item::FIELD_SLOTS)
    }

    /// Snapshot of a menu instance, from the typed handle or the raw store.
    #[must_use]
    pub fn menu(&self, store: &SymbolStore, instance: u32) -> Option<Menu> {
        if let Some(TypedInstance::Menu(handle)) = self.instance(instance) {
            return Some(read_lock(handle).clone());
        }
        project_raw::<Menu>(store, instance, ClassKind::Menu, super::menu::FIELD_SLOTS)
    }

    /// Snapshot of a menu item instance, from the typed handle or the raw store.
    #[must_use]
    pub fn menu_item(&self, store: &SymbolStore, instance: u32) -> Option<MenuItem> {
        if let Some(TypedInstance::MenuItem(handle)) = self.instance(instance) {
            return Some(read_lock(handle).clone());
        }
        project_raw::<MenuItem>(
            store,
            instance,
            ClassKind::MenuItem,
            super::menu_item::FIELD_SLOTS,
        )
    }

    /// Snapshot of a dialog info instance, from the typed handle or the raw store.
    #[must_use]
    pub fn info(&self, store: &SymbolStore, instance: u32) -> Option<Info> {
        if let Some(TypedInstance::Info(handle)) = self.instance(instance) {
            return Some(read_lock(handle).clone());
        }
        project_raw::<Info>(store, instance, ClassKind::Info, super::info::FIELD_SLOTS)
    }

    fn member_belongs_to(&self, store: &SymbolStore, class: ClassKind, member: u32) -> bool {
        self.classes[class.index()].is_some_and(|class| store.parent(member) == Some(class))
    }
}

fn read_lock<T>(lock: &RwLock<T>) -> std::sync::RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(PoisonError::into_inner)
}

fn write_lock<T>(lock: &RwLock<T>) -> std::sync::RwLockWriteGuard<'_, T> {
    lock.write().unwrap_or_else(PoisonError::into_inner)
}

fn instances_of(store: &SymbolStore, class_symbol: u32) -> Vec<u32> {
    (0..store.symbol_count())
        .filter_map(|index| u32::try_from(index).ok())
        .filter(|instance| {
            store
                .meta(*instance)
                .is_some_and(|meta| meta.kind == SymbolKind::Instance)
        })
        .filter(|instance| store.class_of(*instance) == Some(class_symbol))
        .collect()
}

fn seed<T: ScriptVmAccess + Default>(
    store: &mut SymbolStore,
    instance: u32,
    slots: &[FieldSlot],
) -> Arc<RwLock<T>> {
    let mut data = T::default();
    for slot in slots {
        for element in slot.element..slot.element + slot.count {
            let byte_offset = slot.offset + element * 4;
            let Some(raw) = store.read_offset(instance, byte_offset) else {
                continue;
            };
            if data.write(slot.offset, element, raw) {
                store.write_offset(instance, byte_offset, 0);
            }
        }
    }
    Arc::new(RwLock::new(data))
}

fn project_raw<T: ScriptVmAccess + Default>(
    store: &SymbolStore,
    instance: u32,
    class: ClassKind,
    slots: &[FieldSlot],
) -> Option<T> {
    let class_name = class.name();
    let class_symbol = store.class_symbol(class_name)?;
    if store.class_of(instance) != Some(class_symbol) {
        return None;
    }

    let mut data = T::default();
    for slot in slots {
        let Some(member) = store.find_member(class_name, slot.member) else {
            continue;
        };
        let Some(meta) = store.meta(member) else {
            continue;
        };
        for element in slot.element..slot.element + slot.count {
            let Some(raw) = store.read_member(instance, member, element) else {
                continue;
            };
            // The field mapping uses the hardcoded offset; if the layout moved
            // the write is rejected and the field keeps its default, the
            // load-time validation already reported the mismatch.
            data.write(meta.address_or_offset, element, raw);
        }
    }
    Some(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::StringId;
    use daedalus_wasm::daedalus::{Symbol, SymbolKind};

    fn class_symbol(index: u32, name: &str) -> Symbol {
        let mut symbol = Symbol::new(index, false, 0, None);
        symbol.set_name(name);
        symbol.set_kind(SymbolKind::Class);
        symbol
    }

    fn member_symbol(index: u32, name: &str, class: u32, offset: u32, elements: u32) -> Symbol {
        let mut symbol = Symbol::new(index, true, elements, Some(offset));
        symbol.set_name(name);
        symbol.set_parent(Some(class));
        symbol
    }

    fn instance_symbol(index: u32, name: &str, class: u32, elements: u32) -> Symbol {
        let mut symbol = Symbol::new(index, false, elements, None);
        symbol.set_name(name);
        symbol.set_kind(SymbolKind::Instance);
        symbol.set_parent(Some(class));
        symbol
    }

    fn npc_symbols() -> Vec<Symbol> {
        let mut instance = instance_symbol(9, "test_npc", 0, 270);
        let mut defaults = vec![0; 270];
        defaults[292 / 4] = 7;
        defaults[440 / 4] = 100;
        defaults[440 / 4 + 1] = 200;
        defaults[440 / 4 + 2] = 55;
        // Unmodeled element, must stay in the raw store.
        defaults[440 / 4 + 6] = 77;
        defaults[648 / 4 + 4] = 3;
        defaults[1068 / 4] = 1000;
        instance.set_default_values(defaults);

        vec![
            class_symbol(0, "c_npc"),
            member_symbol(1, "c_npc.name", 0, 292, 5),
            member_symbol(2, "c_npc.attribute", 0, 440, 8),
            member_symbol(3, "c_npc.daily_routine", 0, 608, 1),
            member_symbol(4, "c_npc.aivar", 0, 648, 100),
            member_symbol(5, "c_npc.wp", 0, 1048, 1),
            member_symbol(6, "c_npc.exp", 0, 1068, 1),
            member_symbol(7, "c_npc.exp_next", 0, 1072, 1),
            member_symbol(8, "c_npc.lp", 0, 1076, 1),
            instance,
        ]
    }

    #[test]
    fn typed_npc_reads_and_writes_semantic_fields() {
        let symbols = npc_symbols();
        let mut store = SymbolStore::from_symbols(&symbols);
        let registry = TypedRegistry::build(&mut store);

        assert!(registry.issues().is_empty());
        assert_eq!(registry.typed_count(), 1);

        let npc = registry.npc(&store, 9).expect("typed npc");
        assert_eq!(npc.name, StringId::new(7));
        assert_eq!(npc.hitpoints, 100);
        assert_eq!(npc.hitpoints_max, 200);
        assert_eq!(npc.mana, 55);
        assert_eq!(npc.exp, 1000);
        assert_eq!(npc.aivar[4], 3);
        assert_eq!(npc.daily_routine, None);

        // Modeled slots are moved out of the raw store, unmodeled ones stay.
        assert_eq!(store.read_offset(9, 292), Some(0));
        assert_eq!(store.read_offset(9, 440), Some(0));
        assert_eq!(store.read_offset(9, 440 + 6 * 4), Some(77));

        let attribute = store.find_member("c_npc", "attribute").expect("member");
        assert_eq!(
            registry.write_member(&store, 9, attribute, 0, 50),
            MemberAccess::Handled(())
        );
        assert_eq!(registry.npc(&store, 9).expect("npc").hitpoints, 50);

        // Element 6 of `attribute` is not modeled, the caller must fall back.
        assert_eq!(
            registry.read_member(&store, 9, attribute, 6),
            MemberAccess::NotModeled
        );
    }

    #[test]
    fn offset_mismatch_disables_the_class() {
        let mut symbols = npc_symbols();
        // Move the attribute member, the hardcoded accessor would be wrong.
        symbols[2] = member_symbol(2, "c_npc.attribute", 0, 444, 8);

        let mut store = SymbolStore::from_symbols(&symbols);
        let registry = TypedRegistry::build(&mut store);

        assert_eq!(registry.typed_count(), 0);
        assert!(registry.instance(9).is_none());
        assert!(
            registry
                .issues()
                .iter()
                .any(|issue| issue.member == "attribute")
        );
        assert!(registry.npc(&store, 9).is_some());
    }

    #[test]
    fn member_of_another_class_is_not_modeled() {
        let mut symbols = npc_symbols();
        symbols.push(class_symbol(10, "c_item"));
        symbols.push(member_symbol(11, "c_item.name", 10, 292, 1));

        let mut store = SymbolStore::from_symbols(&symbols);
        let registry = TypedRegistry::build(&mut store);
        let item_member = store.find_member("c_item", "name").expect("member");

        assert_eq!(
            registry.read_member(&store, 9, item_member, 0),
            MemberAccess::NotModeled
        );
    }
}

#[cfg(test)]
mod info_tests {
    use super::*;
    use daedalus_wasm::daedalus::{Symbol, SymbolKind};

    fn symbol(index: u32, name: &str, kind: SymbolKind) -> Symbol {
        let mut symbol = Symbol::new(index, false, 0, None);
        symbol.set_name(name);
        symbol.set_kind(kind);
        symbol
    }

    fn member(index: u32, name: &str, class: u32, offset: u32) -> Symbol {
        let mut symbol = Symbol::new(index, true, 1, Some(offset));
        symbol.set_name(name);
        symbol.set_parent(Some(class));
        symbol
    }

    fn info_symbols() -> Vec<Symbol> {
        let mut instance = Symbol::new(9, false, 12, None);
        instance.set_name("test_info");
        instance.set_kind(SymbolKind::Instance);
        instance.set_parent(Some(0));
        let mut defaults = vec![0; 12];
        defaults[0] = 12081; // npc
        defaults[1] = 2; // nr
        defaults[2] = 1; // important
        defaults[3] = 26935; // condition
        defaults[4] = 26936; // information
        instance.set_default_values(defaults);

        vec![
            symbol(0, "c_info", SymbolKind::Class),
            member(1, "c_info.npc", 0, 0),
            member(2, "c_info.nr", 0, 4),
            member(3, "c_info.important", 0, 8),
            member(4, "c_info.condition", 0, 12),
            member(5, "c_info.information", 0, 16),
            member(6, "c_info.description", 0, 20),
            member(7, "c_info.trade", 0, 40),
            member(8, "c_info.permanent", 0, 44),
            instance,
        ]
    }

    #[test]
    fn typed_info_fields() {
        let symbols = info_symbols();
        let mut store = SymbolStore::from_symbols(&symbols);
        let registry = TypedRegistry::build(&mut store);

        assert!(registry.issues().is_empty());
        assert!(registry.is_typed_class(ClassKind::Info));

        let info = registry.info(&store, 9).expect("typed info");
        assert_eq!(info.npc, crate::ids::InstanceId::new(12081));
        assert_eq!(info.order, 2);
        assert!(info.important);
        assert_eq!(info.condition, crate::ids::FunctionId::new(26935));
        assert_eq!(info.information, crate::ids::FunctionId::new(26936));
        assert_eq!(info.description, None);
        assert!(!info.trade);
        assert!(!info.permanent);

        let nr = store.find_member("c_info", "nr").expect("member");
        assert_eq!(
            registry.write_member(&store, 9, nr, 0, 7),
            MemberAccess::Handled(())
        );
        assert_eq!(registry.info(&store, 9).expect("info").order, 7);
    }
}
