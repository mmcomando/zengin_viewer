pub(crate) const CURRENT_CLASS_INSTANCE_MEMORY_GLOBAL_INDEX: u32 = 0;
pub(crate) const CURRENT_INSTANCE_SYMBOL_INDEX: u32 = 1;
/// Global which stores number of all symbols, it is patched with real value
/// when the base module is merged with daedalus data.
pub(crate) const SYMBOLS_NUM_GLOBAL_INDEX: u32 = 3;
/// Index of `set_instance` buildin function inside base module (see `base_module.wat`),
/// converted daedalus `SetInstance` instructions call it.
pub(crate) const BASE_FUNC_SET_INSTANCE: u32 = 3;
/// Index of `cabi_realloc` buildin function inside base module (see `base_module.wat`),
/// string-returning external wrappers use it to allocate the lowered string
/// return area.
pub(crate) const BASE_FUNC_CABI_REALLOC: u32 = 1;
/// Globals which store symbol indices of the well known daedalus instance
/// variables, they are patched with real values (or -1 when the script does
/// not declare them) during merge with daedalus data.
pub(crate) const SELF_SYMBOL_GLOBAL_INDEX: u32 = 4;
pub(crate) const OTHER_SYMBOL_GLOBAL_INDEX: u32 = 5;
pub(crate) const VICTIM_SYMBOL_GLOBAL_INDEX: u32 = 6;
pub(crate) const ITEM_SYMBOL_GLOBAL_INDEX: u32 = 7;
pub(crate) const HERO_SYMBOL_GLOBAL_INDEX: u32 = 8;
/// Global which stores number of all string pairs in memory, it is patched
/// with real value when the base module is merged with daedalus data.
pub(crate) const STRINGS_NUM_GLOBAL_INDEX: u32 = 9;
/// Global which stores base address of the `DYN_STRINGS` pool, it is patched
/// with real value when the base module is merged with daedalus data.
pub(crate) const DYN_POOL_BASE_GLOBAL_INDEX: u32 = 10;
/// Index of `copy_dynamic_string` buildin function inside base module (see
/// `base_module.wat`), converted string-returning external calls use it to
/// materialize the host-returned string as a string id.
pub(crate) const BASE_FUNC_COPY_DYNAMIC_STRING: u32 = 27;
