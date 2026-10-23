use super::constants::DYN_POOL_BASE_GLOBAL_INDEX;
use super::constants::HERO_SYMBOL_GLOBAL_INDEX;
use super::constants::ITEM_SYMBOL_GLOBAL_INDEX;
use super::constants::OTHER_SYMBOL_GLOBAL_INDEX;
use super::constants::SELF_SYMBOL_GLOBAL_INDEX;
use super::constants::STRINGS_NUM_GLOBAL_INDEX;
use super::constants::SYMBOLS_NUM_GLOBAL_INDEX;
use super::constants::VICTIM_SYMBOL_GLOBAL_INDEX;
use crate::common::ToU32 as _;
use crate::common::ToUsize as _;
use crate::daedalus::Symbol;
use crate::wasm::ConvertedFunction;
use crate::wasm::DaedalusMemoryLayout;
use crate::wasm::ImportedFunction;
use wasm_encoder::reencode::{Reencode as _, RoundtripReencoder};
use wasm_encoder::{
    CodeSection, ConstExpr, DataSection, ElementSection, Elements, EntityType, ExportKind,
    ExportSection, Function, FunctionSection, GlobalSection, GlobalType, ImportSection,
    Instruction, MemorySection, MemoryType, Module, TableSection, TableType, TypeSection, ValType,
};
use wasmparser::Parser;
use wit_component::{ComponentEncoder, StringEncoding, embed_component_metadata};
use wit_parser::Resolve;

/// Base module containing memory definitions, globals and buildin functions.
/// It is extended with daedalus script functionality when creating final binary.
const BASE_MODULE_WAT: &str = include_str!("base_module.wat");

#[derive(Debug, Default)]
/// Representation of the base module (see `base_module.wat`) parsed with `wasmparser`.
/// Function indices are already adjusted to the final function index space
/// (shifted by the number of daedalus imported functions).
pub struct BaseModule {
    types: Vec<(Vec<ValType>, Vec<ValType>)>,
    imports: Vec<(String, String, u32)>,
    /// Type indices of the buildin functions defined inside base module,
    /// their count is the number of functions in the final function index
    /// space taken by the base module.
    pub(crate) functions: Vec<u32>,
    tables: Vec<TableType>,
    memories: Vec<MemoryType>,
    globals: Vec<GlobalType>,
    exports: Vec<(String, ExportKind, u32)>,
    code: Vec<Function>,
}

pub fn create_wasm_component_model(module: Module) -> Vec<u8> {
    let mut resolve = Resolve::default();
    // The component must embed the same WIT world that the host bindings in
    // tests are generated from, so resolve this crate's own wit package.
    // Resolved relative to the manifest dir instead of the process CWD, so it
    // works no matter where tests or the app are started from.
    let wit_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/wit");
    let (pkg, _map) = resolve.push_dir(wit_dir).unwrap();

    let world = resolve.select_world(&[pkg], Some("mm-world"));
    let mut module_bytes = module.finish();

    embed_component_metadata(
        &mut module_bytes,
        &resolve,
        world.unwrap(),
        StringEncoding::UTF8,
    )
    .unwrap();

    // std::fs::write("aa_test.wasm", &module_bytes).unwrap();

    let component_bytes = ComponentEncoder::default()
        .module(&module_bytes)
        .unwrap()
        .validate(true)
        .encode()
        .unwrap();

    // std::fs::write("bb_test_component.wasm", &module_bytes).unwrap();

    component_bytes
}

#[must_use]
#[expect(clippy::too_many_lines, reason = "This contains logical code blocks")]
/// Creates core wasm module from the base module (memories, globals, buildin functions)
/// extended with daedalus script functions, data and tables.
pub fn create_wasm_binary(
    base: &BaseModule,
    imported_functions: &[ImportedFunction],
    converted_functions: &[ConvertedFunction],
    symbols: &[Symbol],
    memory_layout: &DaedalusMemoryLayout,
) -> Module {
    let func_index_offset = imported_functions.len().to_u32();
    assert!(
        base.imports.is_empty(),
        "Base module should not define imports, they would shift already adjusted indices"
    );

    let mut module = Module::new();

    // Type section: base types and types for daedalus imported and converted functions
    let base_types_num = base.types.len().to_u32();
    let mut types = TypeSection::new();
    for (params, results) in &base.types {
        types.ty().function(params.clone(), results.clone());
    }
    for func in imported_functions {
        types
            .ty()
            .function(func.params.clone(), func.results.clone());
    }
    let converted_types_start = base_types_num + imported_functions.len().to_u32();
    for func in converted_functions {
        types
            .ty()
            .function(func.params.clone(), func.results.clone());
    }
    module.section(&types);

    // Import section
    let mut imports = ImportSection::new();
    for (index, func) in imported_functions.iter().enumerate() {
        imports.import(
            &func.module,
            &func.name,
            EntityType::Function(base_types_num + index.to_u32()),
        );
    }
    module.section(&imports);

    // Function section
    let converted_functions_start = func_index_offset + base.functions.len().to_u32();
    let mut functions = FunctionSection::new();
    for type_index in &base.functions {
        functions.function(*type_index);
    }
    for (index, _converted_func) in converted_functions.iter().enumerate() {
        functions.function(converted_types_start + index.to_u32());
    }
    module.section(&functions);

    // Table section: extend function table to fit all daedalus functions
    let table_needed_size = converted_functions.len() as u64;
    let mut function_table = TableSection::new();
    for base_table in &base.tables {
        let mut table_type = *base_table;
        table_type.minimum = table_type.minimum.max(table_needed_size);
        function_table.table(table_type);
    }
    module.section(&function_table);

    // Memory section: extend memories to sizes required by daedalus data
    let static_strings_bytes = memory_layout.get_static_strings_bytes();
    let needed_pages = [
        memory_pages_for_layout(static_strings_bytes.len().to_u32()),
        memory_pages_for_layout(memory_layout.symbol_info_table_size()),
        memory_pages_for_layout(memory_layout.symbols_size()),
    ];
    let mut memory = MemorySection::new();
    for (index, base_memory) in base.memories.iter().enumerate() {
        let mut memory_type = *base_memory;
        if let Some(pages) = needed_pages.get(index) {
            memory_type.minimum = *pages;
        }
        memory.memory(memory_type);
    }
    module.section(&memory);

    // Global section: patch number of symbols and strings globals with real
    // values and symbol indices of the well known daedalus instance variables,
    // resolved by name
    let symbols_num = i32::try_from(symbols.len()).unwrap();
    let strings_num = i32::try_from(memory_layout.static_strings.len()).unwrap();
    let well_known_init = |name: &str| {
        symbols
            .iter()
            .position(|symbol| symbol.name.eq_ignore_ascii_case(name))
            .map_or(-1, |index| i32::try_from(index).unwrap())
    };
    let mut globals = GlobalSection::new();
    for (global_index, global_type) in base.globals.iter().enumerate() {
        let init = match global_index.to_u32() {
            SYMBOLS_NUM_GLOBAL_INDEX => symbols_num,
            STRINGS_NUM_GLOBAL_INDEX => strings_num,
            DYN_POOL_BASE_GLOBAL_INDEX => memory_layout.dyn_pool_base().cast_signed(),
            SELF_SYMBOL_GLOBAL_INDEX => well_known_init("self"),
            OTHER_SYMBOL_GLOBAL_INDEX => well_known_init("other"),
            VICTIM_SYMBOL_GLOBAL_INDEX => well_known_init("victim"),
            ITEM_SYMBOL_GLOBAL_INDEX => well_known_init("item"),
            HERO_SYMBOL_GLOBAL_INDEX => well_known_init("hero"),
            _ => 0,
        };
        globals.global(*global_type, &ConstExpr::i32_const(init));
    }
    module.section(&globals);

    // Export section: base exports (function indices already shifted)
    // and exports of functions which expose wasm component interface
    let mut exports = ExportSection::new();
    for (name, kind, index) in &base.exports {
        exports.export(name, *kind, *index);
    }
    for (index, converted_func) in converted_functions.iter().enumerate() {
        // Export test-func
        if converted_func.name.contains("mmcomando:zengine-scripts") {
            exports.export(
                &converted_func.name,
                ExportKind::Func,
                converted_functions_start + index.to_u32(),
            );
        }
    }
    module.section(&exports);

    // Element section
    let mut element_section = ElementSection::new();
    let function_refs: Vec<u32> = (0..converted_functions.len().to_u32())
        .map(|el| converted_functions_start + el)
        .collect();
    element_section.active(
        Some(0),
        &ConstExpr::i32_const(0),
        Elements::Functions(function_refs.into()),
    );
    module.section(&element_section);

    // Code section: base code (call indices already shifted) and daedalus code
    let mut code = CodeSection::new();
    for base_func in &base.code {
        code.function(base_func);
    }
    for converted_func in converted_functions {
        code.function(&converted_func.func);
    }
    module.section(&code);

    // Data section
    let mut data = DataSection::new();
    // Build in strings
    save_non_zero_data(
        &mut data,
        DaedalusMemoryLayout::MAIN_MEMORY,
        &static_strings_bytes,
    );

    let symbol_info_data = memory_layout.symbol_info_bytes();
    save_non_zero_data(
        &mut data,
        DaedalusMemoryLayout::SYMBOL_INFO_MEMORY_INDEX,
        &symbol_info_data,
    );

    let symbols_data = symbols_memory_bytes(symbols, memory_layout);
    save_non_zero_data(
        &mut data,
        DaedalusMemoryLayout::SYMBOLS_MEMORY_INDEX,
        &symbols_data,
    );

    module.section(&data);
    module
}

#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "Base module can contain sections which we don't convert"
)]
#[expect(clippy::too_many_lines, reason = "Parser covers all wasm sections")]
#[must_use]
/// Parses the base module (see `base_module.wat`) with `wasmparser`, its
/// function indices are adjusted to the final function index space where
/// `imported_functions_num` daedalus imported functions come first.
pub fn parse_base_module(imported_functions_num: u32) -> BaseModule {
    let base_bytes = wat::parse_str(BASE_MODULE_WAT).expect("Base module WAT should be valid");
    let mut base = BaseModule::default();

    for payload in Parser::new(0).parse_all(&base_bytes) {
        match payload.unwrap() {
            wasmparser::Payload::TypeSection(section) => {
                for rec_group in section {
                    let rec_group = rec_group.unwrap();
                    for sub_type in rec_group.types() {
                        let func_type = sub_type.unwrap_func();
                        base.types.push((
                            func_type
                                .params()
                                .iter()
                                .copied()
                                .map(convert_val_type)
                                .collect(),
                            func_type
                                .results()
                                .iter()
                                .copied()
                                .map(convert_val_type)
                                .collect(),
                        ));
                    }
                }
            }
            wasmparser::Payload::ImportSection(section) => {
                for imports in section {
                    let imports = imports.unwrap();
                    for import in imports {
                        let (_offset, import) = import.unwrap();
                        let wasmparser::TypeRef::Func(type_index) = import.ty else {
                            panic!("Only function imports are supported in base module");
                        };
                        base.imports.push((
                            import.module.to_owned(),
                            import.name.to_owned(),
                            type_index,
                        ));
                    }
                }
            }
            wasmparser::Payload::FunctionSection(section) => {
                for type_index in section {
                    base.functions.push(type_index.unwrap());
                }
            }
            wasmparser::Payload::TableSection(section) => {
                for table in section {
                    let table = table.unwrap();
                    base.tables.push(TableType {
                        element_type: convert_ref_type(table.ty.element_type),
                        table64: table.ty.table64,
                        minimum: table.ty.initial,
                        maximum: table.ty.maximum,
                        shared: table.ty.shared,
                    });
                }
            }
            wasmparser::Payload::MemorySection(section) => {
                for memory_type in section {
                    let memory_type = memory_type.unwrap();
                    base.memories.push(MemoryType {
                        memory64: memory_type.memory64,
                        shared: memory_type.shared,
                        page_size_log2: memory_type.page_size_log2,
                        minimum: memory_type.initial,
                        maximum: memory_type.maximum,
                    });
                }
            }
            wasmparser::Payload::GlobalSection(section) => {
                for global in section {
                    let global = global.unwrap();
                    base.globals.push(GlobalType {
                        val_type: convert_val_type(global.ty.content_type),
                        mutable: global.ty.mutable,
                        shared: global.ty.shared,
                    });
                }
            }
            wasmparser::Payload::ExportSection(section) => {
                for export in section {
                    let export = export.unwrap();
                    let index = match export.kind {
                        wasmparser::ExternalKind::Func => export.index + imported_functions_num,
                        _ => export.index,
                    };
                    base.exports.push((
                        export.name.to_owned(),
                        convert_export_kind(export.kind),
                        index,
                    ));
                }
            }
            wasmparser::Payload::CodeSectionEntry(body) => {
                base.code
                    .push(convert_base_function(&body, imported_functions_num));
            }
            _ => {}
        }
    }

    base
}

#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "Operators which don't need conversion are skipped"
)]
fn convert_base_function(body: &wasmparser::FunctionBody, func_index_offset: u32) -> Function {
    let mut locals_reader = body.get_locals_reader().unwrap();
    let mut locals = Vec::new();
    for _local_declaration_index in 0..locals_reader.get_count() {
        let (count, val_type) = locals_reader.read().unwrap();
        locals.push((count, convert_val_type(val_type)));
    }

    let mut converted = Function::new(locals);
    let mut operators = body.get_operators_reader().unwrap();
    let mut block_depth = 0_u32;

    loop {
        let operator = operators.read().unwrap();
        match operator {
            // Function body ends when the last block is closed
            wasmparser::Operator::End if block_depth == 0 => {
                converted.instruction(&Instruction::End);
                break;
            }
            wasmparser::Operator::End => {
                block_depth -= 1;
                converted.instruction(&Instruction::End);
            }
            wasmparser::Operator::Block { .. }
            | wasmparser::Operator::Loop { .. }
            | wasmparser::Operator::If { .. } => {
                block_depth += 1;
                converted.instruction(&convert_operator(operator, func_index_offset));
            }
            op => {
                converted.instruction(&convert_operator(op, func_index_offset));
            }
        }
    }

    converted
}

fn convert_operator(operator: wasmparser::Operator<'_>, func_index_offset: u32) -> Instruction<'_> {
    use wasmparser::Operator as Source;
    if let Source::Call { function_index } = &operator {
        return Instruction::Call(function_index + func_index_offset);
    }
    RoundtripReencoder.instruction(operator).unwrap().clone()
}

fn convert_val_type(val_type: wasmparser::ValType) -> ValType {
    RoundtripReencoder.val_type(val_type).unwrap()
}

fn convert_ref_type(ref_type: wasmparser::RefType) -> wasm_encoder::RefType {
    RoundtripReencoder.ref_type(ref_type).unwrap()
}

fn convert_export_kind(kind: wasmparser::ExternalKind) -> ExportKind {
    RoundtripReencoder.export_kind(kind).unwrap()
}

fn save_non_zero_data(data: &mut DataSection, memory_index: u32, bytes: &[u8]) {
    // It takes more space to define section than to skip few zeros.
    // Limit size of one active block we define.
    let skip_gap = 30;

    let mut i = 0;

    while i < bytes.len() {
        // Find the start of the next non-zero segment
        let start = match bytes[i..].iter().position(|el| *el != 0) {
            Some(pos) => i + pos,
            None => break,
        };

        // Find the end: the first zero that is > `skip_gap` away from the `start`
        let end = bytes[start..]
            .iter()
            .enumerate()
            .find(|&(offset, &byte)| byte == 0 && offset > skip_gap)
            .map_or(bytes.len(), |(offset, _)| start + offset);

        // Save the segment
        data.active(
            memory_index,
            &ConstExpr::i32_const(i32::try_from(start).unwrap()),
            bytes[start..end].iter().copied(),
        );

        // Advance
        i = end;
    }
}

fn memory_pages_for_layout(bytes_num: u32) -> u64 {
    let total_size = u64::from(bytes_num.max(4));
    let pages = total_size.div_ceil(0xFFFF);
    pages.max(1)
}

fn symbols_memory_bytes(symbols: &[Symbol], memory_layout: &DaedalusMemoryLayout) -> Vec<u8> {
    let mut bytes = vec![0; memory_layout.symbols_size().to_usize()];

    for (index, symbol) in symbols.iter().enumerate() {
        if symbol.is_class_var {
            continue;
        }
        if symbol.is_const {
            continue;
        }

        let symbol_address = memory_layout.symbols[index].address_or_offset.to_usize();
        for (word_index, value) in symbol
            .default_values
            .iter()
            .take(symbol.elements_count.to_usize())
            .enumerate()
        {
            let offset = symbol_address + word_index * 4;
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
    }

    bytes
}
