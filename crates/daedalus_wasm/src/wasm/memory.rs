use crate::common::ToU32 as _;
use crate::common::ToUsize as _;
use crate::daedalus::Symbol;
use crate::daedalus::SymbolKind;

#[derive(Debug, Clone)]
pub(crate) struct SymbolMemoryInfo {
    pub(crate) kind: SymbolKind,
    pub(crate) is_class_member: bool,
    /// Whether a function symbol returns a value (daedalus `func int`).
    /// Needed by `call_function_discard`, which must call functions returning
    /// values with a different indirect call type than void functions.
    pub(crate) returns_value: bool,
    pub(crate) address_or_offset: u32,
    pub(crate) func_index: u32,
    pub(crate) parent: u32,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct DaedalusMemoryLayout {
    pub(crate) static_strings: Vec<String>,
    pub(crate) symbols: Vec<SymbolMemoryInfo>,
    pub(crate) symbol_memory_size: u32,
}

/// Number of `DYN_STRINGS` ring slots, dynamic string ids are
/// `strings_num .. strings_num + DYN_STRINGS_SLOTS_NUM`.
pub(crate) const DYN_STRINGS_SLOTS_NUM: u32 = 64;
/// Size in bytes of the `DYN_STRINGS` content pool shared by all ring slots,
/// bump-allocated with wrapping so there is no per-string max length.
pub(crate) const DYN_STRINGS_POOL_SIZE: u32 = 8192;

impl SymbolMemoryInfo {
    pub(crate) const RECORD_SIZE: usize = 16;
    pub(crate) const KIND_OFFSET: usize = 0;
    pub(crate) const IS_CLASS_MEMBER_OFFSET: usize = 1;
    pub(crate) const RETURNS_VALUE_OFFSET: usize = 2;
    pub(crate) const ADDRESS_OR_OFFSET_OFFSET: usize = 4;
    pub(crate) const FUNC_INDEX_OFFSET: usize = 8;
    pub(crate) const PARENT_OFFSET: usize = 12;

    fn to_bytes(&self) -> [u8; Self::RECORD_SIZE] {
        let mut bytes = [0; Self::RECORD_SIZE];
        bytes[Self::KIND_OFFSET] = self.kind.to_num();
        bytes[Self::IS_CLASS_MEMBER_OFFSET] = u8::from(self.is_class_member);
        bytes[Self::RETURNS_VALUE_OFFSET] = u8::from(self.returns_value);
        bytes[Self::ADDRESS_OR_OFFSET_OFFSET..Self::ADDRESS_OR_OFFSET_OFFSET + 4]
            .copy_from_slice(&self.address_or_offset.to_le_bytes());
        bytes[Self::FUNC_INDEX_OFFSET..Self::FUNC_INDEX_OFFSET + 4]
            .copy_from_slice(&self.func_index.to_le_bytes());
        bytes[Self::PARENT_OFFSET..Self::PARENT_OFFSET + 4]
            .copy_from_slice(&self.parent.to_le_bytes());
        bytes
    }
}

impl DaedalusMemoryLayout {
    pub(crate) const ALLOCATOR_MEMORY_SIZE: u32 = 4096 * 4;
    pub(crate) const MAIN_MEMORY: u32 = 0;
    pub(crate) const SYMBOL_INFO_MEMORY_INDEX: u32 = 1;
    pub(crate) const SYMBOLS_MEMORY_INDEX: u32 = 2;

    pub(crate) fn add_symbol(&mut self, symbol: &Symbol, internal_func_index: Option<u32>) {
        self.static_strings.push(symbol.name.clone());

        let (is_class_member, address_or_offset) = if symbol.is_class_var {
            (true, symbol.offset.unwrap())
        } else {
            let address = self.symbol_memory_size;
            if !symbol.is_const {
                self.symbol_memory_size += symbol.size();
            }
            (false, address)
        };

        self.symbols.push(SymbolMemoryInfo {
            kind: symbol.kind,
            is_class_member,
            returns_value: symbol
                .function
                .as_ref()
                .is_some_and(|function| function.call_signature.returns),
            address_or_offset,
            func_index: internal_func_index.unwrap_or(u32::MAX),
            parent: symbol.parent.unwrap_or(u32::MAX),
        });
    }

    pub(crate) fn add_string(&mut self, string: &str) {
        self.static_strings.push(string.to_owned());
    }

    #[must_use]
    pub(crate) fn symbol_info_bytes(&self) -> Vec<u8> {
        self.symbols
            .iter()
            .flat_map(SymbolMemoryInfo::to_bytes)
            .collect()
    }

    #[must_use]
    pub(crate) fn symbol_info_table_size(&self) -> u32 {
        self.symbol_count() * SymbolMemoryInfo::RECORD_SIZE as u32
    }

    #[must_use]
    pub(crate) fn get_global_address(&self, index: u32) -> Option<u32> {
        let symbol = &self.symbols[index.to_usize()];
        if symbol.is_class_member {
            return None;
        }
        Some(symbol.address_or_offset)
    }

    #[must_use]
    pub(crate) fn get_member_offset(&self, index: u32) -> Option<u32> {
        let symbol = &self.symbols[index.to_usize()];
        if symbol.is_class_member {
            return Some(symbol.address_or_offset);
        }
        None
    }

    #[must_use]
    pub(crate) fn get_static_strings_bytes(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = vec![];

        let ptr_size_arr_size = usize::try_from(Self::ALLOCATOR_MEMORY_SIZE).unwrap()
            + 2 * size_of::<i32>() * self.static_strings.len()
            // DYN_STRINGS ring slots follow the static pairs, their wat
            // addressing is `16384 + 8 * (strings_num + slot)`
            + 2 * size_of::<i32>() * DYN_STRINGS_SLOTS_NUM.to_usize();
        bytes.reserve(ptr_size_arr_size);
        bytes.extend((0..Self::ALLOCATOR_MEMORY_SIZE).map(|_el| 0));

        let mut strings_total_size = 0;

        for static_string in &self.static_strings {
            bytes.extend(
                u32::try_from(strings_total_size + ptr_size_arr_size)
                    .unwrap()
                    .to_le_bytes(),
            );
            bytes.extend(u32::try_from(static_string.len()).unwrap().to_le_bytes());
            strings_total_size += static_string.len();
        }

        // DYN_STRINGS ring slots (zero-initialized, filled at runtime) sit
        // between the static pairs and the string bytes, their wat addressing
        // is `16384 + 8 * (strings_num + slot)`
        bytes.extend((0..2 * size_of::<i32>() * DYN_STRINGS_SLOTS_NUM.to_usize()).map(|_el| 0));

        bytes.reserve(strings_total_size);
        for static_string in &self.static_strings {
            bytes.extend(static_string.as_bytes());
        }

        // DYN_STRINGS content pool: zero-initialized, filled at runtime. The
        // zero ring slots were already placed before the string bytes
        bytes.extend((0..DYN_STRINGS_POOL_SIZE.to_usize()).map(|_el| 0));

        bytes
    }

    /// Base address of the `DYN_STRINGS` content pool in main memory: after
    /// the (ptr, len) pairs of all strings including the 64 dynamic ring slots.
    #[must_use]
    pub(crate) fn dyn_pool_base(&self) -> u32 {
        Self::ALLOCATOR_MEMORY_SIZE
            + 2 * size_of::<i32>() as u32
                * (self.static_strings.len() as u32 + DYN_STRINGS_SLOTS_NUM)
            + self
                .static_strings
                .iter()
                .map(|el| el.len().to_u32())
                .sum::<u32>()
    }

    #[must_use]
    pub(crate) fn symbol_count(&self) -> u32 {
        self.symbols.len().to_u32()
    }

    #[must_use]
    pub(crate) fn symbols_size(&self) -> u32 {
        self.symbol_memory_size
    }
}
