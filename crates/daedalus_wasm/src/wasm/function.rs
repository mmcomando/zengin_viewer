use crate::common::ToU32 as _;
use crate::common::ToUsize as _;
use crate::daedalus::analyze::AnalyzedInstruction;
use crate::daedalus::analyze::Block;
use crate::daedalus::analyze::ControlBlock;
use crate::daedalus::analyze::InstructionBlock;
use crate::daedalus::analyze::ProvideValType;
use crate::wasm::CURRENT_INSTANCE_SYMBOL_INDEX;
use std::collections::HashMap;
use wasm_encoder::{BlockType, Function, Instruction, MemArg, ValType};

use crate::daedalus::Function as FuncDD;
use crate::daedalus::Instruction as InstDD;
use crate::wasm::BinaryBuilder;
use crate::wasm::CURRENT_CLASS_INSTANCE_MEMORY_GLOBAL_INDEX;
use crate::wasm::DaedalusMemoryLayout;

#[derive(Debug, Clone)]
pub(crate) struct ConvertedFunction {
    pub(crate) func: Function,
    pub(crate) name: String,
    pub(crate) params: Vec<ValType>,
    pub(crate) results: Vec<ValType>,
}

#[derive(Debug)]
struct ConversionState<'a> {
    dd_func: &'a FuncDD,
    func_index_set_instance: u32,
    instruction_offsets: Vec<u32>,
    module_builder: &'a BinaryBuilder,
    tmp_val_0: u32,
    tmp_val_1: u32,
}

impl<'a> ConversionState<'a> {
    fn compute_instruction_offsets(&mut self, dd_func: &FuncDD) {
        let mut next_offset = dd_func.instructions_offset;
        let mut offsets = Vec::with_capacity(dd_func.instructions.len());
        for instruction in &dd_func.instructions {
            offsets.push(next_offset);
            next_offset += instruction.size().to_u32();
        }

        self.instruction_offsets = offsets;
    }

    fn lower_block(&mut self, func: &mut Function, main_block: &Block) {
        for block in &main_block.blocks {
            match block {
                ControlBlock::Instructions(instructions) => {
                    for instruction in &instructions.instructions {
                        self.map_instruction(func, instruction.clone());
                    }
                }
                ControlBlock::If(if_block) => {
                    func.instruction(&Instruction::If(BlockType::Empty));
                    self.lower_block(func, if_block);
                    func.instruction(&Instruction::End);
                }
                ControlBlock::IfElse {
                    if_block,
                    else_block,
                } => {
                    let block_effect = block.stack_effect();
                    let block_type = if block_effect < 0 {
                        BlockType::Empty
                    } else {
                        BlockType::Result(ValType::I32)
                    };

                    func.instruction(&Instruction::If(block_type));
                    self.lower_block(func, if_block);
                    func.instruction(&Instruction::Else);
                    self.lower_block(func, else_block);
                    func.instruction(&Instruction::End);
                }
            }
        }
    }

    fn map_compound_assign(&self, func: &mut Function, is_sub: bool) {
        func.instruction(&Instruction::LocalSet(self.tmp_val_0));

        if is_sub {
            func.instruction(&Instruction::I32Const(-1));
            func.instruction(&Instruction::I32Mul);
        }

        func.instruction(&Instruction::LocalGet(self.tmp_val_0));
        func.instruction(&Instruction::I32Load(MemArg {
            offset: 0,
            align: 2,
            memory_index: DaedalusMemoryLayout::SYMBOLS_MEMORY_INDEX,
        }));

        func.instruction(&Instruction::I32Add);
        func.instruction(&Instruction::LocalGet(self.tmp_val_0));
        self.swap_values_on_stack(func);
        func.instruction(&Instruction::I32Store(MemArg {
            offset: 0,
            align: 2,
            memory_index: DaedalusMemoryLayout::SYMBOLS_MEMORY_INDEX,
        }));
    }

    #[expect(clippy::too_many_lines, reason = "Handling match")]
    fn map_instruction(&self, func: &mut Function, analyzed_instruction: AnalyzedInstruction) {
        let dd_instr = analyzed_instruction.instr;
        // assert!(analyzed_instruction.value_is_used);

        // Daedalus has concept of references which can be pushed to stack, such a reference
        //  can be interpreted as a destination address for assign instructions or as value for common operations
        // From analysis we know what push instructions should produce
        let provide_val_type = analyzed_instruction.provide_val_type == ProvideValType::Ref;

        match dd_instr {
            InstDD::Add => {
                func.instruction(&Instruction::I32Add);
            }
            InstDD::Sub => {
                // Daedalus pushes the right operand first, the op pops the left
                // operand from the top (`a = pop(); b = pop(); push(a - b)`),
                // wasm ops pop the right operand from the top, so the operands
                // have to be swapped before the wasm op.
                self.swap_values_on_stack(func);
                func.instruction(&Instruction::I32Sub);
            }
            InstDD::Mul => {
                func.instruction(&Instruction::I32Mul);
            }
            InstDD::Div => {
                self.swap_values_on_stack(func);
                func.instruction(&Instruction::I32DivS);
            }
            InstDD::Mod => {
                self.swap_values_on_stack(func);
                func.instruction(&Instruction::I32RemS);
            }
            InstDD::OrBitwise | InstDD::LogicalOr => {
                func.instruction(&Instruction::I32Or);
            }
            InstDD::AndBitwise | InstDD::LogicalAnd => {
                func.instruction(&Instruction::I32And);
            }
            // Stack:
            // - Value A - Save this value
            // - Memory address B - Save here
            InstDD::AssignString
            | InstDD::AssignFunc
            | InstDD::AssignFloat
            | InstDD::AssignInstance
            | InstDD::AssignInteger => {
                // Daedalus stack order: memory reference then value (in order of popping values)
                // wasm stack order: value then address (in order of popping values)

                self.swap_values_on_stack(func);
                func.instruction(&Instruction::I32Store(MemArg {
                    offset: 0,
                    align: 2,
                    memory_index: DaedalusMemoryLayout::SYMBOLS_MEMORY_INDEX,
                }));
            }
            // Comparisons: daedalus stack is [right operand, left operand],
            // the op means `left OP right` (`a = pop(); b = pop(); push(a > b)`
            // where `a` is the left operand). Wasm compares `a OP b` with the
            // top value as `b`, so the inverted comparison is used.
            InstDD::LowerEqual => {
                func.instruction(&Instruction::I32GeS);
            }
            InstDD::Equal => {
                func.instruction(&Instruction::I32Eq);
            }
            InstDD::NotEqual => {
                func.instruction(&Instruction::I32Ne);
            }
            InstDD::Lower => {
                func.instruction(&Instruction::I32GtS);
            }
            InstDD::Higher => {
                func.instruction(&Instruction::I32LtS);
            }
            InstDD::HigherEqual => {
                func.instruction(&Instruction::I32LeS);
            }
            InstDD::AddTo => {
                self.map_compound_assign(func, false);
            }
            InstDD::SubTo => {
                self.map_compound_assign(func, true);
            }
            InstDD::UnaryPlus => {}
            InstDD::UnaryMinus => {
                func.instruction(&Instruction::I32Const(-1));
                func.instruction(&Instruction::I32Mul);
            }
            InstDD::UnaryNot => {
                func.instruction(&Instruction::I32Eqz);
            }
            InstDD::Complement => {
                func.instruction(&Instruction::I32Const(-1));
                func.instruction(&Instruction::I32Xor);
            }
            InstDD::Return => {
                if analyzed_instruction.implicit_return {
                    func.instruction(&Instruction::I32Const(0));
                }
                func.instruction(&Instruction::Return);
            }
            InstDD::Call(address) => {
                let Some(func_info) = self.module_builder.instr_offset_to_func_info.get(&address)
                else {
                    let missing_func_name = self
                        .module_builder
                        .symbols
                        .iter()
                        .filter_map(|symbol| symbol.function.as_ref())
                        .find(|function| function.instructions_offset == address)
                        .map_or("<unknown>", |function| function.name.as_str());
                    panic!(
                        "Could not find function with instruction address({address} -> {missing_func_name}), while compiling({})",
                        &self.dd_func.name
                    );
                };

                // Save current instance
                func.instruction(&Instruction::GlobalGet(CURRENT_INSTANCE_SYMBOL_INDEX));
                func.instruction(&Instruction::LocalSet(self.tmp_val_0));
                func.instruction(&Instruction::GlobalGet(
                    CURRENT_CLASS_INSTANCE_MEMORY_GLOBAL_INDEX,
                ));
                func.instruction(&Instruction::LocalSet(self.tmp_val_1));

                // Call
                func.instruction(&Instruction::Call(func_info.wasm_function_index));
                // Restore current Instance
                func.instruction(&Instruction::LocalGet(self.tmp_val_0));
                func.instruction(&Instruction::GlobalSet(CURRENT_INSTANCE_SYMBOL_INDEX));
                func.instruction(&Instruction::LocalGet(self.tmp_val_1));
                func.instruction(&Instruction::GlobalSet(
                    CURRENT_CLASS_INSTANCE_MEMORY_GLOBAL_INDEX,
                ));
            }
            InstDD::CallExternal(func_symbol_index) => {
                let Some(symbol_info) = self.module_builder.symbol_to_info.get(&func_symbol_index)
                else {
                    let missing_func_name = self
                        .module_builder
                        .symbols
                        .get(func_symbol_index.to_usize())
                        .map_or("<unknown>", |sym| &sym.name);
                    panic!(
                        "Could not find external function under symbol({func_symbol_index} -> {missing_func_name}), while compiling({})",
                        &self.dd_func.name
                    );
                };
                let func_info = symbol_info.func_info.as_ref().unwrap();
                func.instruction(&Instruction::Call(func_info.wasm_function_index));
            }
            InstDD::PushInt(num) => {
                func.instruction(&Instruction::I32Const(num));
            }
            InstDD::PushVar(index) => {
                self.push_symbol_value_and_address(func, index, 0, provide_val_type);
            }
            InstDD::PushInstance(index) => {
                // In DecDat daedalus decoder push instance instruction seems to return integer in symbol content field.
                // No idea what this integer means. Currently we assume that instance symbol_index is an ID to instance memory and code.
                // func.instruction(&Instruction::I32Const(index.cast_signed()));
                self.push_symbol_value_and_address(func, index, 0, provide_val_type);
            }
            InstDD::PushArrayVar(index, element_index) => {
                self.push_symbol_value_and_address(func, index, element_index, provide_val_type);
            }
            InstDD::SetInstance(index) => {
                // The daedalus compiler emits `SetInstance` before every member
                // access through an instance variable, meaning "switch the
                // member access context to the instance this symbol points to".
                // For declared instances the symbol identifies the instance
                // itself, for instance variables (`self`, `other`, `var C_Npc`
                // locals) their value holds the reference to the instance.
                let info = self.module_builder.symbol_to_info.get(&index).unwrap();
                if info.func_info.is_some() {
                    func.instruction(&Instruction::I32Const(index.cast_signed()));
                } else {
                    let Some(address) = info.memory_address else {
                        panic!(
                            "SetInstance symbol({index}) has no memory address, it cannot hold an instance reference"
                        );
                    };
                    func.instruction(&Instruction::I32Const(address.cast_signed()));
                    func.instruction(&Instruction::I32Load(MemArg {
                        offset: 0,
                        align: 2,
                        memory_index: DaedalusMemoryLayout::SYMBOLS_MEMORY_INDEX,
                    }));
                }
                func.instruction(&Instruction::Call(self.func_index_set_instance));
            }
            InstDD::Jump(_) | InstDD::JumpIf(_) => {
                panic!("Jump instructions should have been lowered in higher level steps");
            }
        }
        if !analyzed_instruction.value_is_used {
            func.instruction(&Instruction::Drop);
        }
    }

    fn new(module_builder: &'a BinaryBuilder, dd_func: &'a FuncDD) -> Self {
        Self {
            module_builder,
            dd_func,
            instruction_offsets: vec![],
            tmp_val_0: 0,
            tmp_val_1: 0,
            func_index_set_instance: 0,
        }
    }

    fn parse_block_range(
        &mut self,
        offset_to_index: &HashMap<u32, usize>,
        start: usize,
        end: usize,
    ) -> Block {
        let mut result = Block::new();
        let mut current_block = Vec::new();
        let mut index = start;

        while index < end {
            #[expect(
                clippy::wildcard_enum_match_arm,
                reason = "We want to explicitly mention only if instructions"
            )]
            match self.dd_func.instructions[index] {
                InstDD::JumpIf(target_offset) => {
                    if !current_block.is_empty() {
                        result
                            .blocks
                            .push(ControlBlock::Instructions(InstructionBlock::new(
                                current_block,
                            )));
                        current_block = Vec::new();
                    }

                    let (control_block, next_index) =
                        self.parse_if_block(offset_to_index, index, target_offset, end);
                    result.blocks.push(control_block);
                    index = next_index;
                }
                InstDD::Jump(target_offset) => {
                    panic!("Unexpected Jump to {target_offset} outside of if/else lowering");
                }
                intr => {
                    current_block.push(AnalyzedInstruction::new(intr, ProvideValType::Val));
                    index += 1;
                }
            }
        }

        if !current_block.is_empty() {
            result
                .blocks
                .push(ControlBlock::Instructions(InstructionBlock::new(
                    current_block,
                )));
        }

        result
    }

    fn parse_if_block(
        &mut self,
        offset_to_index: &HashMap<u32, usize>,
        jump_if_index: usize,
        false_target_offset: u32,
        end: usize,
    ) -> (ControlBlock, usize) {
        let false_target_index = *offset_to_index
            .get(&false_target_offset)
            .unwrap_or_else(|| panic!("Unknown JumpIf target offset {false_target_offset:x}"));

        assert!(
            false_target_index <= end,
            "JumpIf target offset {false_target_offset} leaves the current block range"
        );

        let mut scan_index = jump_if_index + 1;
        let mut else_jump = None;

        while scan_index < false_target_index {
            #[expect(
                clippy::wildcard_enum_match_arm,
                reason = "We want to explicitly mention only if instructions"
            )]
            match self.dd_func.instructions[scan_index] {
                InstDD::JumpIf(nested_target_offset) => {
                    let (_, next_index) = self.parse_if_block(
                        offset_to_index,
                        scan_index,
                        nested_target_offset,
                        false_target_index,
                    );
                    scan_index = next_index;
                }
                InstDD::Jump(after_else_offset) => {
                    if after_else_offset >= false_target_offset {
                        let after_else_index =
                            *offset_to_index.get(&after_else_offset).unwrap_or_else(|| {
                                panic!("Unknown Jump target offset {after_else_offset}")
                            });
                        else_jump = Some((scan_index, after_else_index));
                        break;
                    }
                    panic!("Unsupported Jump to {after_else_offset} inside if-range");
                }
                _ => {
                    scan_index += 1;
                }
            }
        }

        if let Some((else_jump_index, after_else_index)) = else_jump {
            let if_block =
                self.parse_block_range(offset_to_index, jump_if_index + 1, else_jump_index);
            let else_block =
                self.parse_block_range(offset_to_index, false_target_index, after_else_index);

            (
                ControlBlock::IfElse {
                    if_block,
                    else_block,
                },
                after_else_index,
            )
        } else {
            let if_block =
                self.parse_block_range(offset_to_index, jump_if_index + 1, false_target_index);
            (ControlBlock::If(if_block), false_target_index)
        }
    }

    fn push_symbol_value_and_address(
        &self,
        func: &mut Function,
        symbol_index: u32,
        element_index: u8,
        provide_val_type: bool,
    ) {
        let array_delta = i32::from(element_index) * 4;

        let info = self
            .module_builder
            .symbol_to_info
            .get(&symbol_index)
            .unwrap();

        // If we push symbol which is an instance don't load value but just store symbol index
        // Assume that if we are doing push to symbol which has code in it, it is a instance
        let is_instance = info.func_info.is_some();
        if is_instance {
            assert!(
                element_index == 0,
                "Instance symbol should be pushed with element index 0, got {element_index}"
            );
            func.instruction(&Instruction::I32Const(symbol_index.cast_signed()));
            return;
        }

        if let Some(default_values) = &info.const_values {
            assert!(!provide_val_type, "Const var should not be used as ref");
            // We know final value during compilation
            let value = *default_values.get(usize::from(element_index)).unwrap();
            func.instruction(&Instruction::I32Const(value));
        } else if let Some(address) = info.memory_address {
            // We know the address during compilation but value might change
            func.instruction(&Instruction::I32Const(address.cast_signed() + array_delta));
            if !provide_val_type {
                func.instruction(&Instruction::I32Load(MemArg {
                    offset: 0,
                    align: 2,
                    memory_index: DaedalusMemoryLayout::SYMBOLS_MEMORY_INDEX,
                }));
            }
        } else if let Some(offset) = info.member_offset {
            func.instruction(&Instruction::GlobalGet(
                CURRENT_CLASS_INSTANCE_MEMORY_GLOBAL_INDEX,
            ));
            // We know the member offset during compilation
            func.instruction(&Instruction::I32Const(offset.cast_signed() + array_delta));
            func.instruction(&Instruction::I32Add);

            if !provide_val_type {
                func.instruction(&Instruction::I32Load(MemArg {
                    offset: 0,
                    align: 2,
                    memory_index: DaedalusMemoryLayout::SYMBOLS_MEMORY_INDEX,
                }));
            }
        } else {
            // This case should not happen
            // At this point we should know final value, global address or member offset
            panic!(
                "Can convert PushVar instruction, not enough information for given symbol index({symbol_index})"
            );
        }
    }

    /// We split instructions into blocks so we can reconstruct original scripts control flow.
    /// This is simple algorithm which works only in specific cases.
    ///
    /// We assume:
    /// - There are no loops
    /// - Assembly was not optimized (so trivial control flow is changed to non trivial one)
    /// - if statements don't jump outside of its parent if statement
    /// - if-else statements look like this:
    ///   [`if`, `block_true_instructions`, `jump_after_block_false_instructions`, `else`, `block_false_instructions`]
    fn split_blocks(&mut self) -> Block {
        let offset_to_index: HashMap<u32, usize> = self
            .instruction_offsets
            .iter()
            .copied()
            .enumerate()
            .map(|(index, offset)| (offset, index))
            .collect();

        let mut block =
            self.parse_block_range(&offset_to_index, 0, self.dd_func.instructions.len());
        block.function_parameters_num = self.dd_func.call_signature.parameters_num;
        block.set_function_returns(self.dd_func.call_signature.returns);
        block
    }

    fn swap_values_on_stack(&self, func: &mut Function) {
        func.instruction(&Instruction::LocalSet(self.tmp_val_0));
        func.instruction(&Instruction::LocalSet(self.tmp_val_1));
        func.instruction(&Instruction::LocalGet(self.tmp_val_0));
        func.instruction(&Instruction::LocalGet(self.tmp_val_1));
    }
}

#[must_use]
pub(super) fn convert_daedalus_func(
    module_builder: &BinaryBuilder,
    dd_func: &FuncDD,
    func_index_set_instance: u32,
) -> ConvertedFunction {
    let mut converter = ConversionState::new(module_builder, dd_func);

    // Converted functions use two variables (value and address) for one daedalus value
    let converted_params_num = dd_func.call_signature.parameters_num;
    // dbg!(&dd_func.name);

    converter.tmp_val_0 = converted_params_num;
    converter.tmp_val_1 = converted_params_num + 1;
    converter.func_index_set_instance = func_index_set_instance;
    converter.compute_instruction_offsets(dd_func);

    let mut func = Function::new([(2, ValType::I32)]);
    let mut block = converter.split_blocks();
    block.remove_instructions_after_return();
    block.set_function_returns(dd_func.call_signature.returns);
    block.analyse_stack_usage(module_builder);

    for index in 0..converted_params_num {
        func.instruction(&Instruction::LocalGet(index));
    }
    converter.lower_block(&mut func, &block);

    func.instruction(&Instruction::End);

    let params = dd_func.call_signature.parameters();
    let results = dd_func.call_signature.results();

    ConvertedFunction {
        name: dd_func.name.clone(),
        func,
        params,
        results,
    }
}
