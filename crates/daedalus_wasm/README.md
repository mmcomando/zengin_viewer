# About
Library to convert daedalus scripts to wasm. Conversion is supported only for scripts which are already compiled by Gothic II (.dat files).
Produced wasm is a wasm component module following interface defined in [WIT file](/wit/world.wit).

# Library usage
```rust
use daedalus_wasm::wasm::convert_daedalus_script;
let wasm_bytes = convert_daedalus_script(&PathBuf::from("path_to_gothic_scripts/GOTHIC.dat"));
```

# Challenges
Daedalus and wasm are both stack based virtual machines but there are differences between these two which are harder to solve.

## Control flow
Daedalus assembly uses unstructured control flow (goto) but wasm uses structured control flow (ifs, blocks).
Converting from unstructured to structured might be challenging in some corner cases.
In this library we cut corners by assuming that Gothic daedalus compiler doesn't optimize and produces straight forward assembly.
Daedalus also doesn't support loops. This two facts allows us to use simplified algorithm to reconstruct structured control flow.

## Push instructions
When daedalus push instruction pushes value to the stack but also allows to get source of this value for later instructions. It provides value and address of given variable.
Wasm when pushes to the stack only value information of variable is preserved, and later assign daedalus instructions can't be translated to wasm as we don't know where to assign.
This requires additional analysis pass gather information if daedalus instruction has to push value or reference to a variable.


# TODO
- support setting and assigning strings
