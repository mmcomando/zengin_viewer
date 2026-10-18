/// All opcodes supported by the daedalus interpreter.
///
/// In the documentation for each opcode, the following conventions apply.
/// - `a` refers to the first value on the stack and is required to be an integer.
/// - `b` refers to the second value on the stack and is required to be an integer.
/// - `m` refers to the first value on the stack which may be of any non-reference type.
/// - `n` refers to the second value on the stack which may be of any non-reference type.
/// - `x` refers to the first value on the stack and is required to be variable reference.
/// - `y` refers to the second value on the stack and is required to be variable reference.
///
/// Documentation taken from:
/// <https://github.com/GothicKit/ZenKit/blob/main/include/zenkit/DaedalusScript.hh>.
#[expect(clippy::exhaustive_enums, reason = "DaedalusScript won't change")]
#[expect(
    clippy::arbitrary_source_item_ordering,
    reason = "Ordering matches source material"
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Instruction {
    /// Add `a` and `b` and put the result back onto the stack.
    Add,

    /// Subtract `b` from `a` and put the result back onto the stack.
    Sub,

    /// Multiply `a` and `b` and put the result back onto the stack.
    Mul,

    /// Divide `a` by `b` and put the result back onto the stack.
    Div,

    /// Divide `a` by `b` and put the remainder back onto the stack.
    Mod,

    /// Compute the bitwise or of `a` and `b` and put the result back onto the stack.
    OrBitwise,

    /// Compute the bitwise and of `a` and `b` and put the result back onto the stack.
    AndBitwise,

    /// Test if `a` is less than `b` and put `1` or `0` onto the stack if
    ///        the test is true or false respectively.
    Lower,

    /// Test if `a` is greater than `b` and put `1` or `0` onto the stack
    ///        if the test is true or false respectively.
    Higher,

    /// Write `b` to `x` as an integer.
    AssignInteger,

    /// Test if `a` is less than or equal to `b` and put `1` or `0` onto the
    ///        stack if the test is true or false respectively.
    LowerEqual,

    /// Test if `a` is equal to `b` and put `1` or `0` onto the
    ///        stack if the test is true or false respectively.
    Equal,

    /// Test if `a` is not equal to `b` and put `1` or `0` onto the
    ///        stack if the test is true or false respectively.
    NotEqual,

    /// Test if `a` is greater than or equal to `b` and put `1` or `0` onto the
    ///        stack if the test is true or false respectively.
    HigherEqual,

    /// Add `x` and `b` and assign the result back to `x`.
    /// `x` must be a reference to an integer.
    AddTo,

    /// Subtract `b` from `x` and assign the result back to `x`.
    /// `x` must be a reference to an integer.
    SubTo,

    /// Compute `+a` and put the result back onto the stack.
    UnaryPlus,

    /// Compute `-a` and put the result back onto the stack.
    UnaryMinus,

    /// Compute `!a` and put the result back onto the stack.
    UnaryNot,

    /// Compute the bitwise complement (`~a`) and put the result back onto the stack.
    Complement,

    /// Test if `a == 1` or `b == 1` and put `1` or `0` onto the stack if
    /// the test is true or false respectively.
    LogicalOr,

    /// Test if `a == 1` and `b == 1` and put `1` or `0` onto the stack if
    /// the test is true or false respectively.
    LogicalAnd,

    /// Return from the currently running function.
    Return,

    /// Call the function at the address provided in the instruction.
    Call(u32),

    /// Call the external function at the symbol index provided in the instruction.
    CallExternal(u32),

    /// Push the immediate value provided in the instruction onto the stack as an integer.
    PushInt(i32),

    /// Push the symbol with the index provided in the instruction onto the stack as a reference.
    PushVar(u32),

    /// Push the instance with the symbol index provided in the instruction onto the stack as a reference.
    PushInstance(u32),

    /// Write `m` to `x` as a string.
    AssignString,

    /// Write `b` to `x` as a function reference.
    AssignFunc,

    /// Write `b` to `x` as a floating point number.
    AssignFloat,

    /// Write `y` to `x` as an instance reference.
    AssignInstance,

    /// Immediately jump to the instruction at the address provided in the instruction.
    Jump(u32),

    /// Jump to the instruction at the address provided in the instruction if `a == 0`.
    JumpIf(u32),

    /// Set the global instance reference to the instance with the symbol index provided in the instruction.
    SetInstance(u32),

    /// Push the element at the given index of the symbol with the index provided in the
    /// instruction onto the stack as a reference.
    PushArrayVar(u32, u8),
}

impl Instruction {
    #[must_use]
    /// Presentation name of daedalus instruction.
    pub fn name(&self) -> &str {
        match *self {
            Instruction::Add => "Add",
            Instruction::Sub => "Sub",
            Instruction::Mul => "Mul",
            Instruction::Div => "Div",
            Instruction::Mod => "Mod",
            Instruction::OrBitwise => "OrBitwise",
            Instruction::AndBitwise => "AndBitwise",
            Instruction::AssignInteger => "AssignInteger",
            Instruction::LowerEqual => "LowerEqual",
            Instruction::Equal => "Equal",
            Instruction::NotEqual => "NotEqual",
            Instruction::Lower => "Lower",
            Instruction::Higher => "Higher",
            Instruction::HigherEqual => "HigherEqual",
            Instruction::AddTo => "AddTo",
            Instruction::SubTo => "SubTo",
            Instruction::UnaryPlus => "UnaryPlus",
            Instruction::UnaryMinus => "UnaryMinus",
            Instruction::UnaryNot => "UnaryNot",
            Instruction::Complement => "Complement",
            Instruction::LogicalOr => "LogicalOr",
            Instruction::LogicalAnd => "LogicalAnd",
            Instruction::Return => "Return",
            Instruction::Call(_) => "Call",
            Instruction::CallExternal(_) => "CallExternal",
            Instruction::PushInt(_) => "PushInt",
            Instruction::PushVar(_) => "PushVar",
            Instruction::PushInstance(_) => "PushInstance",
            Instruction::AssignString => "AssignString",
            Instruction::AssignFunc => "AssignFunc",
            Instruction::AssignFloat => "AssignFloat",
            Instruction::AssignInstance => "AssignInstance",
            Instruction::Jump(_) => "Jump",
            Instruction::JumpIf(_) => "JumpIf",
            Instruction::SetInstance(_) => "SetInstance",
            Instruction::PushArrayVar(_, _) => "PushArrayVar",
        }
    }

    #[must_use]
    /// Size in bytes of given instruction in binary script representation.
    pub fn size(&self) -> usize {
        match *self {
            Instruction::Add
            | Instruction::Sub
            | Instruction::Mul
            | Instruction::Div
            | Instruction::Mod
            | Instruction::OrBitwise
            | Instruction::AndBitwise
            | Instruction::AssignInteger
            | Instruction::LowerEqual
            | Instruction::Equal
            | Instruction::NotEqual
            | Instruction::Lower
            | Instruction::Higher
            | Instruction::HigherEqual
            | Instruction::AddTo
            | Instruction::SubTo
            | Instruction::UnaryPlus
            | Instruction::UnaryMinus
            | Instruction::UnaryNot
            | Instruction::Complement
            | Instruction::LogicalOr
            | Instruction::LogicalAnd => 1,
            Instruction::Return
            | Instruction::AssignString
            | Instruction::AssignFunc
            | Instruction::AssignFloat
            | Instruction::AssignInstance => 1,
            Instruction::Call(_)
            | Instruction::CallExternal(_)
            | Instruction::PushInt(_)
            | Instruction::PushVar(_)
            | Instruction::PushInstance(_)
            | Instruction::Jump(_)
            | Instruction::JumpIf(_)
            | Instruction::SetInstance(_) => 5,
            Instruction::PushArrayVar(_, _) => 6,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Instruction;

    #[test]
    fn instruction_size() {
        assert_eq!(Instruction::Add.size(), 1);
        assert_eq!(Instruction::Return.size(), 1);
        assert_eq!(Instruction::PushInt(42).size(), 5);
        assert_eq!(Instruction::Call(7).size(), 5);
        assert_eq!(Instruction::JumpIf(11).size(), 5);
        assert_eq!(Instruction::PushArrayVar(3, 2).size(), 6);
        assert_eq!(Instruction::Complement.size(), 1);
    }
}
