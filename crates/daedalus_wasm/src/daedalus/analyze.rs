use core::fmt::Debug;

use crate::{daedalus::Instruction, wasm::BinaryBuilder};

use crate::daedalus::Instruction as InstDD;
use crate::daedalus::script::CallSignature;

#[derive(Debug, Clone, PartialEq)]
/// Decide if value provided by push instructions should be a value or a reference.
pub(crate) enum ProvideValType {
    Ref,
    Val,
}

#[derive(Debug, Clone)]
/// Daedalus instructions + additional info required for wasm generation.
pub(crate) struct AnalyzedInstruction {
    pub instr: Instruction,
    // Only push values can decide if they produce ref or value
    pub provide_val_type: ProvideValType,
    pub value_is_used: bool,
    pub implicit_return: bool,
}

impl AnalyzedInstruction {
    pub(crate) fn new(instr: Instruction, provide_val_type: ProvideValType) -> AnalyzedInstruction {
        Self {
            instr,
            provide_val_type,
            value_is_used: false,
            implicit_return: false,
        }
    }
}

#[derive(Debug, Clone)]
/// Store information for how many values are produced by given instruction.
struct InstructionProducesValue {
    produces: i32,
    consumes: i32,
}

impl InstructionProducesValue {
    fn new(produces: i32, consumes: i32) -> Self {
        Self { produces, consumes }
    }
}

#[expect(
    clippy::multiple_inherent_impl,
    reason = "Analysis specific helpers live in the analyze module"
)]
impl Instruction {
    fn call_signature(self, binary_builder: &BinaryBuilder) -> Option<&CallSignature> {
        #[expect(
            clippy::wildcard_enum_match_arm,
            reason = "We only care about few Call"
        )]
        match self {
            Instruction::Call(instr_offset) => Some(
                &binary_builder
                    .instr_offset_to_func_info
                    .get(&instr_offset)
                    .unwrap_or_else(|| {
                        panic!("Missing signature for instruction_address({instr_offset})")
                    })
                    .signature,
            ),
            Instruction::CallExternal(symbol_index) => {
                let symbol_info = binary_builder.symbol_to_info.get(&symbol_index)?;
                Some(&symbol_info.func_info.as_ref()?.signature)
            }
            _ => None,
        }
    }
    /// How many values given instruction adds to stack.
    fn instruction_stack_effect(self, binary_builder: &BinaryBuilder) -> InstructionProducesValue {
        match self {
            Self::Add
            | Self::Sub
            | Self::Mul
            | Self::Div
            | Self::Mod
            | Self::OrBitwise
            | Self::AndBitwise
            | Self::Lower
            | Self::Higher
            | Self::LowerEqual
            | Self::Equal
            | Self::NotEqual
            | Self::HigherEqual
            | Self::LogicalOr
            | Self::LogicalAnd => InstructionProducesValue::new(1, 2),
            Self::AssignString
            | Self::AssignFunc
            | Self::AssignFloat
            | Self::AssignInstance
            | Self::AssignInteger
            | Self::AddTo
            | Self::SubTo => InstructionProducesValue::new(0, 2),
            Self::UnaryPlus | Self::UnaryMinus | Self::UnaryNot => {
                InstructionProducesValue::new(1, 1)
            }
            Self::SetInstance(_) | Self::Return => InstructionProducesValue::new(0, 0),

            Self::Call(_) | Self::CallExternal(_) => {
                let signature = self.call_signature(binary_builder).unwrap();
                InstructionProducesValue::new(
                    i32::from(signature.returns),
                    signature.parameters_num.cast_signed(),
                )
            }
            Self::PushInt(_)
            | Self::PushVar(_)
            | Self::PushInstance(_)
            | Self::PushArrayVar(_, _) => InstructionProducesValue::new(1, 0),
            Self::Jump(_) | Self::JumpIf(_) => {
                panic!("Jump instructions should have been lowered in higher level steps");
            }
        }
    }
}

#[derive(Debug, Clone)]
/// Information how many values are added to stack (might be negative) and if instructions have early return.
struct InstructionsEffect {
    effect: i32,
    // Instructions produce early return
    returns: bool,
}

impl InstructionsEffect {
    fn new(effect: i32, returns: bool) -> Self {
        Self { effect, returns }
    }
}

#[derive(Debug, Clone, Copy)]
/// Information by whom the stack value is produced.
enum StackProducer {
    FunctionInput,
    Instruction {
        block_index: usize,
        instruction_index: usize,
    },
    ControlBlock {
        block_index: usize,
    },
}

impl StackProducer {
    // Get instruction index of a given stack value.
    fn instruction_index_in_block(self, expected_block_index: usize) -> usize {
        let Self::Instruction {
            block_index,
            instruction_index,
        } = self
        else {
            panic!(
                "Expected that value was produced by instruction in instruction block, but got {self:?}"
            );
        };
        assert_eq!(
            block_index, expected_block_index,
            "All ref values are expected to be produced in same block"
        );
        instruction_index
    }
}

#[derive(Debug, Clone)]
/// Block of instructions without inner control flow.
pub(crate) struct InstructionBlock {
    pub(crate) instructions: Vec<AnalyzedInstruction>,
    pub(crate) effect: i32,
}

impl InstructionBlock {
    pub(crate) fn new(instructions: Vec<AnalyzedInstruction>) -> Self {
        Self {
            instructions,
            effect: 0,
        }
    }

    // pub fn pretty_print(&self, indent: usize) {
    //     println!(
    //         "{:indent$}InstructionBlock, effect({})",
    //         "",
    //         self.effect.get(),
    //         indent = indent,
    //     );
    //     for instr in &self.instructions {
    //         println!(
    //             "{:indent$}{:?},   provide_val_type({:?}) value_is_used({}) implicit_return({})",
    //             "",
    //             instr.instr,
    //             instr.provide_val_type,
    //             instr.value_is_used,
    //             instr.implicit_return,
    //             indent = indent + 2,
    //         );
    //     }
    // }

    /// In real daedalus or in our tests there are cases when there are instructions after Return in given block.
    /// They are unreachable and create problem in later instruction analysis (like stack effect analysis) so remove them.
    /// Return true if instructions end with return statement.
    pub fn trim_instructions_until_return(&mut self) -> bool {
        let Some(return_instr_index) = self
            .instructions
            .iter()
            .position(|el| el.instr == InstDD::Return)
        else {
            return false;
        };
        if return_instr_index == self.instructions.len() - 1 {
            return true;
        }
        self.instructions.drain(return_instr_index + 1..);
        true
    }

    /// We have to analyse instructions to know if push instructions have to produce value or reference.
    /// We do it by iterating over instructions and marking what push instructions should produce.
    /// We also analyse if instructions are used or not.
    #[expect(clippy::too_many_lines, reason = "Maybe simplify in the future")]
    fn analyse_stack_usage(
        &mut self,
        binary_builder: &BinaryBuilder,
        stack_values: &mut Vec<StackProducer>,
        block_index: usize,
        mark_used: &mut Vec<StackProducer>,
        function_returns: bool,
    ) {
        let mut instructions_to_produce_ref: Vec<usize> = vec![];

        for (index, instr) in self.instructions.iter_mut().enumerate() {
            match instr.instr {
                Instruction::Add
                | Instruction::Sub
                | Instruction::Mul
                | Instruction::Div
                | Instruction::Mod
                | Instruction::OrBitwise
                | Instruction::AndBitwise
                | Instruction::LowerEqual
                | Instruction::Equal
                | Instruction::NotEqual
                | Instruction::Lower
                | Instruction::Higher
                | Instruction::HigherEqual
                | Instruction::LogicalOr
                | Instruction::LogicalAnd => {
                    // Consumes two values
                    mark_used.push(stack_values.pop().unwrap());
                    mark_used.push(stack_values.pop().unwrap());
                    // Provides one value
                    stack_values.push(StackProducer::Instruction {
                        block_index,
                        instruction_index: index,
                    });
                }

                Instruction::AssignString
                | Instruction::AssignInteger
                | Instruction::AssignFunc
                | Instruction::AssignFloat
                | Instruction::AssignInstance
                | Instruction::AddTo
                | Instruction::SubTo => {
                    instr.value_is_used = true;
                    // Consumes ref and value
                    let ref_producer = stack_values.pop().unwrap();
                    mark_used.push(ref_producer);
                    mark_used.push(stack_values.pop().unwrap());
                    // Mark that reference is needed
                    instructions_to_produce_ref
                        .push(ref_producer.instruction_index_in_block(block_index));
                }
                Instruction::UnaryPlus | Instruction::UnaryMinus | Instruction::UnaryNot => {
                    // Consumes one value
                    mark_used.push(stack_values.pop().unwrap());
                    // Produces one value
                    stack_values.push(StackProducer::Instruction {
                        block_index,
                        instruction_index: index,
                    });
                }
                Instruction::Call(_) | Instruction::CallExternal(_) => {
                    let signature = instr.instr.call_signature(binary_builder).unwrap();
                    for _index in 0..signature.parameters_num {
                        mark_used.push(stack_values.pop().unwrap());
                    }
                    if signature.returns {
                        stack_values.push(StackProducer::Instruction {
                            block_index,
                            instruction_index: index,
                        });
                    } else {
                        instr.value_is_used = true;
                    }
                }

                Instruction::PushInt(_)
                | Instruction::PushInstance(_)
                | Instruction::PushVar(_)
                | Instruction::PushArrayVar(_, _) => {
                    // Produces one value or reference
                    stack_values.push(StackProducer::Instruction {
                        block_index,
                        instruction_index: index,
                    });
                }
                Instruction::Return => {
                    instr.value_is_used = true;
                    if function_returns {
                        if stack_values.is_empty() {
                            // Some daedalus functions don't push any instructions where function seems to want to return false
                            // TODO: Daedalus implicitly returns false?
                            instr.implicit_return = true;
                        } else {
                            mark_used.push(stack_values.pop().unwrap());
                        }
                    }
                }
                Instruction::SetInstance(_) => {
                    instr.value_is_used = true;
                }

                Instruction::Jump(_) | Instruction::JumpIf(_) => {
                    panic!(
                        "This instruction should be not present in analysis as they should be previously converted to ControlBlocks"
                    )
                }
            }
        }

        for index in instructions_to_produce_ref {
            #[expect(
                clippy::wildcard_enum_match_arm,
                reason = "We care only about selected instructions"
            )]
            match self.instructions[index].instr {
                Instruction::PushVar(_)
                | Instruction::PushInstance(_)
                | Instruction::PushArrayVar(_, _) => {
                    self.instructions[index].provide_val_type = ProvideValType::Ref;
                }
                _ => {
                    panic!("Non push instruction can't produce ref value");
                }
            }
        }
    }

    /// Compute how many values are pushed to stack (might be negative).
    fn compute_stack_effect(
        &mut self,
        binary_builder: &BinaryBuilder,
        function_returns: bool,
    ) -> InstructionsEffect {
        let mut delta = 0;

        for instruction in &self.instructions {
            let instr_val_info = instruction.instr.instruction_stack_effect(binary_builder);
            delta += instr_val_info.produces - instr_val_info.consumes;
            if instr_val_info.produces > 0 && !instruction.value_is_used {
                delta -= 1;
            }
        }

        let last_instr_is_ret = self.instructions.last().unwrap().instr == InstDD::Return;
        if last_instr_is_ret && function_returns {
            delta -= 1;
        }
        self.effect = delta;

        InstructionsEffect::new(delta, last_instr_is_ret)
    }
}

#[derive(Debug, Clone)]
// Block of instructions with structured control flow.
pub enum ControlBlock {
    Instructions(InstructionBlock),
    If(Block),
    IfElse { if_block: Block, else_block: Block },
}

impl ControlBlock {
    // pub fn pretty_print(&self, indent: usize) {
    //     match self {
    //         ControlBlock::Instructions(instructions) => {
    //             println!("{:indent$}Instructions", "", indent = indent);
    //             instructions.pretty_print(indent + 2);
    //         }
    //         ControlBlock::If(if_block) => {
    //             println!("{:indent$}If Block", "", indent = indent);
    //             if_block.pretty_print(indent + 2);
    //         }
    //         ControlBlock::IfElse {
    //             if_block,
    //             else_block,
    //         } => {
    //             println!("{:indent$}If/Else if Block", "", indent = indent);
    //             if_block.pretty_print(indent + 2);
    //             println!("{:indent$}If/Else else Block", "", indent = indent);
    //             else_block.pretty_print(indent + 2);
    //         }
    //     }
    // }

    /// How many stack values given block produces.
    pub fn stack_effect(&self) -> i32 {
        match self {
            ControlBlock::Instructions(instructions) => instructions.effect,
            ControlBlock::If(_) => -1,
            ControlBlock::IfElse {
                if_block,
                else_block,
            } => {
                match (
                    if_block.returns,
                    else_block.returns,
                    if_block.effect,
                    else_block.effect,
                ) {
                    (true, false, _, else_effect) => -1 + else_effect,
                    (false, true, if_effect, _) => -1 + if_effect,

                    // consumes if arg, and does early return
                    (true, true, if_effect, else_effect) if if_effect == else_effect => {
                        -1 + if_effect
                    }
                    (false, false, if_effect, else_effect) if if_effect == else_effect => {
                        -1 + if_effect
                    }
                    _ => panic!(),
                }
            }
        }
    }

    /// Mark that last instruction in a block is used. Needed for if/else statements which produce value.
    fn mark_last_instruction_as_used(&mut self) {
        match self {
            ControlBlock::Instructions(instructions) => {
                instructions.instructions.last_mut().unwrap().value_is_used = true;
            }
            ControlBlock::If(if_block) => {
                if_block
                    .blocks
                    .last_mut()
                    .unwrap()
                    .mark_last_instruction_as_used();
            }
            ControlBlock::IfElse {
                if_block,
                else_block,
            } => {
                if_block
                    .blocks
                    .last_mut()
                    .unwrap()
                    .mark_last_instruction_as_used();
                else_block
                    .blocks
                    .last_mut()
                    .unwrap()
                    .mark_last_instruction_as_used();
            }
        }
    }

    fn mark_used_instruction(&mut self, index: usize) {
        match self {
            ControlBlock::Instructions(instructions) => {
                instructions.instructions[index].value_is_used = true;
            }
            ControlBlock::If(if_block) => {
                assert_eq!(index, 0, "if block should have instruction index as 0");
                if_block
                    .blocks
                    .last_mut()
                    .unwrap()
                    .mark_last_instruction_as_used();
            }
            ControlBlock::IfElse {
                if_block,
                else_block,
            } => {
                assert_eq!(index, 0, "if/else block should have instruction index as 0");
                if_block
                    .blocks
                    .last_mut()
                    .unwrap()
                    .mark_last_instruction_as_used();
                else_block
                    .blocks
                    .last_mut()
                    .unwrap()
                    .mark_last_instruction_as_used();
            }
        }
    }
}

#[derive(Debug, Clone)]
// Block of instructions in given scope.
// Main block in a function should have an effect 0.
pub struct Block {
    pub blocks: Vec<ControlBlock>,
    pub function_parameters_num: u32,
    pub function_returns_value: bool,
    pub effect: i32,
    pub returns: bool,
}

impl Block {
    pub fn new() -> Self {
        Block {
            blocks: vec![],
            function_parameters_num: 0,
            function_returns_value: false,
            effect: 0,
            returns: false,
        }
    }

    // pub fn pretty_print(&self, indent: usize) {
    //     println!(
    //         "{:indent$}Block, effect_before_block({}), function_returns_value({}), effect({}), returns({})",
    //         "",
    //         self.effect_before.get(),
    //         self.function_returns_value.get(),
    //         self.effect.get(),
    //         self.returns.get(),
    //         indent = indent
    //     );
    //     for block in &self.blocks {
    //         block.pretty_print(indent + 2);
    //     }
    // }

    // Mark that this functions returns value.
    pub fn set_function_returns(&mut self, returns: bool) {
        self.function_returns_value = returns;
        for block in &mut self.blocks {
            match block {
                ControlBlock::Instructions(_) => {}
                ControlBlock::If(if_block) => {
                    if_block.set_function_returns(returns);
                }
                ControlBlock::IfElse {
                    if_block,
                    else_block,
                } => {
                    if_block.set_function_returns(returns);
                    else_block.set_function_returns(returns);
                }
            }
        }
    }

    pub fn remove_instructions_after_return(&mut self) -> bool {
        let mut preserve_block_up_to: Option<usize> = None;
        for (index, block) in self.blocks.iter_mut().enumerate() {
            match block {
                ControlBlock::Instructions(instructions) => {
                    if instructions.trim_instructions_until_return() {
                        preserve_block_up_to = Some(index);
                        break;
                    }
                }
                ControlBlock::If(if_block) => {
                    // If block doesn't stop later instructions from execution even when it returns
                    if_block.remove_instructions_after_return();
                }
                ControlBlock::IfElse {
                    if_block,
                    else_block,
                } => {
                    let early_return_a = if_block.remove_instructions_after_return();
                    let early_return_b = else_block.remove_instructions_after_return();
                    if early_return_a && early_return_b {
                        preserve_block_up_to = Some(index);
                        break;
                    }
                }
            }
        }
        if let Some(preserve_block_up_to) = preserve_block_up_to {
            self.blocks.drain(preserve_block_up_to + 1..);
            // assert!(self.block_returns());
            return true;
        }
        false
    }

    // Check if if/else branch returns and if has effect on stack
    fn if_else_branch_delta(
        then_delta: InstructionsEffect,
        else_delta: InstructionsEffect,
    ) -> InstructionsEffect {
        match (then_delta.returns, else_delta.returns) {
            // Two branches return
            (true, true) => return InstructionsEffect::new(else_delta.effect, true),
            // One of branches returns from function, other branch determines stack effect
            (true, false) => return InstructionsEffect::new(else_delta.effect, false),
            (false, true) => return InstructionsEffect::new(then_delta.effect, false),
            // Both branches return same number of values
            (false, false) if then_delta.effect == else_delta.effect => {
                assert!(
                    then_delta.effect == 0 || then_delta.effect == 1,
                    "if/else can produce one value or none"
                );
                return InstructionsEffect::new(then_delta.effect, false);
            }
            _ => {} // Fall through if effects differ and neither branch returns
        }
        panic!(
            // TODO if stack delta if different maybe value is just unused and has to be dropped
            "If-else branches leave different stack deltas: {} vs {}, it is illegal for wasm",
            then_delta.effect, else_delta.effect
        )
    }

    fn analyse_stack_usage_impl(
        &mut self,
        binary_builder: &BinaryBuilder,
        stack_values: &mut Vec<StackProducer>,
    ) {
        let mut mark_used: Vec<StackProducer> = vec![];

        for (block_index, control_block) in self.blocks.iter_mut().enumerate() {
            match control_block {
                ControlBlock::Instructions(instructions) => {
                    instructions.analyse_stack_usage(
                        binary_builder,
                        stack_values,
                        block_index,
                        &mut mark_used,
                        self.function_returns_value,
                    );
                }
                ControlBlock::If(if_block) => {
                    mark_used.push(stack_values.pop().unwrap());
                    let mut new_stack_values: Vec<StackProducer> = vec![];
                    if_block.analyse_stack_usage_impl(binary_builder, &mut new_stack_values);
                }
                ControlBlock::IfElse {
                    if_block,
                    else_block,
                } => {
                    mark_used.push(stack_values.pop().unwrap());

                    let mut new_stack_values: Vec<StackProducer> = vec![];
                    if_block.analyse_stack_usage_impl(binary_builder, &mut new_stack_values);
                    let if_block_stack_values_len = new_stack_values.len();
                    new_stack_values.clear();
                    else_block.analyse_stack_usage_impl(binary_builder, &mut new_stack_values);

                    // Control block produces value
                    if if_block_stack_values_len > 0 || !new_stack_values.is_empty() {
                        stack_values.push(StackProducer::ControlBlock { block_index });
                    }
                }
            }
        }

        for producer in mark_used {
            match producer {
                StackProducer::FunctionInput => {}
                StackProducer::Instruction {
                    block_index,
                    instruction_index,
                } => self.blocks[block_index].mark_used_instruction(instruction_index),
                StackProducer::ControlBlock { block_index } => {
                    self.blocks[block_index].mark_used_instruction(0);
                }
            }
        }
    }

    fn compute_stack_effect(&mut self, binary_builder: &BinaryBuilder) -> InstructionsEffect {
        let mut my_delta = InstructionsEffect::new(0, false);

        for control_block in &mut self.blocks {
            match control_block {
                ControlBlock::Instructions(instructions) => {
                    let effect = instructions
                        .compute_stack_effect(binary_builder, self.function_returns_value);
                    my_delta.effect += effect.effect;
                    my_delta.returns |= effect.returns;
                }
                ControlBlock::If(if_block) => {
                    let effect = if_block.compute_stack_effect(binary_builder);
                    if effect.effect == 0 || effect.returns {
                        // If instruction used one value
                        my_delta.effect -= 1;
                    }
                }
                ControlBlock::IfElse {
                    if_block,
                    else_block,
                } => {
                    let then_delta = if_block.compute_stack_effect(binary_builder);
                    let else_delta = else_block.compute_stack_effect(binary_builder);
                    let effect = Self::if_else_branch_delta(then_delta, else_delta);
                    my_delta.effect += effect.effect - 1;
                    my_delta.returns |= effect.returns;
                }
            }
        }

        self.effect = my_delta.effect;
        self.returns = my_delta.returns;

        my_delta
    }

    /// Analyse which instructions are used and which push instructions should produce reference.
    pub fn analyse_stack_usage(&mut self, binary_builder: &BinaryBuilder) {
        let mut stack_values: Vec<StackProducer> = vec![];
        for _ in 0..self.function_parameters_num {
            stack_values.push(StackProducer::FunctionInput);
        }

        self.analyse_stack_usage_impl(binary_builder, &mut stack_values);
        self.compute_stack_effect(binary_builder);
        self.fixup_if_else_early_return();
    }

    // Fix up case where last block is if/else with return instruction in both branches and function returns value.
    // In such a case wasm complains about missing value at the end of function even when instructions after if/else are unreachable
    // We can fix this two ways:
    // 1. Make if/else return value (but it returns inside so conceptually it doesn't produce any value)
    // 2. Add fake instructions to the end of function
    //
    // We chose option 2 as it reduces corner case in stack analysis pass
    pub fn fixup_if_else_early_return(&mut self) {
        if !self.function_returns_value {
            // There is no problem in void functions
            return;
        }

        let last_block = self.blocks.last().unwrap();
        if let ControlBlock::IfElse { .. } = last_block {
            if last_block.stack_effect() != -1 {
                // If if/else doesn't produce value anyway
                return;
            }

            // Add unreachable instructions to satisfy wasm validation
            let mut instructions = vec![AnalyzedInstruction::new(
                Instruction::Return,
                ProvideValType::Val,
            )];
            for instr in &mut instructions {
                instr.value_is_used = true;
                instr.implicit_return = true;
            }
            let block = ControlBlock::Instructions(InstructionBlock::new(instructions));
            self.blocks.push(block);
        }
    }
}
