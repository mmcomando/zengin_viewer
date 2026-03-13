use byteorder::{LittleEndian, ReadBytesExt};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};

#[allow(dead_code)]
#[derive(Debug)]
pub struct DatFile {
    pub stack_length: u32,
    pub header: DatHeader,
    pub symbols: Vec<Symbol>,
    pub functions: Vec<Function>,
    pub instances: Vec<Instance>,
    pub prototypes: Vec<Prototype>,
    pub strings: HashMap<u32, String>,
    pub class_offsets: HashMap<u32, u32>,
}

impl DatFile {
    pub fn get_function(&self, name: &str) -> Option<&Function> {
        self.functions.iter().find(|func| func.symbol.name == name)
    }
    pub fn get_function_by_offset(&self, offset: u32) -> Option<&Function> {
        self.functions
            .iter()
            .find(|func| func.symbol.instructions_offset == offset)
    }
    pub fn get_function_by_index(&self, index: u32) -> Option<&Function> {
        self.functions
            .iter()
            .find(|func| func.symbol_table_index == index)
    }

    pub fn get_prototype_by_index(&self, index: u32) -> Option<&Prototype> {
        self.prototypes
            .iter()
            .find(|el| el.symbol_table_index == index)
    }

    // pub fn get_instance(&self, name: &str) -> Option<&Instance> {
    //     self.instances.iter().find(|func| func.symbol.name == name)
    // }
    pub fn get_symbol_by_index(&self, index: u32) -> Option<&Symbol> {
        self.symbols.get(index as usize)
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct DatHeader {
    pub version: u8,
    pub symbol_count: u32,
}

#[derive(Debug, PartialEq)]
pub enum SymbolKind {
    Float,
    Int,
    String,
    Class,
    Func,
    Prototype,
    Instance,
    VariableArgument,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SymbolInt {
    pub name: String,
    pub data: u32,
}
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SymbolArrInt {
    pub name: String,
    pub arr: Vec<u32>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SymbolFloat {
    pub name: String,
    pub data: f32,
}
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SymbolArrFloat {
    pub name: String,
    pub arr: Vec<f32>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SymbolString {
    pub name: String,
    pub data: String,
}
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SymbolArrString {
    pub name: String,
    pub arr: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SymbolArrFunc {
    pub name: String,
    pub arr: Vec<u32>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SymbolClass {
    pub name: String,
    pub size: u32,
}
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SymbolClassVariable {
    pub name: String,
    pub class_index_id: u32,
    pub in_class_offset: u32,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SymbolFunc {
    pub name: String,
    pub instructions_offset: u32,
    pub external: bool,
}
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SymbolInstance {
    pub name: String,
    pub instructions_offset: u32,
    pub parent: Option<u32>,
}
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SymbolPrototype {
    pub name: String,
    pub instructions_offset: u32,
    pub parent: u32,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SymbolVariableArgument {
    pub name: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Symbol {
    SymbolInt(SymbolInt),
    SymbolArrInt(SymbolArrInt),
    SymbolFloat(SymbolFloat),
    SymbolArrFloat(SymbolArrFloat),
    SymbolString(SymbolString),
    SymbolArrString(SymbolArrString),
    SymbolArrFunc(SymbolArrFunc),
    SymbolClass(SymbolClass),
    SymbolClassVariable(SymbolClassVariable),
    SymbolFunc(SymbolFunc),
    SymbolInstance(SymbolInstance),
    SymbolPrototype(SymbolPrototype),
    SymbolVariableArgument(SymbolVariableArgument),
}

impl Symbol {
    #[allow(dead_code)]
    pub fn name(&self) -> &str {
        match self {
            Self::SymbolInt(el) => &el.name,
            Self::SymbolArrInt(el) => &el.name,
            Self::SymbolFloat(el) => &el.name,
            Self::SymbolArrFloat(el) => &el.name,
            Self::SymbolString(el) => &el.name,
            Self::SymbolArrString(el) => &el.name,
            Self::SymbolArrFunc(el) => &el.name,
            Self::SymbolClass(el) => &el.name,
            Self::SymbolClassVariable(el) => &el.name,
            Self::SymbolFunc(el) => &el.name,
            Self::SymbolInstance(el) => &el.name,
            Self::SymbolPrototype(el) => &el.name,
            Self::SymbolVariableArgument(el) => &el.name,
        }
    }

    #[allow(dead_code)]
    pub fn parent(&self) -> Option<u32> {
        match self {
            Self::SymbolInstance(el) => el.parent,
            Self::SymbolPrototype(el) => Some(el.parent),
            Self::SymbolInt(_)
            | Self::SymbolArrInt(_)
            | Self::SymbolFloat(_)
            | Self::SymbolArrFloat(_)
            | Self::SymbolString(_)
            | Self::SymbolArrString(_)
            | Self::SymbolFunc(_)
            | Self::SymbolArrFunc(_)
            | Self::SymbolClass(_)
            | Self::SymbolClassVariable(_)
            | Self::SymbolVariableArgument(_) => None,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct Prototype {
    pub symbol: SymbolPrototype,
    pub symbol_table_index: u32,
    pub instructions: Vec<Instruction>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct Instance {
    pub symbol: SymbolInstance,
    pub symbol_table_index: u32,
    pub parent: Option<u32>,
    pub instructions: Vec<Instruction>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct Function {
    pub symbol: SymbolFunc,
    pub symbol_table_index: u32,
    pub instructions: Vec<Instruction>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum Instruction {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Or,
    And,
    Assign,
    LowerEq,
    Eq,
    Neq,
    Lower,
    Higher,
    HigherEq,
    IsPlus,
    IsMinus,
    // IsMul,
    // IsDiv,
    UnPlus,
    UnMinus,
    UnNot,
    // UnNeg,
    LogOr,
    LogAnd,

    Return,
    Call(u32),
    CallExternal(u32),
    PushInt(i32),
    PushVar(u32),
    PushInstance(u32),
    AssignString,
    // AssignStringP,
    AssignFunc,
    AssignFloat,
    AssignInstance,

    Jump(u32),
    JumpF(u32),
    SetInstance(u32),
    PushArrayVar(u32, u8),
}

pub fn read_string(file: &mut File) -> io::Result<String> {
    let mut string = String::new();
    for index in 0..1000 {
        let byte = file.read_u8()?;
        if index == 0 && (byte == 1 || byte == 255) {
            // println!("strange byte({byte}) sometimes at the start of string?");
            continue;
        }
        if byte == b'\n' {
            return Ok(string);
        }
        string.push(
            char::from_u32(u32::from(byte))
                .unwrap()
                .to_ascii_lowercase(),
        );
    }
    panic!("decoded unexpectdly long string");
}

fn make_parent_option(num: u32) -> Option<u32> {
    if num == u32::MAX {
        return None;
    }
    Some(num)
}

pub fn parse_symbol(file: &mut File) -> io::Result<Symbol> {
    let _b_has_name = file.read_u32::<LittleEndian>()?;

    let name = read_string(file)?;
    let offset = file.read_u32::<LittleEndian>()?;
    let bitfield = file.read_u32::<LittleEndian>()?;
    let _filenr = file.read_u32::<LittleEndian>()?;
    let _line = file.read_u32::<LittleEndian>()?;
    let _line_anz = file.read_u32::<LittleEndian>()?;
    let _pos_beg = file.read_u32::<LittleEndian>()?;
    let _pos_anz = file.read_u32::<LittleEndian>()?;

    let flags = decode_flags(bitfield);
    let kind = decode_symbol_kind(bitfield);

    if flags.is_classvar() {
        let parent = file.read_u32::<LittleEndian>()?;
        assert!(
            parent != u32::MAX,
            "Class variable has to have parent defined"
        );
        let symbol = SymbolClassVariable {
            name,
            class_index_id: parent,
            in_class_offset: offset,
        };
        return Ok(Symbol::SymbolClassVariable(symbol));
    }

    let elements_count = decode_data_len(bitfield);

    match kind {
        SymbolKind::Class => {
            let _content = file.read_u32::<LittleEndian>()?; // Not sure what it is for
            let parent = file.read_u32::<LittleEndian>()?;
            assert!(
                parent == u32::MAX,
                "Class definition should not have a parent"
            );
            return Ok(Symbol::SymbolClass(SymbolClass {
                name,
                // Offset in case of class probably means a total class size
                size: offset,
            }));
        }
        SymbolKind::Float => {
            let arr: Vec<f32> = (0..elements_count)
                .map(|_index| file.read_f32::<LittleEndian>().unwrap())
                .collect();
            let parent = file.read_u32::<LittleEndian>()?;
            assert!(parent == u32::MAX, "Float should not have a parent");
            if arr.len() > 1 || arr.is_empty() {
                return Ok(Symbol::SymbolArrFloat(SymbolArrFloat { name, arr }));
            }
            return Ok(Symbol::SymbolFloat(SymbolFloat { name, data: arr[0] }));
        }
        SymbolKind::Int => {
            let arr: Vec<u32> = (0..elements_count)
                .map(|_index| file.read_u32::<LittleEndian>().unwrap())
                .collect();

            let _parent = file.read_u32::<LittleEndian>()?;
            // function parameters symbols have parent set, other variables don't
            // assert!(parent == u32::MAX, "Int should not have a parent");
            if arr.len() > 1 || arr.is_empty() {
                return Ok(Symbol::SymbolArrInt(SymbolArrInt { name, arr }));
            }
            return Ok(Symbol::SymbolInt(SymbolInt { name, data: arr[0] }));
        }
        SymbolKind::String => {
            let arr: Vec<String> = (0..elements_count)
                .map(|_index| read_string(file).unwrap())
                .collect();
            let _parent = file.read_u32::<LittleEndian>()?;
            // function parameters symbols have parent set, other variables don't
            // assert!(parent == u32::MAX, "String should not have a parent");
            if arr.len() > 1 || arr.is_empty() {
                return Ok(Symbol::SymbolArrString(SymbolArrString { name, arr }));
            }
            return Ok(Symbol::SymbolString(SymbolString {
                name,
                data: arr[0].clone(),
            }));
        }
        SymbolKind::Func => {
            let instructions_offset = file.read_u32::<LittleEndian>()?;
            let parent = file.read_u32::<LittleEndian>()?;
            assert!(parent == u32::MAX, "Function should not have a parent");
            return Ok(Symbol::SymbolFunc(SymbolFunc {
                name,
                instructions_offset,
                external: flags.is_external(),
            }));
        }
        SymbolKind::Instance => {
            let instructions_offset = file.read_u32::<LittleEndian>()?;
            let elements_count = elements_count.max(1);
            let _arr: Vec<u32> = (0..elements_count - 1)
                .map(|_index| file.read_u32::<LittleEndian>().unwrap())
                .collect(); // Not sure what it is for
            let parent = file.read_u32::<LittleEndian>()?;
            return Ok(Symbol::SymbolInstance(SymbolInstance {
                name,
                instructions_offset,
                parent: make_parent_option(parent),
            }));
        }
        SymbolKind::Prototype => {
            let instructions_offset = file.read_u32::<LittleEndian>()?; // Not sure what it is for
            let parent = file.read_u32::<LittleEndian>()?;
            assert!(parent != u32::MAX, "Prototype should have a parent");
            return Ok(Symbol::SymbolPrototype(SymbolPrototype {
                name,
                instructions_offset,
                parent,
            }));
        }
        SymbolKind::VariableArgument => {
            let _arr: Vec<String> = (0..elements_count)
                .map(|_index| read_string(file).unwrap())
                .collect(); // Not sure what it is for
            let _parent = file.read_u32::<LittleEndian>()?;
            return Ok(Symbol::SymbolVariableArgument(SymbolVariableArgument {
                name,
            }));
        }
    }
}

pub fn parse_dat(path: &str) -> io::Result<DatFile> {
    let mut file = File::open(path)?;

    let version = file.read_u8()?;
    let symbol_count = file.read_u32::<LittleEndian>()?;

    let header = DatHeader {
        version,
        symbol_count,
    };

    let mut symbols = Vec::new();

    for _ in 0..symbol_count {
        // It is used for something?
        let _num = file.read_u32::<LittleEndian>()?;
    }

    for _ in 0..symbol_count {
        let symbol = parse_symbol(&mut file).unwrap();
        symbols.push(symbol);
    }

    let stack_length = file.read_u32::<LittleEndian>()?;

    let mut remaining = Vec::new();
    file.read_to_end(&mut remaining)?;

    let mut functions = Vec::new();
    let mut instances = Vec::new();
    let mut prototypes = Vec::new();
    let mut strings = HashMap::new();
    let mut class_offsets = HashMap::new();
    for (index, symbol) in symbols.iter().enumerate() {
        if let Symbol::SymbolFunc(func) = symbol {
            if func.external {
                continue;
            }
            let instructions = decode_bytecode(&remaining, func.instructions_offset as usize);
            let function = Function {
                symbol: func.clone(),
                symbol_table_index: index as u32,
                instructions,
            };
            functions.push(function);
            strings.insert(index as u32, func.name.clone());
        }
        if let Symbol::SymbolInstance(instance) = symbol {
            let instructions = decode_bytecode(&remaining, instance.instructions_offset as usize);
            let instance = Instance {
                symbol: instance.clone(),
                symbol_table_index: index as u32,
                parent: instance.parent,
                instructions,
            };
            strings.insert(index as u32, instance.symbol.name.clone());
            instances.push(instance);
        }
        if let Symbol::SymbolPrototype(prototype) = symbol {
            let instructions = decode_bytecode(&remaining, prototype.instructions_offset as usize);
            let prototype = Prototype {
                symbol: prototype.clone(),
                symbol_table_index: index as u32,
                instructions,
            };
            strings.insert(index as u32, prototype.symbol.name.clone());
            prototypes.push(prototype);
        }

        if let Symbol::SymbolString(string) = symbol {
            strings.insert(index as u32, string.data.clone());
        }
        if let Symbol::SymbolClassVariable(var) = symbol {
            class_offsets.insert(index as u32, var.in_class_offset);
        }
    }

    Ok(DatFile {
        stack_length,
        header,
        symbols,
        functions,
        instances,
        prototypes,
        strings,
        class_offsets,
    })
}

fn print_hex_bytes(title: &str, bytes: &[u8], offset: usize) {
    println!("{:x}    {:x?}({:?}) - {title})", offset, bytes, bytes);
}

fn read_u8(bytes: &[u8], index: &mut usize) -> u8 {
    let value = bytes[*index];
    *index += 1;
    return value;
}
fn read_u32(bytes: &[u8], index: &mut usize) -> u32 {
    let value = u32::from_le_bytes([
        bytes[*index + 1],
        bytes[*index + 2],
        bytes[*index + 3],
        bytes[*index + 4],
    ]);
    *index += 5;
    return value;
}
fn read_i32(bytes: &[u8], index: &mut usize) -> i32 {
    read_u32(bytes, index) as i32
}

fn add_instruction(arr: &mut Vec<Instruction>, index: &mut usize, instr: Instruction) {
    *index += 1;
    arr.push(instr);
}

fn decode_bytecode(all_bytecode: &[u8], offset: usize) -> Vec<Instruction> {
    let bytes = &all_bytecode[offset..];

    let mut result = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            0 => add_instruction(&mut result, &mut i, Instruction::Add),
            1 => add_instruction(&mut result, &mut i, Instruction::Sub),
            2 => add_instruction(&mut result, &mut i, Instruction::Mul),
            3 => add_instruction(&mut result, &mut i, Instruction::Div),
            4 => add_instruction(&mut result, &mut i, Instruction::Mod),
            5 => add_instruction(&mut result, &mut i, Instruction::Or),
            6 => add_instruction(&mut result, &mut i, Instruction::And),
            7 => add_instruction(&mut result, &mut i, Instruction::Lower),
            8 => add_instruction(&mut result, &mut i, Instruction::Higher),
            9 => add_instruction(&mut result, &mut i, Instruction::Assign),
            11 => add_instruction(&mut result, &mut i, Instruction::LogOr),
            12 => add_instruction(&mut result, &mut i, Instruction::LogAnd),
            15 => add_instruction(&mut result, &mut i, Instruction::LowerEq),
            16 => add_instruction(&mut result, &mut i, Instruction::Eq),
            17 => add_instruction(&mut result, &mut i, Instruction::Neq),
            18 => add_instruction(&mut result, &mut i, Instruction::HigherEq),
            19 => add_instruction(&mut result, &mut i, Instruction::IsPlus),
            20 => add_instruction(&mut result, &mut i, Instruction::IsMinus),
            // 21 => add_instruction(&mut result, &mut i, Instruction::IsMul),
            // 22 => add_instruction(&mut result, &mut i, Instruction::IsDiv),
            30 => add_instruction(&mut result, &mut i, Instruction::UnPlus),
            31 => add_instruction(&mut result, &mut i, Instruction::UnMinus),
            32 => add_instruction(&mut result, &mut i, Instruction::UnNot),
            // 33 => add_instruction(&mut result, &mut i, Instruction::UnNeg),
            60 => {
                result.push(Instruction::Return);
                break;
            }
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
            76 => result.push(Instruction::JumpF(read_u32(bytes, &mut i))),
            80 => result.push(Instruction::SetInstance(read_u32(bytes, &mut i))),
            245 => {
                let value = read_u32(bytes, &mut i);
                let index_in_arr = read_u8(bytes, &mut i);
                result.push(Instruction::PushArrayVar(value, index_in_arr));
            }

            _opcode => {
                // result.push(Instruction::Unknown(opcode));
                // println!("{:#?}", result);
                // print_hex_bytes("a", &bytes[(i - 5)..(i - 4)], offset + i - 5);
                // print_hex_bytes("a", &bytes[(i - 4)..(i - 3)], offset + i - 4);
                // print_hex_bytes("a", &bytes[(i - 3)..(i - 2)], offset + i - 3);
                // print_hex_bytes("a", &bytes[(i - 2)..(i - 1)], offset + i - 2);
                // print_hex_bytes("a", &bytes[(i - 1)..(i + 0)], offset + i - 1);
                print_hex_bytes("unknown ----- ", &bytes[i..=i], offset + i);
                panic!();
            }
        }
    }

    result
}

#[derive(Debug)]
pub struct Flags(u32);

impl Flags {
    // pub fn is_const(&self) -> bool {
    //     self.0 & 1 > 0
    // }
    // pub fn is_return(&self) -> bool {
    //     self.0 & 2 > 0
    // }
    pub fn is_classvar(&self) -> bool {
        self.0 & 4 > 0
    }
    pub fn is_external(&self) -> bool {
        self.0 & 8 > 0
    }
    // pub fn is_merged(&self) -> bool {
    //     self.0 & 16 > 0
    // }
}
fn decode_flags(bits: u32) -> Flags {
    let mask = ((1 << 6) - 1) << 16;
    let bits_moved = (bits & mask) >> 16;
    return Flags(bits_moved);
}

fn decode_symbol_kind(bits: u32) -> SymbolKind {
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

fn decode_data_len(bits: u32) -> u32 {
    let mask = (1 << 12) - 1;
    let data_len = bits & mask;
    return data_len;
}
