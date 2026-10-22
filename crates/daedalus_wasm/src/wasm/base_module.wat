(module
  ;; Type used by call_indirect for functions with no parameters and no results
  (type $void_void_func (func))
  ;; Type used by call_indirect for functions with no parameters returning i32 (treated as bool)
  (type $bool_void_func (func (result i32)))

  ;; Memories, indices are significant, they match constants in DaedalusMemoryLayout (memory.rs):
  ;; 0 - main memory: allocator region, (ptr, size) pairs of symbol names and static strings
  ;; 1 - symbol info: 16-byte record per symbol:
  ;;     kind @0 (byte), is class member @1 (byte), function returns value @2 (byte),
  ;;     address or offset @4, wasm function table index @8, parent @12 (u32::MAX when symbol has no parent)
  ;; 2 - symbol default values
  ;; Minimal sizes are placeholders, they are extended with daedalus data during merge.
  (memory $main 1)
  (memory $symbol_info 1)
  (memory $symbols 1)

  ;; Function table with with daedalus functions, used so we can dynamically call any daedalus function from wasm
  (table $funcs 0 funcref)

  ;; Globals, indices are significant, they match constants in wasm.rs
  (global $current_class_instance_memory (mut i32) (i32.const 0))
  (global $current_instance_symbol_index (mut i32) (i32.const 0))
  (global $allocator (mut i32) (i32.const 0))
  ;; Number of all symbols, patched with real value during merge with daedalus data
  (global $symbols_num (mut i32) (i32.const 0))
  ;; Symbol indices of the well known daedalus instance variables, patched with
  ;; real values during merge with daedalus data, -1 when not declared
  (global $self_symbol_index (mut i32) (i32.const 0))
  (global $other_symbol_index (mut i32) (i32.const 0))
  (global $victim_symbol_index (mut i32) (i32.const 0))
  (global $item_symbol_index (mut i32) (i32.const 0))
  (global $hero_symbol_index (mut i32) (i32.const 0))
  ;; Number of all string pairs in memory, patched with real value during merge
  ;; with daedalus data
  (global $strings_num (mut i32) (i32.const 0))


  ;; Compares two strings given as (ptr, size) pairs
  (func $compare_strings (param $ptr_a i32) (param $len_a i32) (param $ptr_b i32) (param $len_b i32) (result i32)
    (local $index i32) (local $byte_a i32) (local $byte_b i32)

    ;; If lengths are not equal return 0
    local.get $len_a
    local.get $len_b
    i32.ne
    if
      i32.const 0
      return
    end

    loop (result i32)
      ;; if string ends it means all characters where the same
      local.get $index
      local.get $len_a
      i32.ge_u
      if
        i32.const 1
        return
      end

      ;; Load byte from first string
      local.get $index
      local.get $ptr_a
      i32.add
      i32.load8_u $main offset=0
      local.set $byte_a

      ;; Load byte from second string
      local.get $index
      local.get $ptr_b
      i32.add
      i32.load8_u $main offset=0
      local.set $byte_b

      ;; If bytes are different strings are not the same
      local.get $byte_a
      local.get $byte_b
      i32.ne
      if
        i32.const 0
        return
      end

      ;; Increment index
      local.get $index
      i32.const 1
      i32.add
      local.set $index

      ;; Return to start of the loop
      br 0
    end
  )

  ;; Dummy memory allocator, it rotates through 128 byte blocks
  (func $cabi_realloc (param $old_ptr i32) (param $old_size i32) (param  i32) (param $new_size i32) (result i32)
    ;; Increment allocator with rotate
    global.get $allocator
    i32.const 1
    i32.add
    i32.const 127 ;; Rotate trick, number of blocks - 1
    i32.and
    global.set $allocator

    ;; Return address
    global.get $allocator
    i32.const 128
    i32.mul
  )


  ;; Returns address of symbol value:
  ;; global variable address or current instance memory + class member offset
  (func $get_memory_value_i32 (param $symbol_index i32) (result i32)
    (local $record_base i32)

    local.get $symbol_index
    i32.const 16
    i32.mul
    local.set $record_base
    local.get $record_base
    i32.load8_u $symbol_info offset=1 ;; is class member
    global.get $current_class_instance_memory
    i32.mul

    local.get $record_base
    i32.load $symbol_info offset=4 ;; address or offset
    i32.add
  )

  ;; Sets the current instance context (symbol index and its member memory
  ;; address) used for class member access. This is what the daedalus
  ;; `SetInstance` instruction means, converted functions call this builtin.
  ;; If index of this function changes change BASE_FUNC_SET_INSTANCE
  (func $set_instance (param $symbol_id i32)
    local.get $symbol_id
    global.set $current_instance_symbol_index

    local.get $symbol_id
    call $get_memory_value_i32
    global.set $current_class_instance_memory
  )

  ;; Writes an instance reference into the value slot of the given symbol, the
  ;; daedalus `symbol = instance` assignment. Does nothing when the symbol or
  ;; the instance id is not valid.
  (func $assign_instance (param $symbol_id i32) (param $instance_id i32)
    local.get $symbol_id
    i32.const 0
    i32.ge_s
    local.get $instance_id
    i32.const 0
    i32.gt_s
    i32.and
    if
      local.get $symbol_id
      call $get_memory_value_i32
      local.get $instance_id
      i32.store $symbols
    end
  )

  ;; Binds one of the well known daedalus instance variables (`self`, `other`,
  ;; `victim`, `item`, `hero`) to the given instance, like the original engine
  ;; does before it invokes a script function for a specific npc (zenkit
  ;; `ScopeVar` self/other bindings). Does nothing when the script does not
  ;; declare the variable (its symbol index is patched to -1).
  (func $set_self (param $instance_id i32)
    global.get $self_symbol_index
    local.get $instance_id
    call $assign_instance
  )

  (func $set_other (param $instance_id i32)
    global.get $other_symbol_index
    local.get $instance_id
    call $assign_instance
  )

  (func $set_victim (param $instance_id i32)
    global.get $victim_symbol_index
    local.get $instance_id
    call $assign_instance
  )

  (func $set_item (param $instance_id i32)
    global.get $item_symbol_index
    local.get $instance_id
    call $assign_instance
  )

  (func $set_hero (param $instance_id i32)
    global.get $hero_symbol_index
    local.get $instance_id
    call $assign_instance
  )

  ;; Calls npc_routine function of the instance
  (func $call_npc_routine (param $symbol_id i32)
    (local $routine_symbol_index i32)

    (call $get_class_member (local.get $symbol_id) (i32.const 608))
    ;; If < 1 no npc_routine set, so early return
    local.tee $routine_symbol_index
    i32.const 1
    i32.le_s
    if
      return
    end

    local.get $routine_symbol_index
    i32.const 16
    i32.mul ;; Address of npc_routine symbol info record
    i32.load $symbol_info offset=8 ;; Function index in wasm table
    call_indirect (type $void_void_func)
  )

  ;; Returns address of (ptr, size) pair with symbol name
  (func $get_name (param $symbol_index i32) (result i32)
    local.get $symbol_index
    i32.const 8
    i32.mul
    i32.const 16384 ;; DaedalusMemoryLayout::ALLOCATOR_MEMORY_SIZE
    i32.add
  )

  ;; Returns pointer to option<s32> with id of symbol with given name,
  ;; None when there is no such symbol. It does not trap for missing symbols,
  ;; a trap would make the component instance unusable for all further calls.
  (func $get_symbol_id (param $symbol_str_ptr i32) (param $symbol_str_len i32) (result i32)
    (local $index i32) (local $tested_symbol_offset i32) (local $mem_start i32)

    ;; Get memory where we will put results
    (call $cabi_realloc (i32.const 0) (i32.const 0) (i32.const 0) (i32.const 0))
    local.set $mem_start

    (loop $symbol_loop (result i32)
      ;; if we iterated over all symbols the symbol does not exist
      local.get $index
      global.get $symbols_num
      i32.ge_u
      if
        ;; None
        local.get $mem_start
        i32.const 0
        i32.store8 $main offset=0 ;; value present or not
        local.get $mem_start
        i32.const 0
        i32.store $main offset=4 ;; symbol id, ignored when not present
        local.get $mem_start
        return
      end

      ;; Get string (ptr, size) offset
      local.get $index
      i32.const 8
      i32.mul
      local.set $tested_symbol_offset

      ;; Check if strings are the same, if they are return index
      local.get $symbol_str_ptr
      local.get $symbol_str_len
      local.get $tested_symbol_offset
      i32.load $main offset=16384 ;; Get ptr
      local.get $tested_symbol_offset
      i32.load $main offset=16388 ;; Get len
      call $compare_strings
      if
        ;; Some
        local.get $mem_start
        i32.const 1
        i32.store8 $main offset=0 ;; value present or not
        local.get $mem_start
        local.get $index
        i32.store $main offset=4 ;; symbol id
        local.get $mem_start
        return
      end

      ;; Increment index
      local.get $index
      i32.const 1
      i32.add
      local.set $index

      ;; Return to start of the loop
      br $symbol_loop
    )
  )

  ;; Calls function of given symbol
  (func $call_function (param $symbol_id i32)
    local.get $symbol_id
    i32.const 16
    i32.mul ;; Address of symbol info record
    i32.load $symbol_info offset=8 ;; Function index in wasm table
    call_indirect (type $void_void_func)
  )

  ;; Calls function of given symbol, discarding its result when it has one.
  ;; Some dialog functions are declared `func int` while the dialog code calls
  ;; them as void, so the indirect call type must match the symbol's flag.
  (func $call_function_discard (param $symbol_id i32)
    (local $record_base i32) (local $func_index i32)

    local.get $symbol_id
    i32.const 16
    i32.mul
    local.set $record_base

    local.get $record_base
    i32.load $symbol_info offset=8 ;; Function index in wasm table
    local.set $func_index

    local.get $record_base
    i32.load8_u $symbol_info offset=2 ;; returns value flag
    if
      local.get $func_index
      call_indirect (type $bool_void_func)
      drop
    else
      local.get $func_index
      call_indirect (type $void_void_func)
    end
  )

  ;; Returns number of all symbols
  (func $get_all_symbols_num (result i32)
    global.get $symbols_num
  )

  ;; Returns number of all string pairs in memory
  (func $get_all_strings_num (result i32)
    global.get $strings_num
  )

  ;; Returns pointer to record symbol-info:
  ;; kind @0 (byte), parent option: present @4 (byte), value @8, function option: present @12 (byte), value @16
  (func $get_symbol_info (param $symbol_id i32) (result i32)
    (local $mem_start i32) (local $parent i32) (local $func_index i32)

    ;; Get memory where we will put results
    (call $cabi_realloc (i32.const 0) (i32.const 0) (i32.const 0) (i32.const 0))
    local.set $mem_start

    ;; kind,
    local.get $mem_start
    local.get $symbol_id
    i32.const 16
    i32.mul
    i32.load8_s $symbol_info offset=0
    i32.store8 $main offset=0

    ;; parent
    local.get $symbol_id
    i32.const 16
    i32.mul
    i32.load $symbol_info offset=12
    local.set $parent

    local.get $mem_start
    local.get $parent
    i32.const -1
    i32.ne ;; value present or not
    i32.store8 $main offset=4

    local.get $mem_start
    local.get $parent
    i32.store $main offset=8 ;; parent symbol id, ignored when not present

    ;; function
    local.get $symbol_id
    i32.const 16
    i32.mul
    i32.load $symbol_info offset=8 ;; Function index in wasm table
    local.set $func_index

    local.get $mem_start
    local.get $func_index
    i32.const 0
    i32.ge_s ;; value present or not, u32::MAX reads back as -1 when there is no function
    i32.store8 $main offset=12

    local.get $mem_start
    local.get $func_index
    i32.store $main offset=16 ;; wasm function table index, ignored when not present

    local.get $mem_start
  )

  ;; Calls instance function for all instance symbols
  (func $initialize_all_instances
    (local $index i32) (local $func_index i32)

    (loop $symbol_loop
      ;; if we iterated over all symbols exit
      local.get $index
      global.get $symbols_num
      i32.ge_u
      if
        return
      end

      ;; Get symbol kind, if it is not instance continue
      local.get $index
      i32.const 16
      i32.mul
      i32.load8_s $symbol_info offset=0
      i32.const 6 ;; SymbolKind::Instance
      i32.ne
      if
        local.get $index
        i32.const 1
        i32.add
        local.set $index
        br $symbol_loop
      end

      local.get $index
      call $set_instance

      ;; Instance bodies can use `self` (e.g. npc instances pass it to helper
      ;; functions), bind it like the original engine does
      local.get $index
      call $set_self

      local.get $index
      i32.const 16
      i32.mul ;; Address of instance symbol info record
      i32.load $symbol_info offset=8 ;; Function index in wasm table

      ;; If instance doesn't have code, func_index < 0
      local.tee $func_index
      i32.const 0
      i32.le_s
      if
        local.get $index
        i32.const 1
        i32.add
        local.set $index
        br $symbol_loop
      end

      ;; Call function
      local.get $func_index
      call_indirect (type $void_void_func)

      local.get $index
      i32.const 1
      i32.add
      local.set $index

      ;; Return to start of the loop
      br $symbol_loop
    )
  )



  ;; Returns class member variable at offset
  (func $get_class_member (param $symbol_id i32) (param $member_offset i32) (result i32)
    ;; Get field value
    local.get $symbol_id
    i32.const 16
    i32.mul
    i32.load $symbol_info offset=4 ;; Instance address
    local.get $member_offset
    i32.add
    i32.load $symbols offset=0
  )


  ;; Returns pointer to option<string> with value of class member string
  (func $get_class_member_string (param $symbol_id i32) (param $member_offset i32) (result i32)
    (local $mem_start i32) (local $field_value i32) (local $str_ptr_size_addr i32)

    ;; Get memory where we will put results
    (call $cabi_realloc (i32.const 0) (i32.const 0) (i32.const 0) (i32.const 0))
    local.set $mem_start

    ;; Get field value
    (call $get_class_member (local.get $symbol_id) (local.get $member_offset))
    local.tee $field_value

    ;; If symbol is 0 return None
    i32.const 0
    i32.eq
    if
      local.get $mem_start
      i32.const 0
      i32.store $main offset=0 ;; value present or not
      local.get $mem_start
      i32.const 0
      i32.store $main offset=4 ;; string ptr
      local.get $mem_start
      i32.const 0
      i32.store $main offset=8 ;; string size
      local.get $mem_start
      return
    end

    ;; Value is present
    local.get $mem_start
    i32.const 1
    i32.store $main offset=0 ;; value present or not

    ;; There are already (ptr, size) pairs in memory for symbols, copy them
    local.get $field_value
    i32.const 8
    i32.mul
    i32.const 16384 ;; DaedalusMemoryLayout::ALLOCATOR_MEMORY_SIZE
    i32.add
    local.set $str_ptr_size_addr ;; Address of ptr,size tuple

    ;; Where to store later
    local.get $mem_start
    local.get $mem_start

    local.get $str_ptr_size_addr
    i32.load $main offset=0 ;; Get ptr
    i32.store $main offset=4 ;; Store ptr

    local.get $str_ptr_size_addr
    i32.load $main offset=4 ;; Get size
    i32.store $main offset=8 ;; Store size

    ;; Return address
    local.get $mem_start
  )

  ;; Returns pointer to option<string> with value of item visual member variable
  (func $get_item_visual (param $symbol_id i32) (result i32)
    (call $get_class_member_string (local.get $symbol_id) (i32.const 524))
  )

  ;; Returns pointer to option<string> with value of item visual change member variable
  (func $get_item_visual_change (param $symbol_id i32) (result i32)
    (call $get_class_member_string (local.get $symbol_id) (i32.const 544))
  )

  ;; Returns pointer to an npc-info record with the hitpoints, hitpoints max
  ;; and name of an npc instance (C_NPC attribute ATR_HITPOINTS and
  ;; ATR_HITPOINTS_MAX member offsets, C_NPC name member offset). The name is
  ;; the raw daedalus string id, None when the member value is not set (0).
  (func $get_npc_info (param $symbol_id i32) (result i32)
    (local $mem_start i32) (local $name_value i32)

    (call $cabi_realloc (i32.const 0) (i32.const 0) (i32.const 0) (i32.const 0))
    local.set $mem_start

    ;; hitpoints @0
    local.get $mem_start
    (call $get_class_member (local.get $symbol_id) (i32.const 440))
    i32.store $main offset=0

    ;; hitpoints-max @4
    local.get $mem_start
    (call $get_class_member (local.get $symbol_id) (i32.const 444))
    i32.store $main offset=4

    ;; name @8: option<s32> lowers to a discriminant byte @8 and the value @12
    ;; C_NPC name member offset, member value 0 means the string is not set
    (call $get_class_member (local.get $symbol_id) (i32.const 292))
    local.set $name_value

    local.get $mem_start
    local.get $name_value
    i32.const 0
    i32.ne ;; normalize bool to 0/1
    i32.store8 $main offset=8

    local.get $mem_start
    local.get $name_value
    i32.store $main offset=12

    local.get $mem_start
  )

  ;; Returns pointer to an item-info record with the stats of an item instance
  ;; (C_ITEM member offsets). String members are raw daedalus string ids, None
  ;; when the member value is not set (0).
  (func $get_item_info (param $symbol_id i32) (result i32)
    (local $mem_start i32) (local $name_value i32) (local $buffer i32)

    (call $cabi_realloc (i32.const 0) (i32.const 0) (i32.const 0) (i32.const 60))
    local.set $mem_start

    ;; damage-total @0 (C_ITEM damagetotal)
    local.get $mem_start
    (call $get_class_member (local.get $symbol_id) (i32.const 360))
    i32.store $main offset=0

    ;; name @4: option<s32> lowers to a discriminant byte @4 and the value @8
    ;; C_ITEM name member offset, member value 0 means the string is not set
    (call $get_class_member (local.get $symbol_id) (i32.const 292))
    local.set $name_value

    local.get $mem_start
    local.get $name_value
    i32.const 0
    i32.ne ;; normalize bool to 0/1
    i32.store8 $main offset=4

    local.get $mem_start
    local.get $name_value
    i32.store $main offset=8

    ;; value @12 (C_ITEM value)
    local.get $mem_start
    (call $get_class_member (local.get $symbol_id) (i32.const 352))
    i32.store $main offset=12

    ;; weight @16 (C_ITEM weight)
    local.get $mem_start
    (call $get_class_member (local.get $symbol_id) (i32.const 348))
    i32.store $main offset=16

    ;; cond-attribute @20: list of 3 ints (ptr @20, len @24) from
    ;; C_ITEM cond_atr
    (call $cabi_realloc (i32.const 0) (i32.const 0) (i32.const 0) (i32.const 12))
    local.set $buffer
    local.get $buffer
    (call $get_class_member (local.get $symbol_id) (i32.const 448))
    i32.store $main
    local.get $buffer
    (call $get_class_member (local.get $symbol_id) (i32.const 452))
    i32.store $main offset=4
    local.get $buffer
    (call $get_class_member (local.get $symbol_id) (i32.const 456))
    i32.store $main offset=8

    local.get $mem_start
    local.get $buffer
    i32.store $main offset=20
    local.get $mem_start
    (i32.const 3)
    i32.store $main offset=24

    ;; cond-value @28: list of 3 ints (ptr @28, len @32) from C_ITEM cond_value
    (call $cabi_realloc (i32.const 0) (i32.const 0) (i32.const 0) (i32.const 12))
    local.set $buffer
    local.get $buffer
    (call $get_class_member (local.get $symbol_id) (i32.const 460))
    i32.store $main
    local.get $buffer
    (call $get_class_member (local.get $symbol_id) (i32.const 464))
    i32.store $main offset=4
    local.get $buffer
    (call $get_class_member (local.get $symbol_id) (i32.const 468))
    i32.store $main offset=8

    local.get $mem_start
    local.get $buffer
    i32.store $main offset=28
    local.get $mem_start
    (i32.const 3)
    i32.store $main offset=32

    ;; description @36: option<s32> lowers to a discriminant byte @36 and the
    ;; value @40, C_ITEM description member offset
    (call $get_class_member (local.get $symbol_id) (i32.const 628))
    local.set $name_value

    local.get $mem_start
    local.get $name_value
    i32.const 0
    i32.ne
    i32.store8 $main offset=36

    local.get $mem_start
    local.get $name_value
    i32.store $main offset=40

    ;; text @44: list of 6 ints (ptr @44, len @48) from C_ITEM text
    (call $cabi_realloc (i32.const 0) (i32.const 0) (i32.const 0) (i32.const 24))
    local.set $buffer
    local.get $buffer
    (call $get_class_member (local.get $symbol_id) (i32.const 648))
    i32.store $main
    local.get $buffer
    (call $get_class_member (local.get $symbol_id) (i32.const 652))
    i32.store $main offset=4
    local.get $buffer
    (call $get_class_member (local.get $symbol_id) (i32.const 656))
    i32.store $main offset=8
    local.get $buffer
    (call $get_class_member (local.get $symbol_id) (i32.const 660))
    i32.store $main offset=12
    local.get $buffer
    (call $get_class_member (local.get $symbol_id) (i32.const 664))
    i32.store $main offset=16
    local.get $buffer
    (call $get_class_member (local.get $symbol_id) (i32.const 668))
    i32.store $main offset=20

    local.get $mem_start
    local.get $buffer
    i32.store $main offset=44
    local.get $mem_start
    (i32.const 6)
    i32.store $main offset=48

    ;; count @52: list of 6 ints (ptr @52, len @56) from C_ITEM count
    (call $cabi_realloc (i32.const 0) (i32.const 0) (i32.const 0) (i32.const 24))
    local.set $buffer
    local.get $buffer
    (call $get_class_member (local.get $symbol_id) (i32.const 768))
    i32.store $main
    local.get $buffer
    (call $get_class_member (local.get $symbol_id) (i32.const 772))
    i32.store $main offset=4
    local.get $buffer
    (call $get_class_member (local.get $symbol_id) (i32.const 776))
    i32.store $main offset=8
    local.get $buffer
    (call $get_class_member (local.get $symbol_id) (i32.const 780))
    i32.store $main offset=12
    local.get $buffer
    (call $get_class_member (local.get $symbol_id) (i32.const 784))
    i32.store $main offset=16
    local.get $buffer
    (call $get_class_member (local.get $symbol_id) (i32.const 788))
    i32.store $main offset=20

    local.get $mem_start
    local.get $buffer
    i32.store $main offset=52
    local.get $mem_start
    (i32.const 6)
    i32.store $main offset=56

    local.get $mem_start
  )

  ;; Return information about dialog
  (func $get_dialog_info (param $symbol_id i32) (result i32)
    (local $mem_start i32)

    ;; Get memory where we will put results
    (call $cabi_realloc (i32.const 0) (i32.const 0) (i32.const 0) (i32.const 0))
    local.set $mem_start

    ;; Construct WIT record dialog-info in $mem_start:
    ;; npc @0, condition @4, information @8, important @12 (byte), permanent @13 (byte)

    ;; npc
    local.get $mem_start
    (call $get_class_member (local.get $symbol_id) (i32.const 0))
    i32.store $main offset=0

    ;; order
    local.get $mem_start
    (call $get_class_member (local.get $symbol_id) (i32.const 4))
    i32.store $main offset=4

    ;; condition
    local.get $mem_start
    (call $get_class_member (local.get $symbol_id) (i32.const 12))
    i32.store $main offset=8

    ;; information
    local.get $mem_start
    (call $get_class_member (local.get $symbol_id) (i32.const 16))
    i32.store $main offset=12

    ;; description
    local.get $mem_start
    (call $get_class_member (local.get $symbol_id) (i32.const 20))
    i32.store $main offset=16

    ;; important
    local.get $mem_start
    (call $get_class_member (local.get $symbol_id) (i32.const 8))
    i32.const 0
    i32.ne ;; normalize bool to 0/1
    i32.store8 $main offset=20

    ;; permanent
    local.get $mem_start
    (call $get_class_member (local.get $symbol_id) (i32.const 44))
    i32.const 0
    i32.ne ;; normalize bool to 0/1
    i32.store8 $main offset=21

    local.get $mem_start
  )

  ;; Calls function of given symbol, its i32 result is normalized to a bool (0 or 1)
  (func $call_function_bool (param $symbol_id i32) (result i32)
    local.get $symbol_id
    i32.const 16
    i32.mul ;; Address of symbol info record
    i32.load $symbol_info offset=8 ;; Function index in wasm table
    call_indirect (type $bool_void_func)
    i32.const 0
    i32.ne ;; normalize bool to 0/1
  )

  ;; Export section
  (export "cabi_realloc" (func $cabi_realloc))
  (export "mmcomando:zengine-scripts/manage@0.1.0#set-instance" (func $set_instance))
  (export "mmcomando:zengine-scripts/manage@0.1.0#set-self" (func $set_self))
  (export "mmcomando:zengine-scripts/manage@0.1.0#set-other" (func $set_other))
  (export "mmcomando:zengine-scripts/manage@0.1.0#set-victim" (func $set_victim))
  (export "mmcomando:zengine-scripts/manage@0.1.0#set-item" (func $set_item))
  (export "mmcomando:zengine-scripts/manage@0.1.0#set-hero" (func $set_hero))
  (export "mmcomando:zengine-scripts/manage@0.1.0#call-npc-routine" (func $call_npc_routine))
  (export "mmcomando:zengine-scripts/manage@0.1.0#get-name" (func $get_name))
  (export "mmcomando:zengine-scripts/manage@0.1.0#get-symbol-id" (func $get_symbol_id))
  (export "mmcomando:zengine-scripts/manage@0.1.0#call-function" (func $call_function))
  (export "mmcomando:zengine-scripts/manage@0.1.0#call-function-discard" (func $call_function_discard))
  (export "mmcomando:zengine-scripts/manage@0.1.0#call-function-bool" (func $call_function_bool))
  (export "mmcomando:zengine-scripts/manage@0.1.0#get-all-symbols-num" (func $get_all_symbols_num))
  (export "mmcomando:zengine-scripts/manage@0.1.0#get-all-strings-num" (func $get_all_strings_num))
  (export "mmcomando:zengine-scripts/manage@0.1.0#get-symbol-info" (func $get_symbol_info))
  (export "mmcomando:zengine-scripts/manage@0.1.0#initialize-all-instances" (func $initialize_all_instances))
  (export "mmcomando:zengine-scripts/manage@0.1.0#get-item-visual" (func $get_item_visual))
  (export "mmcomando:zengine-scripts/manage@0.1.0#get-item-visual-change" (func $get_item_visual_change))
  (export "mmcomando:zengine-scripts/manage@0.1.0#get-npc-info" (func $get_npc_info))
  (export "mmcomando:zengine-scripts/manage@0.1.0#get-item-info" (func $get_item_info))
  (export "mmcomando:zengine-scripts/manage@0.1.0#get-dialog-info" (func $get_dialog_info))
  (export "memory" (memory $main))
)
