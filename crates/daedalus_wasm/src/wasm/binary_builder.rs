use crate::common::ToU32 as _;
use crate::daedalus;
use crate::wasm::BASE_FUNC_SET_INSTANCE;
use crate::wasm::DaedalusMemoryLayout;
use crate::wasm::create_wasm_binary;
use crate::wasm::function::{ConvertedFunction, convert_daedalus_func};
use crate::wasm::wasm_binary::{BaseModule, create_wasm_component_model};
use std::collections::HashMap;
use wasm_encoder::ValType;

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

        self.convert_functions();
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
        let results = func.call_signature.results();
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
