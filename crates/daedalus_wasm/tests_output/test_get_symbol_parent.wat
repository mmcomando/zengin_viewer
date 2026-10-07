(component
  (core module $main (;0;)
    (type (;0;) (func))
    (type (;1;) (func (param i32 i32 i32 i32) (result i32)))
    (type (;2;) (func (param i32) (result i32)))
    (type (;3;) (func (param i32)))
    (type (;4;) (func (param i32 i32) (result i32)))
    (type (;5;) (func (result i32)))
    (type (;6;) (func (result i32)))
    (table (;0;) 1 funcref)
    (memory (;0;) 1)
    (memory (;1;) 1)
    (memory (;2;) 1)
    (global (;0;) (mut i32) i32.const 0)
    (global (;1;) (mut i32) i32.const 0)
    (global (;2;) (mut i32) i32.const 0)
    (global (;3;) (mut i32) i32.const 3)
    (export "cabi_realloc" (func 1))
    (export "mmcomando:zengine-scripts/manage@0.1.0#set-instance" (func 3))
    (export "mmcomando:zengine-scripts/manage@0.1.0#call-npc-routine" (func 4))
    (export "mmcomando:zengine-scripts/manage@0.1.0#get-name" (func 5))
    (export "mmcomando:zengine-scripts/manage@0.1.0#get-symbol-id" (func 6))
    (export "mmcomando:zengine-scripts/manage@0.1.0#call-function" (func 7))
    (export "mmcomando:zengine-scripts/manage@0.1.0#get-all-symbols-num" (func 8))
    (export "mmcomando:zengine-scripts/manage@0.1.0#get-symbol-kind" (func 9))
    (export "mmcomando:zengine-scripts/manage@0.1.0#get-symbol-parent" (func 10))
    (export "mmcomando:zengine-scripts/manage@0.1.0#get-symbol-info" (func 11))
    (export "mmcomando:zengine-scripts/manage@0.1.0#initialize-all-instances" (func 12))
    (export "mmcomando:zengine-scripts/manage@0.1.0#get-item-visual" (func 15))
    (export "mmcomando:zengine-scripts/manage@0.1.0#get-item-visual-change" (func 16))
    (export "mmcomando:zengine-scripts/manage@0.1.0#get-dialog-info" (func 17))
    (export "memory" (memory 0))
    (export "mmcomando:zengine-scripts/manage@0.1.0#test-func" (func 18))
    (elem (;0;) (table 0) (i32.const 0) func 18)
    (func (;0;) (type 1) (param i32 i32 i32 i32) (result i32)
      (local i32 i32 i32)
      local.get 1
      ;; [i32]
      local.get 3
      ;; [i32 i32]
      i32.ne
      ;; [i32]
      if ;; label = @1
        ;; []
        i32.const 0
        ;; [i32]
        return
        ;; []
      end
      ;; []
      loop (result i32) ;; label = @1
        ;; []
        local.get 4
        ;; [i32]
        local.get 1
        ;; [i32 i32]
        i32.ge_u
        ;; [i32]
        if ;; label = @2
          ;; []
          i32.const 1
          ;; [i32]
          return
          ;; []
        end
        ;; []
        local.get 4
        ;; [i32]
        local.get 0
        ;; [i32 i32]
        i32.add
        ;; [i32]
        i32.load8_u
        ;; [i32]
        local.set 5
        ;; []
        local.get 4
        ;; [i32]
        local.get 2
        ;; [i32 i32]
        i32.add
        ;; [i32]
        i32.load8_u
        ;; [i32]
        local.set 6
        ;; []
        local.get 5
        ;; [i32]
        local.get 6
        ;; [i32 i32]
        i32.ne
        ;; [i32]
        if ;; label = @2
          ;; []
          i32.const 0
          ;; [i32]
          return
          ;; []
        end
        ;; []
        local.get 4
        ;; [i32]
        i32.const 1
        ;; [i32 i32]
        i32.add
        ;; [i32]
        local.set 4
        ;; []
        br 0 (;@1;)
        ;; []
      end
      ;; [i32]
    )
    (func (;1;) (type 1) (param i32 i32 i32 i32) (result i32)
      global.get 2
      ;; [i32]
      i32.const 1
      ;; [i32 i32]
      i32.add
      ;; [i32]
      i32.const 127
      ;; [i32 i32]
      i32.and
      ;; [i32]
      global.set 2
      ;; []
      global.get 2
      ;; [i32]
      i32.const 128
      ;; [i32 i32]
      i32.mul
      ;; [i32]
    )
    (func (;2;) (type 2) (param i32) (result i32)
      (local i32)
      local.get 0
      ;; [i32]
      i32.const 16
      ;; [i32 i32]
      i32.mul
      ;; [i32]
      local.set 1
      ;; []
      local.get 1
      ;; [i32]
      i32.load8_u 1 offset=1
      ;; [i32]
      global.get 0
      ;; [i32 i32]
      i32.mul
      ;; [i32]
      local.get 1
      ;; [i32 i32]
      i32.load 1 offset=4
      ;; [i32 i32]
      i32.add
      ;; [i32]
    )
    (func (;3;) (type 3) (param i32)
      local.get 0
      ;; [i32]
      global.set 1
      ;; []
      local.get 0
      ;; [i32]
      call 2
      ;; [i32]
      global.set 0
      ;; []
    )
    (func (;4;) (type 3) (param i32)
      (local i32)
      local.get 0
      ;; [i32]
      i32.const 608
      ;; [i32 i32]
      call 13
      ;; [i32]
      local.tee 1
      ;; [i32]
      i32.const 1
      ;; [i32 i32]
      i32.le_s
      ;; [i32]
      if ;; label = @1
        ;; []
        return
        ;; []
      end
      ;; []
      local.get 1
      ;; [i32]
      i32.const 16
      ;; [i32 i32]
      i32.mul
      ;; [i32]
      i32.load 1 offset=8
      ;; [i32]
      call_indirect (type 0)
      ;; []
    )
    (func (;5;) (type 2) (param i32) (result i32)
      local.get 0
      ;; [i32]
      i32.const 8
      ;; [i32 i32]
      i32.mul
      ;; [i32]
      i32.const 16384
      ;; [i32 i32]
      i32.add
      ;; [i32]
    )
    (func (;6;) (type 4) (param i32 i32) (result i32)
      (local i32 i32)
      loop (result i32) ;; label = @1
        ;; []
        local.get 2
        ;; [i32]
        global.get 3
        ;; [i32 i32]
        i32.ge_u
        ;; [i32]
        if ;; label = @2
          ;; []
          unreachable
          ;; []
        end
        ;; []
        local.get 2
        ;; [i32]
        i32.const 8
        ;; [i32 i32]
        i32.mul
        ;; [i32]
        local.set 3
        ;; []
        local.get 0
        ;; [i32]
        local.get 1
        ;; [i32 i32]
        local.get 3
        ;; [i32 i32 i32]
        i32.load offset=16384
        ;; [i32 i32 i32]
        local.get 3
        ;; [i32 i32 i32 i32]
        i32.load offset=16388
        ;; [i32 i32 i32 i32]
        call 0
        ;; [i32]
        if ;; label = @2
          ;; []
          local.get 2
          ;; [i32]
          return
          ;; []
        end
        ;; []
        local.get 2
        ;; [i32]
        i32.const 1
        ;; [i32 i32]
        i32.add
        ;; [i32]
        local.set 2
        ;; []
        br 0 (;@1;)
        ;; []
      end
      ;; [i32]
    )
    (func (;7;) (type 3) (param i32)
      local.get 0
      ;; [i32]
      i32.const 16
      ;; [i32 i32]
      i32.mul
      ;; [i32]
      i32.load 1 offset=8
      ;; [i32]
      call_indirect (type 0)
      ;; []
    )
    (func (;8;) (type 5) (result i32)
      global.get 3
      ;; [i32]
    )
    (func (;9;) (type 2) (param i32) (result i32)
      local.get 0
      ;; [i32]
      i32.const 16
      ;; [i32 i32]
      i32.mul
      ;; [i32]
      i32.load8_s 1
      ;; [i32]
    )
    (func (;10;) (type 2) (param i32) (result i32)
      local.get 0
      ;; [i32]
      i32.const 16
      ;; [i32 i32]
      i32.mul
      ;; [i32]
      i32.load 1 offset=12
      ;; [i32]
    )
    (func (;11;) (type 2) (param i32) (result i32)
      (local i32 i32 i32)
      i32.const 0
      ;; [i32]
      i32.const 0
      ;; [i32 i32]
      i32.const 0
      ;; [i32 i32 i32]
      i32.const 0
      ;; [i32 i32 i32 i32]
      call 1
      ;; [i32]
      local.set 1
      ;; []
      local.get 1
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 16
      ;; [i32 i32 i32]
      i32.mul
      ;; [i32 i32]
      i32.load8_s 1
      ;; [i32 i32]
      i32.store8
      ;; []
      local.get 0
      ;; [i32]
      i32.const 16
      ;; [i32 i32]
      i32.mul
      ;; [i32]
      i32.load 1 offset=12
      ;; [i32]
      local.set 2
      ;; []
      local.get 1
      ;; [i32]
      local.get 2
      ;; [i32 i32]
      i32.const -1
      ;; [i32 i32 i32]
      i32.ne
      ;; [i32 i32]
      i32.store8 offset=4
      ;; []
      local.get 1
      ;; [i32]
      local.get 2
      ;; [i32 i32]
      i32.store offset=8
      ;; []
      local.get 0
      ;; [i32]
      i32.const 16
      ;; [i32 i32]
      i32.mul
      ;; [i32]
      i32.load 1 offset=8
      ;; [i32]
      local.set 3
      ;; []
      local.get 1
      ;; [i32]
      local.get 3
      ;; [i32 i32]
      i32.const 0
      ;; [i32 i32 i32]
      i32.ge_s
      ;; [i32 i32]
      i32.store8 offset=12
      ;; []
      local.get 1
      ;; [i32]
      local.get 3
      ;; [i32 i32]
      i32.store offset=16
      ;; []
      local.get 1
      ;; [i32]
    )
    (func (;12;) (type 0)
      (local i32 i32)
      loop ;; label = @1
        ;; []
        local.get 0
        ;; [i32]
        global.get 3
        ;; [i32 i32]
        i32.ge_u
        ;; [i32]
        if ;; label = @2
          ;; []
          return
          ;; []
        end
        ;; []
        local.get 0
        ;; [i32]
        i32.const 16
        ;; [i32 i32]
        i32.mul
        ;; [i32]
        i32.load8_s 1
        ;; [i32]
        i32.const 6
        ;; [i32 i32]
        i32.ne
        ;; [i32]
        if ;; label = @2
          ;; []
          local.get 0
          ;; [i32]
          i32.const 1
          ;; [i32 i32]
          i32.add
          ;; [i32]
          local.set 0
          ;; []
          br 1 (;@1;)
          ;; []
        end
        ;; []
        local.get 0
        ;; [i32]
        call 3
        ;; []
        local.get 0
        ;; [i32]
        i32.const 16
        ;; [i32 i32]
        i32.mul
        ;; [i32]
        i32.load 1 offset=8
        ;; [i32]
        local.tee 1
        ;; [i32]
        i32.const 0
        ;; [i32 i32]
        i32.le_s
        ;; [i32]
        if ;; label = @2
          ;; []
          local.get 0
          ;; [i32]
          i32.const 1
          ;; [i32 i32]
          i32.add
          ;; [i32]
          local.set 0
          ;; []
          br 1 (;@1;)
          ;; []
        end
        ;; []
        local.get 1
        ;; [i32]
        call_indirect (type 0)
        ;; []
        local.get 0
        ;; [i32]
        i32.const 1
        ;; [i32 i32]
        i32.add
        ;; [i32]
        local.set 0
        ;; []
        br 0 (;@1;)
        ;; []
      end
      ;; []
    )
    (func (;13;) (type 4) (param i32 i32) (result i32)
      local.get 0
      ;; [i32]
      i32.const 16
      ;; [i32 i32]
      i32.mul
      ;; [i32]
      i32.load 1 offset=4
      ;; [i32]
      local.get 1
      ;; [i32 i32]
      i32.add
      ;; [i32]
      i32.load 2
      ;; [i32]
    )
    (func (;14;) (type 4) (param i32 i32) (result i32)
      (local i32 i32 i32)
      i32.const 0
      ;; [i32]
      i32.const 0
      ;; [i32 i32]
      i32.const 0
      ;; [i32 i32 i32]
      i32.const 0
      ;; [i32 i32 i32 i32]
      call 1
      ;; [i32]
      local.set 2
      ;; []
      local.get 0
      ;; [i32]
      local.get 1
      ;; [i32 i32]
      call 13
      ;; [i32]
      local.tee 3
      ;; [i32]
      i32.const 0
      ;; [i32 i32]
      i32.eq
      ;; [i32]
      if ;; label = @1
        ;; []
        local.get 2
        ;; [i32]
        i32.const 0
        ;; [i32 i32]
        i32.store
        ;; []
        local.get 2
        ;; [i32]
        i32.const 0
        ;; [i32 i32]
        i32.store offset=4
        ;; []
        local.get 2
        ;; [i32]
        i32.const 0
        ;; [i32 i32]
        i32.store offset=8
        ;; []
        local.get 2
        ;; [i32]
        return
        ;; []
      end
      ;; []
      local.get 2
      ;; [i32]
      i32.const 1
      ;; [i32 i32]
      i32.store
      ;; []
      local.get 3
      ;; [i32]
      i32.const 8
      ;; [i32 i32]
      i32.mul
      ;; [i32]
      i32.const 16384
      ;; [i32 i32]
      i32.add
      ;; [i32]
      local.set 4
      ;; []
      local.get 2
      ;; [i32]
      local.get 2
      ;; [i32 i32]
      local.get 4
      ;; [i32 i32 i32]
      i32.load
      ;; [i32 i32 i32]
      i32.store offset=4
      ;; [i32]
      local.get 4
      ;; [i32 i32]
      i32.load offset=4
      ;; [i32 i32]
      i32.store offset=8
      ;; []
      local.get 2
      ;; [i32]
    )
    (func (;15;) (type 2) (param i32) (result i32)
      local.get 0
      ;; [i32]
      i32.const 524
      ;; [i32 i32]
      call 14
      ;; [i32]
    )
    (func (;16;) (type 2) (param i32) (result i32)
      local.get 0
      ;; [i32]
      i32.const 544
      ;; [i32 i32]
      call 14
      ;; [i32]
    )
    (func (;17;) (type 2) (param i32) (result i32)
      (local i32)
      i32.const 0
      ;; [i32]
      i32.const 0
      ;; [i32 i32]
      i32.const 0
      ;; [i32 i32 i32]
      i32.const 0
      ;; [i32 i32 i32 i32]
      call 1
      ;; [i32]
      local.set 1
      ;; []
      local.get 1
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 0
      ;; [i32 i32 i32]
      call 13
      ;; [i32 i32]
      i32.store
      ;; []
      local.get 1
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 12
      ;; [i32 i32 i32]
      call 13
      ;; [i32 i32]
      i32.store offset=4
      ;; []
      local.get 1
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 16
      ;; [i32 i32 i32]
      call 13
      ;; [i32 i32]
      i32.store offset=8
      ;; []
      local.get 1
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 8
      ;; [i32 i32 i32]
      call 13
      ;; [i32 i32]
      i32.const 0
      ;; [i32 i32 i32]
      i32.ne
      ;; [i32 i32]
      i32.store8 offset=12
      ;; []
      local.get 1
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 44
      ;; [i32 i32 i32]
      call 13
      ;; [i32 i32]
      i32.const 0
      ;; [i32 i32 i32]
      i32.ne
      ;; [i32 i32]
      i32.store8 offset=13
      ;; []
      local.get 1
      ;; [i32]
    )
    (func (;18;) (type 6) (result i32)
      (local i32 i32)
      i32.const 55
      ;; [i32]
      return
      ;; []
    )
    (data (;0;) (i32.const 16384) "\18@\00\00\01\00\00\00\19@\00\00\01\00\00\00\1a@\00\00\01\00\00\00012")
    (data (;1;) (memory 1) (i32.const 0) "\01\00\00\00\00\00\00\00\ff\ff\ff\ff\ff\ff\ff\ff\01\01\00\00\00\00\00\00\ff\ff\ff\ff\00\00\00")
    (data (;2;) (memory 1) (i32.const 32) "\01\00\00\00\04\00\00\00\ff\ff\ff\ff\ff\ff\ff\ff")
    (@producers
      (processed-by "wit-component" "0.251.0")
    )
  )
  (type (;0;) (enum "float" "int" "str" "class" "function" "prototype" "instance" "variable-argument"))
  (type (;1;) (record (field "npc" s32) (field "condition" s32) (field "information" s32) (field "important" bool) (field "permanent" bool)))
  (type (;2;) (option s32))
  (type (;3;) (record (field "kind" 0) (field "parent" 2) (field "function" 2)))
  (core instance $main (;0;) (instantiate $main))
  (alias core export $main "memory" (core memory $memory (;0;)))
  (type (;4;) (func (result s32)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#test-func" (core func $mmcomando:zengine-scripts/manage@0.1.0#test-func (;0;)))
  (alias core export $main "cabi_realloc" (core func $cabi_realloc (;1;)))
  (func $test-func (;0;) (type 4) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#test-func)))
  (type (;5;) (func (param "instance-id" s32)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#set-instance" (core func $mmcomando:zengine-scripts/manage@0.1.0#set-instance (;2;)))
  (func $set-instance (;1;) (type 5) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#set-instance)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#call-npc-routine" (core func $mmcomando:zengine-scripts/manage@0.1.0#call-npc-routine (;3;)))
  (func $call-npc-routine (;2;) (type 5) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#call-npc-routine)))
  (type (;6;) (func (param "symbol-id" s32) (result string)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#get-name" (core func $mmcomando:zengine-scripts/manage@0.1.0#get-name (;4;)))
  (func $get-name (;3;) (type 6) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#get-name) (memory $memory) string-encoding=utf8))
  (type (;7;) (func (param "symbol-id" s32)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#call-function" (core func $mmcomando:zengine-scripts/manage@0.1.0#call-function (;5;)))
  (func $call-function (;4;) (type 7) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#call-function)))
  (type (;8;) (func (param "symbol-name" string) (result s32)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#get-symbol-id" (core func $mmcomando:zengine-scripts/manage@0.1.0#get-symbol-id (;6;)))
  (func $get-symbol-id (;5;) (type 8) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#get-symbol-id) (memory $memory) (realloc $cabi_realloc) string-encoding=utf8))
  (type (;9;) (option string))
  (type (;10;) (func (param "symbol-id" s32) (result 9)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#get-item-visual" (core func $mmcomando:zengine-scripts/manage@0.1.0#get-item-visual (;7;)))
  (func $get-item-visual (;6;) (type 10) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#get-item-visual) (memory $memory) string-encoding=utf8))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#get-item-visual-change" (core func $mmcomando:zengine-scripts/manage@0.1.0#get-item-visual-change (;8;)))
  (func $get-item-visual-change (;7;) (type 10) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#get-item-visual-change) (memory $memory) string-encoding=utf8))
  (type (;11;) (func))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#initialize-all-instances" (core func $mmcomando:zengine-scripts/manage@0.1.0#initialize-all-instances (;9;)))
  (func $initialize-all-instances (;8;) (type 11) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#initialize-all-instances)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#get-all-symbols-num" (core func $mmcomando:zengine-scripts/manage@0.1.0#get-all-symbols-num (;10;)))
  (func $get-all-symbols-num (;9;) (type 4) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#get-all-symbols-num)))
  (type (;12;) (func (param "symbol-id" s32) (result 0)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#get-symbol-kind" (core func $mmcomando:zengine-scripts/manage@0.1.0#get-symbol-kind (;11;)))
  (func $get-symbol-kind (;10;) (type 12) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#get-symbol-kind)))
  (type (;13;) (func (param "symbol-id" s32) (result s32)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#get-symbol-parent" (core func $mmcomando:zengine-scripts/manage@0.1.0#get-symbol-parent (;12;)))
  (func $get-symbol-parent (;11;) (type 13) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#get-symbol-parent)))
  (type (;14;) (func (param "symbol-id" s32) (result 1)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#get-dialog-info" (core func $mmcomando:zengine-scripts/manage@0.1.0#get-dialog-info (;13;)))
  (func $get-dialog-info (;12;) (type 14) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#get-dialog-info) (memory $memory)))
  (type (;15;) (func (param "symbol-id" s32) (result 3)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#get-symbol-info" (core func $mmcomando:zengine-scripts/manage@0.1.0#get-symbol-info (;14;)))
  (func $get-symbol-info (;13;) (type 15) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#get-symbol-info) (memory $memory)))
  (component $mmcomando:zengine-scripts/manage@0.1.0-shim-component (;0;)
    (type (;0;) (func (result s32)))
    (import "import-func-test-func" (func (;0;) (type 0)))
    (type (;1;) (func (param "instance-id" s32)))
    (import "import-func-set-instance" (func (;1;) (type 1)))
    (import "import-func-call-npc-routine" (func (;2;) (type 1)))
    (type (;2;) (func (param "symbol-id" s32) (result string)))
    (import "import-func-get-name" (func (;3;) (type 2)))
    (type (;3;) (func (param "symbol-id" s32)))
    (import "import-func-call-function" (func (;4;) (type 3)))
    (type (;4;) (func (param "symbol-name" string) (result s32)))
    (import "import-func-get-symbol-id" (func (;5;) (type 4)))
    (type (;5;) (option string))
    (type (;6;) (func (param "symbol-id" s32) (result 5)))
    (import "import-func-get-item-visual" (func (;6;) (type 6)))
    (import "import-func-get-item-visual-change" (func (;7;) (type 6)))
    (type (;7;) (func))
    (import "import-func-initialize-all-instances" (func (;8;) (type 7)))
    (import "import-func-get-all-symbols-num" (func (;9;) (type 0)))
    (type (;8;) (enum "float" "int" "str" "class" "function" "prototype" "instance" "variable-argument"))
    (import "import-type-symbol-kind" (type (;9;) (eq 8)))
    (type (;10;) (func (param "symbol-id" s32) (result 9)))
    (import "import-func-get-symbol-kind" (func (;10;) (type 10)))
    (type (;11;) (func (param "symbol-id" s32) (result s32)))
    (import "import-func-get-symbol-parent" (func (;11;) (type 11)))
    (type (;12;) (record (field "npc" s32) (field "condition" s32) (field "information" s32) (field "important" bool) (field "permanent" bool)))
    (import "import-type-dialog-info" (type (;13;) (eq 12)))
    (type (;14;) (func (param "symbol-id" s32) (result 13)))
    (import "import-func-get-dialog-info" (func (;12;) (type 14)))
    (type (;15;) (option s32))
    (type (;16;) (record (field "kind" 9) (field "parent" 15) (field "function" 15)))
    (import "import-type-symbol-info" (type (;17;) (eq 16)))
    (type (;18;) (func (param "symbol-id" s32) (result 17)))
    (import "import-func-get-symbol-info" (func (;13;) (type 18)))
    (type (;19;) (enum "float" "int" "str" "class" "function" "prototype" "instance" "variable-argument"))
    (export (;20;) "symbol-kind" (type 19))
    (type (;21;) (record (field "npc" s32) (field "condition" s32) (field "information" s32) (field "important" bool) (field "permanent" bool)))
    (export (;22;) "dialog-info" (type 21))
    (type (;23;) (option s32))
    (type (;24;) (record (field "kind" 20) (field "parent" 23) (field "function" 23)))
    (export (;25;) "symbol-info" (type 24))
    (type (;26;) (func (result s32)))
    (export (;14;) "test-func" (func 0) (func (type 26)))
    (type (;27;) (func (param "instance-id" s32)))
    (export (;15;) "set-instance" (func 1) (func (type 27)))
    (export (;16;) "call-npc-routine" (func 2) (func (type 27)))
    (type (;28;) (func (param "symbol-id" s32) (result string)))
    (export (;17;) "get-name" (func 3) (func (type 28)))
    (type (;29;) (func (param "symbol-id" s32)))
    (export (;18;) "call-function" (func 4) (func (type 29)))
    (type (;30;) (func (param "symbol-name" string) (result s32)))
    (export (;19;) "get-symbol-id" (func 5) (func (type 30)))
    (type (;31;) (option string))
    (type (;32;) (func (param "symbol-id" s32) (result 31)))
    (export (;20;) "get-item-visual" (func 6) (func (type 32)))
    (export (;21;) "get-item-visual-change" (func 7) (func (type 32)))
    (type (;33;) (func))
    (export (;22;) "initialize-all-instances" (func 8) (func (type 33)))
    (export (;23;) "get-all-symbols-num" (func 9) (func (type 26)))
    (type (;34;) (func (param "symbol-id" s32) (result 20)))
    (export (;24;) "get-symbol-kind" (func 10) (func (type 34)))
    (type (;35;) (func (param "symbol-id" s32) (result s32)))
    (export (;25;) "get-symbol-parent" (func 11) (func (type 35)))
    (type (;36;) (func (param "symbol-id" s32) (result 22)))
    (export (;26;) "get-dialog-info" (func 12) (func (type 36)))
    (type (;37;) (func (param "symbol-id" s32) (result 25)))
    (export (;27;) "get-symbol-info" (func 13) (func (type 37)))
  )
  (instance $mmcomando:zengine-scripts/manage@0.1.0-shim-instance (;0;) (instantiate $mmcomando:zengine-scripts/manage@0.1.0-shim-component
      (with "import-func-test-func" (func $test-func))
      (with "import-func-set-instance" (func $set-instance))
      (with "import-func-call-npc-routine" (func $call-npc-routine))
      (with "import-func-get-name" (func $get-name))
      (with "import-func-call-function" (func $call-function))
      (with "import-func-get-symbol-id" (func $get-symbol-id))
      (with "import-func-get-item-visual" (func $get-item-visual))
      (with "import-func-get-item-visual-change" (func $get-item-visual-change))
      (with "import-func-initialize-all-instances" (func $initialize-all-instances))
      (with "import-func-get-all-symbols-num" (func $get-all-symbols-num))
      (with "import-func-get-symbol-kind" (func $get-symbol-kind))
      (with "import-func-get-symbol-parent" (func $get-symbol-parent))
      (with "import-func-get-dialog-info" (func $get-dialog-info))
      (with "import-func-get-symbol-info" (func $get-symbol-info))
      (with "import-type-symbol-kind" (type 0))
      (with "import-type-dialog-info" (type 1))
      (with "import-type-symbol-info" (type 3))
    )
  )
  (export $mmcomando:zengine-scripts/manage@0.1.0 (;1;) "mmcomando:zengine-scripts/manage@0.1.0" (instance $mmcomando:zengine-scripts/manage@0.1.0-shim-instance))
  (@producers
    (processed-by "wit-component" "0.251.0")
  )
)
