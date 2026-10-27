// Global indices of base_module.wat, they are significant and must match its
// declaration order.
pub(crate) const CURRENT_INSTANCE_SYMBOL_INDEX: u32 = 0;
/// Global which stores number of all symbols, it is patched with real value
/// when the base module is merged with daedalus data.
pub(crate) const SYMBOLS_NUM_GLOBAL_INDEX: u32 = 2;
/// Globals which store symbol indices of the well known daedalus instance
/// variables, they are patched with real values (or -1 when the script does
/// not declare them) during merge with daedalus data.
pub(crate) const SELF_SYMBOL_GLOBAL_INDEX: u32 = 3;
pub(crate) const OTHER_SYMBOL_GLOBAL_INDEX: u32 = 4;
pub(crate) const VICTIM_SYMBOL_GLOBAL_INDEX: u32 = 5;
pub(crate) const ITEM_SYMBOL_GLOBAL_INDEX: u32 = 6;
pub(crate) const HERO_SYMBOL_GLOBAL_INDEX: u32 = 7;
/// Global which stores number of all string pairs in memory, it is patched
/// with real value when the base module is merged with daedalus data.
pub(crate) const STRINGS_NUM_GLOBAL_INDEX: u32 = 8;
/// Global which stores base address of the `DYN_STRINGS` pool, it is patched
/// with real value when the base module is merged with daedalus data.
pub(crate) const DYN_POOL_BASE_GLOBAL_INDEX: u32 = 9;
/// Global which stores size in bytes of the symbol value word space, it is
/// patched with the real value when the base module is merged with daedalus
/// data. The host uses it to size its symbol store.
pub(crate) const SYMBOLS_MEMORY_SIZE_GLOBAL_INDEX: u32 = 12;
/// Global caching the instance id of the well known `self` variable, so
/// `SetInstance(self)` does not need a host call. It is updated by the
/// `set_self` export.
pub(crate) const SELF_INSTANCE_GLOBAL_INDEX: u32 = 13;

/// Number of functions imported by the `symbols` WIT interface. They are added
/// before the daedalus external function imports and shift the base module
/// function index space.
pub(crate) const SYMBOLS_IMPORTS_NUM: u32 = 4;
/// Index of `get-symbol` in the final function index space.
pub(crate) const SYMBOL_IMPORT_GET_SYMBOL: u32 = 0;
/// Index of `set-symbol` in the final function index space.
pub(crate) const SYMBOL_IMPORT_SET_SYMBOL: u32 = 1;
/// Index of `get-member` in the final function index space.
pub(crate) const SYMBOL_IMPORT_GET_MEMBER: u32 = 2;
/// Index of `set-member` in the final function index space.
pub(crate) const SYMBOL_IMPORT_SET_MEMBER: u32 = 3;

/// Index of `set_instance` buildin function inside base module (see
/// `base_module.wat`), converted daedalus `SetInstance` instructions call it.
pub(crate) const BASE_FUNC_SET_INSTANCE: u32 = 2;
/// Index of `cabi_realloc` buildin function inside base module (see
/// `base_module.wat`), string-returning external wrappers use it to allocate
/// the lowered string return area.
pub(crate) const BASE_FUNC_CABI_REALLOC: u32 = 1;
/// Index of `copy_dynamic_string` buildin function inside base module (see
/// `base_module.wat`), converted string-returning external calls use it to
/// materialize the host-returned string as a string id.
pub(crate) const BASE_FUNC_COPY_DYNAMIC_STRING: u32 = 14;
