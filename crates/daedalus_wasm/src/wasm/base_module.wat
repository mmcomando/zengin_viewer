(module
  ;; Type used by call_indirect for functions with no parameters and no results
  (type $void_void_func (func))
  ;; Type used by call_indirect for functions with no parameters returning i32 (treated as bool)
  (type $bool_void_func (func (result i32)))

  ;; Memories, indices are significant, they match constants in DaedalusMemoryLayout (memory.rs):
  ;; 0 - main memory: allocator region, (ptr, size) pairs of symbol names and static strings,
  ;;     their bytes, then the DYN_STRINGS area (64 pair slots + 8192 byte pool)
  ;; 1 - symbol info: 16-byte record per symbol:
  ;;     kind @0 (byte), is class member @1 (byte), function returns value @2 (byte),
  ;;     is const @3 (byte), address or offset @4, wasm function table index @8,
  ;;     parent @12 (u32::MAX when symbol has no parent)
  ;; 2 - symbol default values, only read by `get-symbol-values` (runtime values
  ;;     live in the host symbol store)
  ;; 3 - one u32 element count per symbol, 0 for class members and consts
  ;; Minimal sizes are placeholders, they are extended with daedalus data during merge.
  (memory $main 1)
  (memory $symbol_info 1)
  (memory $symbol_defaults 1)
  (memory $symbol_counts 1)

  ;; Function table with with daedalus functions, used so we can dynamically call any daedalus function from wasm
  (table $funcs 0 funcref)

  ;; Globals, indices are significant, they match constants in wasm.rs
  (global $current_instance_symbol_index (mut i32) (i32.const 0))
  ;; Bump offset of the canonical ABI allocator, starts at 16 so address 0 is
  ;; never returned to the host
  (global $allocator (mut i32) (i32.const 16))
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
  ;; Base of the DYN_STRINGS pool (8192 bytes appended after the static string
  ;; bytes), patched with real value during merge with daedalus data
  (global $dyn_pool_base (mut i32) (i32.const 0))
  ;; Number of dynamic strings created so far, never reset, ring slot is
  ;; `dyn_strings_next_slot % 64`
  (global $dyn_strings_next_slot (mut i32) (i32.const 0))
  ;; Bump offset into the DYN_STRINGS pool, wraps to 0 when the next string
  ;; would not fit
  (global $dyn_pool_offset (mut i32) (i32.const 0))
  ;; Size in bytes of the symbol value word space, patched during merge with
  ;; daedalus data, read by `get-symbols-memory-size`
  (global $symbols_memory_size (mut i32) (i32.const 0))
  ;; Instance id of the well known `self` variable, cached so
  ;; `SetInstance(self)` does not need a host call. Updated by the `set_self`
  ;; export. Other scope variables read their value from the host store.
  (global $self_instance (mut i32) (i32.const -1))


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

  ;; Memory allocator for the canonical ABI (host-returned strings, export
  ;; result areas) and for guest builtin result records. Wrapping bump
  ;; allocator over the allocator region at the start of main memory, the
  ;; region size matches `DaedalusMemoryLayout::ALLOCATOR_MEMORY_SIZE`
  ;; (4096 * 4). Allocations are scratch: callers consume the data before the
  ;; next allocation, so wrapping over old data is safe. Does not support
  ;; allocations larger than the region.
  (func $cabi_realloc (param $old_ptr i32) (param $old_size i32) (param $align i32) (param $new_size i32) (result i32)
    (local $offset i32)

    ;; Internal callers pass 0 as alignment, their records hold i32 values so
    ;; 4 is used, which also satisfies every host lift alignment
    local.get $align
    i32.const 4
    i32.gt_u
    if
      ;; keep host-requested alignment, it is already >= 4
    else
      i32.const 4
      local.set $align
    end

    ;; Round the bump offset up to the requested alignment, alignment is
    ;; always a power of two, so `& -align` masks the low bits. Note the
    ;; operand order: wasm binary ops pop the right operand from the top, so
    ;; the negation is `0 - align`.
    global.get $allocator
    local.get $align
    i32.add
    i32.const 1
    i32.sub
    i32.const 0
    local.get $align
    i32.sub
    i32.and
    local.set $offset

    ;; Wrap to the region start when the allocation would not fit. Callers
    ;; may write more than they requested (internal zero-sized scratch records
    ;; hold up to ~60 bytes), so keep 128 bytes of slack like the old block
    ;; allocator did, keeping every write inside the allocator region.
    local.get $offset
    local.get $new_size
    i32.add
    i32.const 16256
    i32.gt_u
    if
      i32.const 16
      local.set $offset
    end

    ;; Advance the bump offset, zero-sized allocations still advance so
    ;; successive scratch records do not overlap
    local.get $offset
    local.get $new_size
    i32.add
    local.get $new_size
    i32.eqz
    i32.add
    global.set $allocator

    ;; Return address
    local.get $offset
  )


  ;; Sets the current instance context (symbol index) used for class member
  ;; access. This is what the daedalus `SetInstance` instruction means,
  ;; converted functions call this builtin. If index of this function changes
  ;; change BASE_FUNC_SET_INSTANCE
  (func $set_instance (param $symbol_id i32)
    local.get $symbol_id
    global.set $current_instance_symbol_index
  )

  ;; Binds the well known `self` variable to the given instance. The symbol
  ;; value itself lives in the host store, the host writes it before calling
  ;; this export; here only the cached instance id used by `SetInstance(self)`
  ;; is updated.
  (func $set_self (param $instance_id i32)
    local.get $instance_id
    global.set $self_instance
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
  ;; kind @0 (byte), parent option: present @4 (byte), value @8,
  ;; function option: present @12 (byte), value @16, is-const @20 (byte),
  ;; is-class-member @21 (byte), address-or-offset @24
  (func $get_symbol_info (param $symbol_id i32) (result i32)
    (local $mem_start i32) (local $parent i32) (local $func_index i32)

    ;; Get memory where we will put results
    (call $cabi_realloc (i32.const 0) (i32.const 0) (i32.const 0) (i32.const 28))
    local.set $mem_start

    ;; kind,
    local.get $mem_start
    local.get $symbol_id
    i32.const 16
    i32.mul
    i32.load8_s $symbol_info offset=0
    i32.store8 $main offset=0

    ;; is-const
    local.get $mem_start
    local.get $symbol_id
    i32.const 16
    i32.mul
    i32.load8_u $symbol_info offset=3
    i32.store8 $main offset=20

    ;; is-class-member
    local.get $mem_start
    local.get $symbol_id
    i32.const 16
    i32.mul
    i32.load8_u $symbol_info offset=1
    i32.store8 $main offset=21

    ;; address-or-offset
    local.get $mem_start
    local.get $symbol_id
    i32.const 16
    i32.mul
    i32.load $symbol_info offset=4
    i32.store $main offset=24

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

  ;; Returns pointer to a (ptr, len) pair with the initial values of a symbol
  ;; value slot. Runtime values live in the host store, this copies from the
  ;; compile-time defaults memory and is meant to be called before instance
  ;; initialization runs. Class members and consts have no slot, they return an
  ;; empty list.
  (func $get_symbol_values (param $symbol_id i32) (result i32)
    (local $count i32) (local $address i32) (local $buffer i32) (local $pair i32)

    ;; element count, 0 when the symbol has no value slot
    local.get $symbol_id
    i32.const 4
    i32.mul
    i32.load $symbol_counts offset=0
    local.set $count

    ;; Element buffer is allocated before the returned pair, so an allocator
    ;; wrap cannot invalidate the pair before the host lifts it.
    i32.const 0
    i32.const 0
    i32.const 0
    local.get $count
    i32.const 4
    i32.mul
    call $cabi_realloc
    local.set $buffer

    ;; Copy the defaults into the buffer
    local.get $symbol_id
    i32.const 16
    i32.mul
    i32.load $symbol_info offset=4 ;; Address of the symbol value slot
    local.set $address

    local.get $buffer
    local.get $address
    local.get $count
    i32.const 4
    i32.mul
    memory.copy $main $symbol_defaults

    ;; Returned (ptr, len) pair
    i32.const 0
    i32.const 0
    i32.const 0
    i32.const 8
    call $cabi_realloc
    local.set $pair

    local.get $pair
    local.get $buffer
    i32.store $main offset=0

    local.get $pair
    local.get $count
    i32.store $main offset=4

    local.get $pair
  )

  ;; Returns size in bytes of the symbol value word space
  (func $get_symbols_memory_size (result i32)
    global.get $symbols_memory_size
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

  ;; Copies a host-returned string into the DYN_STRINGS ring and returns its
  ;; string id. The ring holds 64 pair slots appended after the static string
  ;; pairs (ids `strings_num .. strings_num+63`), contents live in a shared
  ;; 8192 byte pool with a wrapping bump offset, so there is no per-string max
  ;; length. A dynamic string stays valid only until the slot or the pool
  ;; cursor overwrites it, mirroring the original engine which reuses a single
  ;; temp symbol for all external string results.
  (func $copy_dynamic_string (param $ptr i32) (param $len i32) (result i32)
    (local $slot i32) (local $dest i32) (local $pair_addr i32)

    ;; Truncate strings longer than the whole pool
    local.get $len
    i32.const 8192
    i32.gt_u
    if
      i32.const 8192
      local.set $len
    end

    ;; Wrap the pool bump offset when the string would not fit
    global.get $dyn_pool_offset
    local.get $len
    i32.add
    i32.const 8192
    i32.gt_u
    if
      i32.const 0
      global.set $dyn_pool_offset
    end

    ;; Ring slot, wraps after 64 dynamic strings
    global.get $dyn_strings_next_slot
    i32.const 64
    i32.rem_u
    local.set $slot

    ;; Destination in the pool
    global.get $dyn_pool_base
    global.get $dyn_pool_offset
    i32.add
    local.set $dest

    ;; Copy the string bytes
    local.get $dest
    local.get $ptr
    local.get $len
    memory.copy $main $main

    ;; Write the (ptr, len) pair into the ring slot, pairs start right after
    ;; the allocator region (16384 bytes)
    global.get $strings_num
    local.get $slot
    i32.add
    i32.const 8
    i32.mul
    i32.const 16384
    i32.add
    local.set $pair_addr

    local.get $pair_addr
    local.get $dest
    i32.store $main offset=0
    local.get $pair_addr
    local.get $len
    i32.store $main offset=4

    ;; Bump both cursors
    global.get $dyn_strings_next_slot
    i32.const 1
    i32.add
    global.set $dyn_strings_next_slot

    global.get $dyn_pool_offset
    local.get $len
    i32.add
    global.set $dyn_pool_offset

    ;; Return the string id of the ring slot
    global.get $strings_num
    local.get $slot
    i32.add
  )

  ;; Export section
  (export "cabi_realloc" (func $cabi_realloc))
  (export "mmcomando:zengine-scripts/manage@0.1.0#set-instance" (func $set_instance))
  (export "mmcomando:zengine-scripts/manage@0.1.0#set-self" (func $set_self))
  (export "mmcomando:zengine-scripts/manage@0.1.0#get-name" (func $get_name))
  (export "mmcomando:zengine-scripts/manage@0.1.0#get-symbol-id" (func $get_symbol_id))
  (export "mmcomando:zengine-scripts/manage@0.1.0#call-function" (func $call_function))
  (export "mmcomando:zengine-scripts/manage@0.1.0#call-function-discard" (func $call_function_discard))
  (export "mmcomando:zengine-scripts/manage@0.1.0#call-function-bool" (func $call_function_bool))
  (export "mmcomando:zengine-scripts/manage@0.1.0#get-all-symbols-num" (func $get_all_symbols_num))
  (export "mmcomando:zengine-scripts/manage@0.1.0#get-all-strings-num" (func $get_all_strings_num))
  (export "mmcomando:zengine-scripts/manage@0.1.0#get-symbol-info" (func $get_symbol_info))
  (export "mmcomando:zengine-scripts/manage@0.1.0#get-symbol-values" (func $get_symbol_values))
  (export "mmcomando:zengine-scripts/manage@0.1.0#get-symbols-memory-size" (func $get_symbols_memory_size))
  (export "memory" (memory $main))
)
