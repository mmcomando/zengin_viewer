#![allow(clippy::as_conversions, reason = "parser already validated the values")]

use crate::common::{ToU32 as _, ToUsize as _};
use crate::daedalus::Instruction;
use byteorder::{LittleEndian, ReadBytesExt as _};
use core::cmp::max;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{self, BufReader, Read as _};
use std::path::Path;
use wasm_encoder::ValType;

#[derive(Debug, Clone, Copy)]
/// Representation of function signature.
pub struct CallSignature {
    pub(crate) parameters_num: u32,
    pub(crate) returns: bool,
}

impl CallSignature {
    #[must_use]
    /// Create new signature.
    pub fn new(parameters_num: u32, returns: bool) -> Self {
        CallSignature {
            parameters_num,
            returns,
        }
    }

    #[must_use]
    pub(crate) fn parameters(self) -> Vec<ValType> {
        (0..self.parameters_num)
            .flat_map(|_el| [ValType::I32])
            .collect()
    }

    #[must_use]
    pub(crate) fn results(self) -> Vec<ValType> {
        if self.returns {
            return vec![ValType::I32];
        }
        vec![]
    }
}

#[derive(Debug, Default, Clone)]
/// Struct containing data from compiled daedalus script.
pub struct CompiledDaedalusScript {
    /// Vector of symbols contained in daedalus script.
    pub symbols: Vec<Symbol>,
    /// Static strings contained in this script file.
    pub static_strings: Vec<String>,
    /// Number of symbols contained in the original script file.
    /// Dynamically added symbols (e.g. strings) are placed after them.
    pub symbols_count: u32,
}

impl CompiledDaedalusScript {
    /// Load script from file.
    pub fn from_file(path: &Path) -> io::Result<CompiledDaedalusScript> {
        let mut dat_file = CompiledDaedalusScript::default();

        let file = File::open(path)?;
        let mut file = BufReader::new(file);

        let _version = file.read_u8()?;
        let symbol_count = file.read_u32::<LittleEndian>()?;

        for _ in 0..symbol_count {
            // It is used for something?
            let _num = file.read_u32::<LittleEndian>()?;
        }

        dat_file.symbols_count = symbol_count;
        for _ in 0..symbol_count {
            dat_file.parse_symbol(&mut file)?;
        }

        let _stack_length = file.read_u32::<LittleEndian>()?;

        let mut remaining = Vec::new();
        file.read_to_end(&mut remaining)?;

        let mut function_offsets = BTreeSet::new();
        for symbol in &mut dat_file.symbols {
            let Some(func) = &mut symbol.function else {
                continue;
            };
            if func.is_external {
                continue;
            }
            function_offsets.insert(func.instructions_offset);
        }

        for symbol in &mut dat_file.symbols {
            let Some(func) = &mut symbol.function else {
                continue;
            };
            if func.is_external {
                continue;
            }
            let next_func_offset = function_offsets
                .range(func.instructions_offset + 1..)
                .next()
                .copied()
                .unwrap_or(remaining.len().to_u32());

            let instructions = decode_bytecode(
                &remaining[func.instructions_offset.to_usize()..next_func_offset.to_usize()],
                func.instructions_offset.to_usize(),
            );
            func.instructions = instructions;
        }

        Ok(dat_file)
    }

    #[must_use]
    /// Get daedalus instruction offset for given function.
    pub fn get_function_address(&self, name: &str) -> Option<u32> {
        let symbol = self.get_symbol_by_name(name)?;
        Some(symbol.function.as_ref()?.instructions_offset)
    }

    #[must_use]
    pub(crate) fn get_symbol_by_name(&self, name: &str) -> Option<&Symbol> {
        self.symbols.iter().find(|el| el.name == name)
    }

    fn add_string(&mut self, string: String) -> u32 {
        let string_index = self.symbols_count + self.static_strings.len() as u32;
        self.static_strings.push(string);

        string_index
    }

    fn find_parent_elements_count(&self, parent_symbol_index: Option<u32>) -> u32 {
        let Some(parent_symbol_index) = parent_symbol_index else {
            return 0;
        };
        let parent_symbol = &self.symbols[parent_symbol_index as usize];
        max(
            parent_symbol.elements_count,
            self.find_parent_elements_count(parent_symbol.parent),
        )
    }

    #[expect(clippy::too_many_lines, reason = "For parsing we allow long functions")]
    pub(crate) fn parse_symbol(&mut self, file: &mut BufReader<File>) -> io::Result<()> {
        let _b_has_name = file.read_u32::<LittleEndian>()?;

        let name = read_utf8_string(file)?;
        let name_lowercase = name.to_lowercase(); // Normalize non display strings to be always lowercase
        let offset = file.read_u32::<LittleEndian>()?;
        let bitfield = file.read_u32::<LittleEndian>()?;
        let _file_nr = file.read_u32::<LittleEndian>()?;
        let _line = file.read_u32::<LittleEndian>()?;
        let _line_end = file.read_u32::<LittleEndian>()?;
        let _pos_begin = file.read_u32::<LittleEndian>()?;
        let _pos_end = file.read_u32::<LittleEndian>()?;

        let flags = Flags::from(bitfield);
        let kind = SymbolKind::from(bitfield);
        let elements_count = decode_data_len(bitfield);
        let index = self.symbols.len().to_u32();

        if flags.is_class_var() {
            let parent = file.read_u32::<LittleEndian>()?;
            if parent == u32::MAX {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Class variable has to have parent defined",
                ));
            }
            self.symbols.push(Symbol {
                name: name_lowercase,
                index,
                kind,
                is_class_var: flags.is_class_var(),
                is_const: flags.is_const(),
                elements_count,
                offset: Some(offset),
                default_values: vec![0; elements_count as usize],
                function: None,
                parent: make_parent_option(parent),
            });
            return Ok(());
        }

        match kind {
            SymbolKind::Class => {
                let _content = file.read_u32::<LittleEndian>()?; // Not sure what it is for
                let parent = file.read_u32::<LittleEndian>()?;
                if parent != u32::MAX {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Class definition should not have a parent",
                    ));
                }

                let elements_count2 = offset / 4;
                if offset % 4 != 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Size of class is multiple of 4",
                    ));
                }

                let parent_option = make_parent_option(parent);
                let my_elements_count = max(
                    elements_count2,
                    self.find_parent_elements_count(parent_option),
                );

                self.symbols.push(Symbol {
                    name: name_lowercase,
                    index,
                    kind,
                    is_class_var: flags.is_class_var(),
                    is_const: flags.is_const(),
                    elements_count: my_elements_count,
                    offset: None,
                    default_values: vec![0; my_elements_count as usize],
                    function: None,
                    parent: make_parent_option(parent),
                });
            }
            SymbolKind::Float => {
                let arr: Vec<f32> = (0..elements_count)
                    .map(|_index| file.read_f32::<LittleEndian>().unwrap())
                    .collect();
                let parent = file.read_u32::<LittleEndian>()?;
                if parent != u32::MAX {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Float should not have a parent",
                    ));
                }
                let default_values = arr
                    .iter()
                    .map(|value| value.to_bits().cast_signed())
                    .collect();
                self.symbols.push(Symbol {
                    name: name_lowercase,
                    index,
                    kind,
                    is_class_var: flags.is_class_var(),
                    is_const: flags.is_const(),
                    elements_count,
                    offset: None,
                    default_values,
                    function: None,
                    parent: make_parent_option(parent),
                });
            }
            SymbolKind::Int => {
                let arr: Vec<i32> = (0..elements_count)
                    .map(|_index| file.read_i32::<LittleEndian>().unwrap())
                    .collect();

                let parent = file.read_u32::<LittleEndian>()?;
                self.symbols.push(Symbol {
                    name: name_lowercase,
                    index,
                    kind,
                    is_class_var: flags.is_class_var(),
                    is_const: flags.is_const(),
                    elements_count,
                    offset: None,
                    default_values: arr,
                    function: None,
                    parent: make_parent_option(parent),
                });
            }
            SymbolKind::String => {
                let raw_strings: Vec<String> = (0..elements_count)
                    .map(|_index| read_utf8_string(file).unwrap())
                    .collect();
                let parent = file.read_u32::<LittleEndian>()?;

                let mut arr = vec![];
                for el in raw_strings {
                    let string_index = self.add_string(el);
                    arr.push(string_index.cast_signed());
                }
                self.symbols.push(Symbol {
                    name: name_lowercase,
                    index,
                    kind,
                    is_class_var: flags.is_class_var(),
                    is_const: flags.is_const(),
                    elements_count,
                    offset: None,
                    default_values: arr,
                    function: None,
                    parent: make_parent_option(parent),
                });
            }
            SymbolKind::Func => {
                let instructions_offset = file.read_u32::<LittleEndian>()?;
                let parent = file.read_u32::<LittleEndian>()?;
                if parent != u32::MAX {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Function should not have a parent",
                    ));
                }
                let func = if instructions_offset > 0 {
                    Some(Function {
                        name: name_lowercase.clone(),
                        call_signature: CallSignature::new(elements_count, flags.is_return()),
                        instructions: Vec::new(),
                        instructions_offset,
                        is_external: flags.is_external(),
                    })
                } else {
                    None
                };
                self.symbols.push(Symbol {
                    name: name_lowercase,
                    index,
                    kind,
                    is_class_var: flags.is_class_var(),
                    is_const: flags.is_const(),
                    elements_count,
                    offset: None,
                    default_values: vec![0; elements_count as usize],
                    function: func,
                    parent: make_parent_option(parent),
                });
            }
            SymbolKind::Instance => {
                let instructions_offset = file.read_u32::<LittleEndian>()?;
                let elements_count = elements_count.max(1);
                let arr: Vec<i32> = (0..elements_count - 1)
                    .map(|_index| file.read_i32::<LittleEndian>().unwrap())
                    .collect(); // Not sure what it is for
                let parent = file.read_u32::<LittleEndian>()?;
                let mut default_values = arr;
                default_values.resize(elements_count as usize, 0);
                let func = if instructions_offset > 0 {
                    Some(Function {
                        name: name_lowercase.clone(),
                        call_signature: CallSignature::new(0, flags.is_return()),
                        instructions: Vec::new(),
                        instructions_offset,
                        is_external: false,
                    })
                } else {
                    None
                };

                let parent_option = make_parent_option(parent);
                let my_elements_count = max(
                    elements_count,
                    self.find_parent_elements_count(parent_option),
                );

                self.symbols.push(Symbol {
                    name: name_lowercase,
                    index,
                    kind,
                    is_class_var: flags.is_class_var(),
                    is_const: false, // Even when flag says it is const, instances can be modified anyway
                    elements_count: my_elements_count,
                    offset: None,
                    default_values,
                    function: func,
                    parent: parent_option,
                });
            }
            SymbolKind::Prototype => {
                let instructions_offset = file.read_u32::<LittleEndian>()?;
                let parent = file.read_u32::<LittleEndian>()?;
                if parent == u32::MAX {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Prototype should have a parent",
                    ));
                }

                let parent_option = make_parent_option(parent);
                let my_elements_count = max(
                    elements_count,
                    self.find_parent_elements_count(parent_option),
                );

                let func = Function {
                    name: name_lowercase.clone(),
                    call_signature: CallSignature::new(elements_count, flags.is_return()),
                    instructions: Vec::new(),
                    instructions_offset,
                    is_external: false,
                };

                self.symbols.push(Symbol {
                    name: name_lowercase,
                    index,
                    kind,
                    is_class_var: flags.is_class_var(),
                    is_const: flags.is_const(),
                    elements_count: my_elements_count,
                    offset: None,
                    default_values: vec![0; my_elements_count as usize],
                    function: Some(func),
                    parent: make_parent_option(parent),
                });
            }
            SymbolKind::VariableArgument => {
                let _arr: Vec<String> = (0..elements_count)
                    .map(|_index| read_utf8_string(file).unwrap())
                    .collect(); // Not sure what it is for
                let parent = file.read_u32::<LittleEndian>()?;
                self.symbols.push(Symbol {
                    name: name_lowercase,
                    index,
                    kind,
                    is_class_var: flags.is_class_var(),
                    is_const: flags.is_const(),
                    elements_count,
                    offset: None,
                    default_values: vec![0; elements_count as usize],
                    function: None,
                    parent: make_parent_option(parent),
                });
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
/// Struct representing daedalus symbol.
pub struct Symbol {
    /// 32-bit words used to initialize symbol memory.
    pub(crate) default_values: Vec<i32>,
    /// Number of 32bit values which can be stored in this symbol.
    pub(crate) elements_count: u32,
    /// Function data if symbol is a function.
    pub(crate) function: Option<Function>,
    pub(crate) index: u32,
    pub(crate) kind: SymbolKind,
    pub(crate) is_class_var: bool,
    pub(crate) is_const: bool,
    /// Symbol name as specified in script binary.
    pub(crate) name: String,
    /// For offset in class variables.
    pub(crate) offset: Option<u32>,
    pub(crate) parent: Option<u32>,
}

impl Symbol {
    #[must_use]
    /// Create new daedalus symbol.
    pub fn new(index: u32, is_class_var: bool, elements_count: u32, offset: Option<u32>) -> Self {
        Symbol {
            name: index.to_string(),
            index,
            kind: SymbolKind::Int,
            is_class_var,
            is_const: false,
            elements_count,
            offset,
            default_values: vec![0; elements_count as usize],
            function: None,
            parent: None,
        }
    }

    /// Set values symbol contains during initialization.
    pub fn set_default_values(&mut self, default_values: Vec<i32>) {
        self.default_values = default_values;
    }

    /// Set symbol function.
    pub fn set_function(&mut self, function: Function) {
        self.function = Some(function);
    }

    /// Set symbol kind.
    pub fn set_kind(&mut self, kind: SymbolKind) {
        self.kind = kind;
    }

    /// Set symbol parent.
    pub fn set_parent(&mut self, parent: Option<u32>) {
        self.parent = parent;
    }

    #[must_use]
    pub(crate) fn size(&self) -> u32 {
        self.elements_count * 4
    }

    #[must_use]
    /// Name of daedalus symbol.
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    /// Check if symbol is a function and if this function is not external function.
    pub fn is_function_with_code(&self) -> bool {
        if let Some(func) = &self.function
            && !func.is_external
        {
            return true;
        }
        false
    }
}

#[must_use]
/// Number of functions which are imported from the engine.
/// They become imports of the created wasm module.
pub fn imported_functions_num(symbols: &[Symbol]) -> u32 {
    symbols
        .iter()
        .filter(|symbol| {
            symbol
                .function
                .as_ref()
                .is_some_and(|func| func.is_external)
        })
        .count()
        .to_u32()
}

#[derive(Debug, Clone)]
/// External function declared by the compiled script (engine-provided).
pub struct ExternalFunctionInfo {
    /// Lowercased external function name.
    pub name: String,
    /// Number of parameters the external expects.
    pub parameters_num: u32,
    /// Whether the external returns a value.
    pub returns: bool,
}

#[must_use]
/// Information about all functions which are imported from the engine.
/// They become imports of the created wasm module.
pub fn external_functions(symbols: &[Symbol]) -> Vec<ExternalFunctionInfo> {
    symbols
        .iter()
        .filter_map(|symbol| {
            let func = symbol.function.as_ref()?;
            if !func.is_external {
                return None;
            }
            Some(ExternalFunctionInfo {
                name: func.name.clone(),
                parameters_num: func.call_signature.parameters_num,
                returns: func.call_signature.returns,
            })
        })
        .collect()
}

#[derive(Debug, Clone)]
/// Daedalus function representation.
pub struct Function {
    pub(crate) call_signature: CallSignature,
    pub(crate) instructions: Vec<Instruction>,
    pub(crate) instructions_offset: u32,
    pub(crate) is_external: bool,
    pub(crate) name: String,
}

impl Function {
    #[must_use]
    /// Create new daedalus function.
    pub fn new(name: &str, call_signature: CallSignature, instructions: &[Instruction]) -> Self {
        Self {
            name: name.to_owned(),
            instructions_offset: 0,
            call_signature,
            instructions: instructions.to_vec(),
            is_external: false,
        }
    }

    #[must_use]
    /// If symbol is external from perspective of daedalus script.
    pub fn is_external(&self) -> bool {
        self.is_external
    }

    #[expect(unused, reason = "Used for generation wit interface file")]
    pub(crate) fn print_wit_declaration(&self) {
        let params = self.call_signature.parameters();
        let results = self.call_signature.results();
        print!("    {}: func(", self.name.replace('_', "-"));
        for index in 0..params.len() {
            print!("v-{}: s32", index);
            if index != params.len() - 1 {
                print!(", ");
            }
        }
        print!(")");
        if !results.is_empty() {
            print!(" -> s32");
        }
        println!(";");

        print!("RR    count_import!({}(", &self.name);
        for index in 0..params.len() {
            print!("_{}", index);
            if index != params.len() - 1 {
                print!(", ");
            }
        }
        print!(")");
        if !results.is_empty() {
            print!(" -> i32");
        }
        println!(");");
    }
}

#[derive(Debug, Clone, Copy)]
struct Flags(u32);

impl Flags {
    #[must_use]
    pub(crate) fn from(bits: u32) -> Self {
        let mask = ((1 << 6) - 1) << 16;
        let bits_moved = (bits & mask) >> 16;
        Self(bits_moved)
    }
    #[must_use]
    pub(crate) fn is_class_var(self) -> bool {
        self.0 & 4 > 0
    }
    #[must_use]
    pub(crate) fn is_const(self) -> bool {
        self.0 & 1 > 0
    }
    #[must_use]
    pub(crate) fn is_external(self) -> bool {
        self.0 & 8 > 0
    }
    #[must_use]
    pub(crate) fn is_return(self) -> bool {
        self.0 & 2 > 0
    }
    // pub(crate) fn is_merged(&self) -> bool {
    //     self.0 & 16 > 0
    // }
}

/// Kind of a daedalus symbol.
#[expect(
    clippy::exhaustive_enums,
    reason = "The variants come from the fixed .DAT file format, so they are complete"
)]
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum SymbolKind {
    /// Class.
    Class,
    /// Float.
    Float,
    /// Func.
    Func,
    /// Instance.
    Instance,
    /// Int.
    Int,
    /// Prototype.
    Prototype,
    /// String.
    String,
    /// Variable argument.
    VariableArgument,
}

impl SymbolKind {
    fn from(bits: u32) -> SymbolKind {
        let mask = ((1 << 4) - 1) << 12;
        let bits_moved = (bits & mask) >> 12;
        match bits_moved {
            1 => SymbolKind::Float,
            2 => SymbolKind::Int,
            3 => SymbolKind::String,
            4 => SymbolKind::Class,
            5 => SymbolKind::Func,
            6 => SymbolKind::Prototype,
            7 => SymbolKind::Instance,
            8 => SymbolKind::VariableArgument,
            _ => {
                panic!("unknown symbol");
            }
        }
    }

    // Values should be the same as in world.wit
    pub(crate) fn to_num(self) -> u8 {
        match self {
            SymbolKind::Float => 0,
            SymbolKind::Int => 1,
            SymbolKind::String => 2,
            SymbolKind::Class => 3,
            SymbolKind::Func => 4,
            SymbolKind::Prototype => 5,
            SymbolKind::Instance => 6,
            SymbolKind::VariableArgument => 7,
        }
    }
}

fn read_utf8_string(file: &mut BufReader<File>) -> io::Result<String> {
    let bytes = read_raw_string(file)?;
    Ok(decode_dat_string(&bytes))
}

/// Converts engine string encoding to utf-8.
fn decode_dat_string(bytes: &[u8]) -> String {
    let (decoded, _, had_errors) = encoding_rs::UTF_8.decode(bytes);
    if had_errors {
        let string = encoding_rs::WINDOWS_1250.decode(bytes).0.into_owned();
        return string;
    }
    decoded.into_owned()
}

fn read_raw_string(file: &mut BufReader<File>) -> io::Result<Vec<u8>> {
    let mut string = Vec::new();
    for _index in 0..1000 {
        let byte = file.read_u8()?;

        if byte == b'\n' {
            return Ok(string);
        }
        string.push(byte);
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "decoded unexpectedly long string",
    ))
}

fn make_parent_option(num: u32) -> Option<u32> {
    if num == u32::MAX {
        return None;
    }
    Some(num)
}

fn print_hex_bytes(title: &str, bytes: &[u8], offset: usize) {
    println!("{:x}    {:x?}({:?}) - {title})", offset, bytes, bytes);
}

fn read_u8(bytes: &[u8], index: &mut usize) -> u8 {
    let value = bytes[*index];
    *index += 1;
    value
}
fn read_u32(bytes: &[u8], index: &mut usize) -> u32 {
    let value = u32::from_le_bytes([
        bytes[*index + 1],
        bytes[*index + 2],
        bytes[*index + 3],
        bytes[*index + 4],
    ]);
    *index += 5;
    value
}
fn read_i32(bytes: &[u8], index: &mut usize) -> i32 {
    read_u32(bytes, index).cast_signed()
}

fn add_instruction(arr: &mut Vec<Instruction>, index: &mut usize, instr: Instruction) {
    *index += 1;
    arr.push(instr);
}

fn decode_bytecode(bytes: &[u8], offset: usize) -> Vec<Instruction> {
    let mut result = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            0 => add_instruction(&mut result, &mut i, Instruction::Add),
            1 => add_instruction(&mut result, &mut i, Instruction::Sub),
            2 => add_instruction(&mut result, &mut i, Instruction::Mul),
            3 => add_instruction(&mut result, &mut i, Instruction::Div),
            4 => add_instruction(&mut result, &mut i, Instruction::Mod),
            5 => add_instruction(&mut result, &mut i, Instruction::OrBitwise),
            6 => add_instruction(&mut result, &mut i, Instruction::AndBitwise),
            7 => add_instruction(&mut result, &mut i, Instruction::Lower),
            8 => add_instruction(&mut result, &mut i, Instruction::Higher),
            9 => add_instruction(&mut result, &mut i, Instruction::AssignInteger),
            11 => add_instruction(&mut result, &mut i, Instruction::LogicalOr),
            12 => add_instruction(&mut result, &mut i, Instruction::LogicalAnd),
            15 => add_instruction(&mut result, &mut i, Instruction::LowerEqual),
            16 => add_instruction(&mut result, &mut i, Instruction::Equal),
            17 => add_instruction(&mut result, &mut i, Instruction::NotEqual),
            18 => add_instruction(&mut result, &mut i, Instruction::HigherEqual),
            19 => add_instruction(&mut result, &mut i, Instruction::AddTo),
            20 => add_instruction(&mut result, &mut i, Instruction::SubTo),
            // 21 => add_instruction(&mut result, &mut i, Instruction::IsMul),
            // 22 => add_instruction(&mut result, &mut i, Instruction::IsDiv),
            30 => add_instruction(&mut result, &mut i, Instruction::UnaryPlus),
            31 => add_instruction(&mut result, &mut i, Instruction::UnaryMinus),
            32 => add_instruction(&mut result, &mut i, Instruction::UnaryNot),
            33 => add_instruction(&mut result, &mut i, Instruction::Complement),
            60 => add_instruction(&mut result, &mut i, Instruction::Return),
            61 => result.push(Instruction::Call(read_u32(bytes, &mut i))),
            62 => result.push(Instruction::CallExternal(read_u32(bytes, &mut i))),
            // 63 => result.push(Instruction::PopInt(read_i32(bytes, &mut i))),
            64 => result.push(Instruction::PushInt(read_i32(bytes, &mut i))),
            65 => result.push(Instruction::PushVar(read_u32(bytes, &mut i))),
            // 66 => result.push(Instruction::PushString(read_u32(bytes, &mut i))),
            67 => result.push(Instruction::PushInstance(read_u32(bytes, &mut i))),
            // 68 => result.push(Instruction::PushIndex(read_u32(bytes, &mut i))),
            // 69 => result.push(Instruction::PopVar(read_u32(bytes, &mut i))),
            70 => add_instruction(&mut result, &mut i, Instruction::AssignString),
            // 71 => add_instruction(&mut result, &mut i, Instruction::AssignStringP),
            72 => add_instruction(&mut result, &mut i, Instruction::AssignFunc),
            73 => add_instruction(&mut result, &mut i, Instruction::AssignFloat),
            74 => add_instruction(&mut result, &mut i, Instruction::AssignInstance),
            75 => result.push(Instruction::Jump(read_u32(bytes, &mut i))),
            76 => result.push(Instruction::JumpIf(read_u32(bytes, &mut i))),
            80 => result.push(Instruction::SetInstance(read_u32(bytes, &mut i))),
            245 => {
                let value = read_u32(bytes, &mut i);
                let index_in_arr = read_u8(bytes, &mut i);
                result.push(Instruction::PushArrayVar(value, index_in_arr));
            }

            _opcode => {
                print_hex_bytes("unknown ----- ", &bytes[i..=i], offset + i);
                panic!();
            }
        }
    }

    result
}

fn decode_data_len(bits: u32) -> u32 {
    let mask = (1 << 12) - 1;
    bits & mask
}
