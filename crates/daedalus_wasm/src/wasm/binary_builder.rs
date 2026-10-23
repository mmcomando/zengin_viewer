use super::constants::BASE_FUNC_CABI_REALLOC;
use super::constants::BASE_FUNC_COPY_DYNAMIC_STRING;
use super::constants::BASE_FUNC_SET_INSTANCE;
use crate::common::ToU32 as _;
use crate::common::ToUsize as _;
use crate::daedalus;
use crate::wasm::DaedalusMemoryLayout;
use crate::wasm::create_wasm_binary;
use crate::wasm::function::{ConvertedFunction, convert_daedalus_func};
use crate::wasm::wasm_binary::{BaseModule, create_wasm_component_model};
use core::mem::take;
use std::collections::HashMap;
use wasm_encoder::Instruction;
use wasm_encoder::MemArg;
use wasm_encoder::ValType;

/// Lowercased names of external functions whose WIT result is a `list<u8>`. Their
/// lowered imports return the bytes through a return area, and a generated
/// wrapper function copies them into the `DYN_STRINGS` ring so the daedalus
/// code sees an ordinary string id.
const STRING_RETURNING_EXTERNALS: [&str; 3] = ["concatstrings", "inttostring", "floattostring"];

/// Prepared specification of a string-returning external wrapper, see
/// `BinaryBuilder::prepare_string_return_wrappers`.
#[derive(Debug)]
struct StringReturnWrapper {
    import_index: u32,
    params_num: usize,
    name: String,
}

#[derive(Debug)]
pub(crate) struct ImportedFunction {
    pub module: String,
    pub name: String,
    pub params: Vec<ValType>,
    pub results: Vec<ValType>,
}

#[derive(Debug)]
pub(crate) struct FunctionInformation {
    pub(crate) wasm_function_index: u32,
    pub(crate) signature: daedalus::CallSignature,
}

#[derive(Debug, Default)]
pub(crate) struct SymbolInformation {
    pub(crate) memory_address: Option<u32>,
    pub(crate) member_offset: Option<u32>,
    pub(crate) const_values: Option<Vec<i32>>,
    pub(crate) func_info: Option<FunctionInformation>,
}

#[expect(
    clippy::arbitrary_source_item_ordering,
    reason = "Better for readability"
)]
#[derive(Debug, Default)]
/// Builder used to convert daedalus scripts to wasm scripts.
pub struct BinaryBuilder {
    pub(crate) functions: Vec<ConvertedFunction>,
    pub(crate) imported_functions: Vec<ImportedFunction>,
    pub(crate) memory: DaedalusMemoryLayout,
    pub(crate) symbols: Vec<daedalus::Symbol>,

    // Only compile selected functions
    pub(crate) whitelisted_functions: Vec<String>,

    // Index of `set_instance` buildin function in the final function index space
    pub(crate) func_index_set_instance: Option<u32>,

    /// External functions whose result is a string: (symbol index, import
    /// function index). Wrappers are generated after all imports are known.
    string_return_externals: Vec<(u32, u32)>,

    /// Prepared wrapper specifications, function bodies are pushed after the
    /// converted daedalus functions so their indices do not shift.
    string_return_wrappers: Vec<StringReturnWrapper>,

    /// Base module with function indices adjusted to the final function index
    /// space, shifted by the number of imported functions.
    base: BaseModule,

    // Symbols info
    pub(crate) symbol_to_info: HashMap<u32, SymbolInformation>,
    // Daedalus function to wasm function mapping
    pub(crate) instr_offset_to_func_info: HashMap<u32, FunctionInformation>,
}

impl BinaryBuilder {
    /// Whitelist which daedalus function symbols to convert to wasm functions.
    /// Main purpose for this function is testing.
    /// If not called or `whitelisted_functions` is empty, all functions will be compiled.
    pub fn whitelist_functions(&mut self, whitelisted_functions: &[&str]) {
        self.whitelisted_functions = whitelisted_functions
            .iter()
            .map(|el| (*el).to_owned())
            .collect();
    }

    /// Add function which can be used for testing from wasm component interface.
    pub fn add_test_function(&mut self, instructions: &[daedalus::Instruction]) {
        let dd_func = daedalus::Function::new(
            "mmcomando:zengine-scripts/manage@0.1.0#test-func",
            daedalus::CallSignature::new(0, true),
            instructions,
        );

        let func = convert_daedalus_func(self, &dd_func, self.func_index_set_instance.unwrap());
        self.functions.push(func);
    }

    #[must_use]
    fn compile_func(&self, symbol: &daedalus::Symbol) -> bool {
        if self.whitelisted_functions.is_empty() {
            return true;
        }
        let Some(func) = &symbol.function else {
            return true;
        };
        if func.is_external {
            return true;
        }

        if !self.whitelisted_functions.contains(&func.name) {
            return false;
        }
        true
    }

    /// Add daedalus symbols to be included in the wasm binary.
    /// Symbols should have continuous index.
    pub fn add_symbols(&mut self, symbols: &[daedalus::Symbol]) {
        // Add external functions (wasm requires them to be defined before other functions)
        for (index, symbol) in symbols.iter().enumerate() {
            assert!(
                usize::try_from(symbol.index).unwrap() == index,
                "Symbols should have continuous index"
            );
            let Some(func) = &symbol.function else {
                continue;
            };
            if !func.is_external {
                continue;
            }
            self.add_external_function(symbol, func);
        }

        self.symbols = symbols.to_vec();

        // Buildin functions are defined in the base module (base_module.wat),
        // here only their indices in the final function index space are computed.
        self.func_index_set_instance =
            Some(self.imported_functions.len().to_u32() + BASE_FUNC_SET_INSTANCE);
        let base_functions_num = self.base.functions.len().to_u32();

        let mut internal_func_index = 0;
        for symbol in symbols {
            self.add_symbol_memory_info(symbol, internal_func_index);
            if let Some(func) = &symbol.function
                && !func.is_external
                && self.compile_func(symbol)
            {
                let index = base_functions_num
                    + self.imported_functions.len().to_u32()
                    + internal_func_index;
                self.instr_offset_to_func_info.insert(
                    func.instructions_offset,
                    FunctionInformation {
                        wasm_function_index: index,
                        signature: func.call_signature,
                    },
                );
                internal_func_index += 1;
            }
        }

        self.prepare_string_return_wrappers();
        self.convert_functions();
        self.push_string_return_wrappers();
    }

    /// Add static strings to memory.
    pub fn add_static_strings(&mut self, strings: &[String]) {
        for str in strings {
            self.memory.add_string(str);
        }
    }

    fn add_external_function(&mut self, symbol: &daedalus::Symbol, func: &daedalus::Function) {
        assert!(
            func.is_external,
            "Imported functions should be already present"
        );
        assert!(
            self.functions.is_empty(),
            "Imported functions should be added before other functions"
        );

        let index = self.imported_functions.len().to_u32();
        let info = SymbolInformation {
            func_info: Some(FunctionInformation {
                wasm_function_index: index,
                signature: func.call_signature,
            }),
            ..Default::default()
        };
        let params = func.call_signature.parameters();
        let is_string_return = Self::is_string_returning_external(&func.name);
        let results = if is_string_return {
            // A `string` result does not fit into the flat result slot, the
            // lowered import takes an extra trailing return-area pointer and
            // returns nothing (canonical ABI, see `wit-parser` `wasm_signature`)
            self.string_return_externals.push((symbol.index, index));
            vec![]
        } else {
            func.call_signature.results()
        };
        let params = if is_string_return {
            let mut lowered_params = params.clone();
            lowered_params.push(ValType::I32);
            lowered_params
        } else {
            params
        };
        // func.print_wit_declaration();

        let imported_func = ImportedFunction {
            module: "mmcomando:zengine-scripts/scripts@0.1.0".to_owned(),
            name: func.name.replace('_', "-"),
            params,
            results,
        };
        self.imported_functions.push(imported_func);

        self.symbol_to_info.insert(symbol.index, info);
    }

    /// Computes the final function indices of the string-returning wrappers
    /// and points the external symbols at them, so `CallExternal` compilation
    /// calls the wrapper instead of the lowered import directly.
    fn prepare_string_return_wrappers(&mut self) {
        let externals = take(&mut self.string_return_externals);
        if externals.is_empty() {
            return;
        }

        let imported_num = self.imported_functions.len().to_u32();
        let base_functions_num = self.base.functions.len().to_u32();
        // Converted daedalus functions occupy the converted index space before
        // the wrappers, their count is known without converting
        let converted_funcs_num = self
            .symbols
            .iter()
            .filter(|symbol| {
                symbol
                    .function
                    .as_ref()
                    .is_some_and(|func| !func.is_external)
            })
            .filter(|symbol| self.compile_func(symbol))
            .count()
            .to_u32();

        for (ordinal, (symbol_index, import_index)) in externals.into_iter().enumerate() {
            let symbol = &self.symbols[symbol_index.to_usize()];
            let func = symbol.function.as_ref().unwrap();
            let params_num = func.call_signature.parameters().len();
            let wrapper_index =
                imported_num + base_functions_num + converted_funcs_num + ordinal.to_u32();
            let info = self.symbol_to_info.get_mut(&symbol_index).unwrap();
            let func_info = info.func_info.as_mut().unwrap();
            func_info.wasm_function_index = wrapper_index;
            self.string_return_wrappers.push(StringReturnWrapper {
                import_index,
                params_num,
                name: func.name.clone(),
            });
        }
    }

    /// Builds the wrapper function bodies and appends them after the converted
    /// daedalus functions, matching the indices computed by
    /// `prepare_string_return_wrappers`.
    fn push_string_return_wrappers(&mut self) {
        let imported_num = self.imported_functions.len().to_u32();
        let realloc_index = imported_num + BASE_FUNC_CABI_REALLOC;
        let copy_dyn_index = imported_num + BASE_FUNC_COPY_DYNAMIC_STRING;

        let wrappers = take(&mut self.string_return_wrappers);
        for wrapper in wrappers {
            let func = Self::build_string_return_wrapper(
                wrapper.import_index,
                wrapper.params_num,
                realloc_index,
                copy_dyn_index,
            );
            self.functions.push(ConvertedFunction {
                func,
                name: format!("dyn-string-wrapper-{}", wrapper.name),
                params: vec![ValType::I32; wrapper.params_num],
                results: vec![ValType::I32],
            });
        }
    }

    /// Wrapper body: calls the lowered import with an extra return-area
    /// pointer, reads the (ptr, len) bytes pair the host wrote into it and
    /// copies the bytes into the `DYN_STRINGS` ring. The daedalus-visible result
    /// is the ring string id.
    fn build_string_return_wrapper(
        import_index: u32,
        params_num: usize,
        realloc_index: u32,
        copy_dyn_index: u32,
    ) -> wasm_encoder::Function {
        let mut func = wasm_encoder::Function::new([(3_u32, ValType::I32)]);
        let ret_area = params_num.to_u32();
        let str_ptr = ret_area + 1;
        let str_len = ret_area + 2;

        // Allocate the return area for the lowered (ptr, len) string result
        func.instruction(&Instruction::I32Const(0));
        func.instruction(&Instruction::I32Const(0));
        func.instruction(&Instruction::I32Const(4));
        func.instruction(&Instruction::I32Const(8));
        func.instruction(&Instruction::Call(realloc_index));
        func.instruction(&Instruction::LocalSet(ret_area));

        // Forward the daedalus parameters
        for index in 0..params_num.to_u32() {
            func.instruction(&Instruction::LocalGet(index));
        }
        func.instruction(&Instruction::LocalGet(ret_area));
        func.instruction(&Instruction::Call(import_index));

        // Read the (ptr, len) pair the host wrote into the return area
        func.instruction(&Instruction::LocalGet(ret_area));
        func.instruction(&Instruction::I32Load(MemArg {
            offset: 0,
            align: 2,
            memory_index: DaedalusMemoryLayout::MAIN_MEMORY,
        }));
        func.instruction(&Instruction::LocalSet(str_ptr));
        func.instruction(&Instruction::LocalGet(ret_area));
        func.instruction(&Instruction::I32Load(MemArg {
            offset: 4,
            align: 2,
            memory_index: DaedalusMemoryLayout::MAIN_MEMORY,
        }));
        func.instruction(&Instruction::LocalSet(str_len));

        // Copy the string into the DYN_STRINGS ring, its id is the result
        func.instruction(&Instruction::LocalGet(str_ptr));
        func.instruction(&Instruction::LocalGet(str_len));
        func.instruction(&Instruction::Call(copy_dyn_index));

        func.instruction(&Instruction::End);
        func
    }

    fn is_string_returning_external(name: &str) -> bool {
        STRING_RETURNING_EXTERNALS
            .iter()
            .any(|el| name.eq_ignore_ascii_case(el))
    }

    fn convert_functions(&mut self) {
        for symbol in &self.symbols {
            let Some(dd_func) = &symbol.function else {
                continue;
            };
            if dd_func.is_external {
                continue;
            }
            if !self.compile_func(symbol) {
                continue;
            }
            let func = convert_daedalus_func(self, dd_func, self.func_index_set_instance.unwrap());
            self.functions.push(func);
        }
    }

    fn add_symbol_memory_info(&mut self, symbol: &daedalus::Symbol, internal_func_index: u32) {
        let mut info = SymbolInformation::default();

        if let Some(func) = &symbol.function {
            let wasm_index = if func.is_external {
                self.symbol_to_info
                    .get(&symbol.index)
                    .unwrap()
                    .func_info
                    .as_ref()
                    .unwrap()
                    .wasm_function_index
            } else {
                u32::try_from(self.imported_functions.len()).unwrap() + internal_func_index
            };

            info.func_info = Some(FunctionInformation {
                wasm_function_index: wasm_index,
                signature: func.call_signature,
            });
        }

        if symbol.is_function_with_code() {
            self.memory.add_symbol(symbol, Some(internal_func_index));
        } else {
            self.memory.add_symbol(symbol, None);
        }

        if let Some(address) = self.memory.get_global_address(symbol.index) {
            info.memory_address = Some(address);
        }
        if let Some(offset) = self.memory.get_member_offset(symbol.index) {
            info.member_offset = Some(offset);
        }
        if symbol.is_const {
            info.const_values = Some(symbol.default_values.clone());
        }
        self.symbol_to_info.insert(symbol.index, info);
    }

    #[must_use]
    /// Consume builder and return wasm component model binary.
    pub fn finalize(self) -> Vec<u8> {
        let module = create_wasm_binary(
            &self.base,
            &self.imported_functions,
            &self.functions,
            &self.symbols,
            &self.memory,
        );
        create_wasm_component_model(module)
    }

    #[must_use]
    /// Create builder instance.
    pub fn new(base: BaseModule) -> Self {
        Self {
            base,
            ..Self::default()
        }
    }
}
