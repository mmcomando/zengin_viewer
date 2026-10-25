(component
  (core module $main (;0;)
    (type (;0;) (func))
    (type (;1;) (func (result i32)))
    (type (;2;) (func (param i32 i32 i32 i32) (result i32)))
    (type (;3;) (func (param i32) (result i32)))
    (type (;4;) (func (param i32)))
    (type (;5;) (func (param i32 i32)))
    (type (;6;) (func (param i32 i32) (result i32)))
    (type (;7;) (func (result i32)))
    (table (;0;) 1 funcref)
    (memory (;0;) 1)
    (memory (;1;) 1)
    (memory (;2;) 1)
    (global (;0;) (mut i32) i32.const 0)
    (global (;1;) (mut i32) i32.const 0)
    (global (;2;) (mut i32) i32.const 0)
    (global (;3;) (mut i32) i32.const 1)
    (global (;4;) (mut i32) i32.const -1)
    (global (;5;) (mut i32) i32.const -1)
    (global (;6;) (mut i32) i32.const -1)
    (global (;7;) (mut i32) i32.const -1)
    (global (;8;) (mut i32) i32.const -1)
    (global (;9;) (mut i32) i32.const 1)
    (global (;10;) (mut i32) i32.const 16905)
    (global (;11;) (mut i32) i32.const 0)
    (global (;12;) (mut i32) i32.const 0)
    (export "cabi_realloc" (func 1))
    (export "mmcomando:zengine-scripts/manage@0.1.0#set-instance" (func 3))
    (export "mmcomando:zengine-scripts/manage@0.1.0#set-self" (func 5))
    (export "mmcomando:zengine-scripts/manage@0.1.0#set-other" (func 6))
    (export "mmcomando:zengine-scripts/manage@0.1.0#set-victim" (func 7))
    (export "mmcomando:zengine-scripts/manage@0.1.0#set-item" (func 8))
    (export "mmcomando:zengine-scripts/manage@0.1.0#set-hero" (func 9))
    (export "mmcomando:zengine-scripts/manage@0.1.0#call-npc-routine" (func 10))
    (export "mmcomando:zengine-scripts/manage@0.1.0#get-name" (func 11))
    (export "mmcomando:zengine-scripts/manage@0.1.0#get-symbol-id" (func 12))
    (export "mmcomando:zengine-scripts/manage@0.1.0#call-function" (func 13))
    (export "mmcomando:zengine-scripts/manage@0.1.0#call-function-discard" (func 14))
    (export "mmcomando:zengine-scripts/manage@0.1.0#call-function-bool" (func 26))
    (export "mmcomando:zengine-scripts/manage@0.1.0#get-all-symbols-num" (func 15))
    (export "mmcomando:zengine-scripts/manage@0.1.0#get-all-strings-num" (func 16))
    (export "mmcomando:zengine-scripts/manage@0.1.0#get-symbol-info" (func 17))
    (export "mmcomando:zengine-scripts/manage@0.1.0#initialize-all-instances" (func 18))
    (export "mmcomando:zengine-scripts/manage@0.1.0#get-item-visual" (func 21))
    (export "mmcomando:zengine-scripts/manage@0.1.0#get-item-visual-change" (func 22))
    (export "mmcomando:zengine-scripts/manage@0.1.0#get-npc-info" (func 23))
    (export "mmcomando:zengine-scripts/manage@0.1.0#get-item-info" (func 24))
    (export "mmcomando:zengine-scripts/manage@0.1.0#get-dialog-info" (func 25))
    (export "memory" (memory 0))
    (export "mmcomando:zengine-scripts/manage@0.1.0#test-func" (func 28))
    (elem (;0;) (table 0) (i32.const 0) func 28)
    (func (;0;) (type 2) (param i32 i32 i32 i32) (result i32)
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
    (func (;1;) (type 2) (param i32 i32 i32 i32) (result i32)
      (local i32)
      local.get 2
      ;; [i32]
      i32.const 4
      ;; [i32 i32]
      i32.gt_u
      ;; [i32]
      if ;; label = @1
        ;; []
      else
        ;; []
        i32.const 4
        ;; [i32]
        local.set 2
        ;; []
      end
      ;; []
      global.get 2
      ;; [i32]
      local.get 2
      ;; [i32 i32]
      i32.add
      ;; [i32]
      i32.const 1
      ;; [i32 i32]
      i32.sub
      ;; [i32]
      i32.const 0
      ;; [i32 i32]
      local.get 2
      ;; [i32 i32 i32]
      i32.sub
      ;; [i32 i32]
      i32.and
      ;; [i32]
      local.set 4
      ;; []
      local.get 4
      ;; [i32]
      local.get 3
      ;; [i32 i32]
      i32.add
      ;; [i32]
      i32.const 16256
      ;; [i32 i32]
      i32.gt_u
      ;; [i32]
      if ;; label = @1
        ;; []
        i32.const 16
        ;; [i32]
        local.set 4
        ;; []
      end
      ;; []
      local.get 4
      ;; [i32]
      local.get 3
      ;; [i32 i32]
      i32.add
      ;; [i32]
      local.get 3
      ;; [i32 i32]
      i32.eqz
      ;; [i32 i32]
      i32.add
      ;; [i32]
      global.set 2
      ;; []
      local.get 4
      ;; [i32]
    )
    (func (;2;) (type 3) (param i32) (result i32)
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
    (func (;3;) (type 4) (param i32)
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
    (func (;4;) (type 5) (param i32 i32)
      local.get 0
      ;; [i32]
      i32.const 0
      ;; [i32 i32]
      i32.ge_s
      ;; [i32]
      local.get 1
      ;; [i32 i32]
      i32.const 0
      ;; [i32 i32 i32]
      i32.gt_s
      ;; [i32 i32]
      i32.and
      ;; [i32]
      if ;; label = @1
        ;; []
        local.get 0
        ;; [i32]
        call 2
        ;; [i32]
        local.get 1
        ;; [i32 i32]
        i32.store 2
        ;; []
      end
      ;; []
    )
    (func (;5;) (type 4) (param i32)
      global.get 4
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      call 4
      ;; []
    )
    (func (;6;) (type 4) (param i32)
      global.get 5
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      call 4
      ;; []
    )
    (func (;7;) (type 4) (param i32)
      global.get 6
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      call 4
      ;; []
    )
    (func (;8;) (type 4) (param i32)
      global.get 7
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      call 4
      ;; []
    )
    (func (;9;) (type 4) (param i32)
      global.get 8
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      call 4
      ;; []
    )
    (func (;10;) (type 4) (param i32)
      (local i32)
      local.get 0
      ;; [i32]
      i32.const 608
      ;; [i32 i32]
      call 19
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
    (func (;11;) (type 3) (param i32) (result i32)
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
    (func (;12;) (type 6) (param i32 i32) (result i32)
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
      local.set 4
      ;; []
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
          local.get 4
          ;; [i32]
          i32.const 0
          ;; [i32 i32]
          i32.store8
          ;; []
          local.get 4
          ;; [i32]
          i32.const 0
          ;; [i32 i32]
          i32.store offset=4
          ;; []
          local.get 4
          ;; [i32]
          return
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
          local.get 4
          ;; [i32]
          i32.const 1
          ;; [i32 i32]
          i32.store8
          ;; []
          local.get 4
          ;; [i32]
          local.get 2
          ;; [i32 i32]
          i32.store offset=4
          ;; []
          local.get 4
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
    (func (;13;) (type 4) (param i32)
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
    (func (;14;) (type 4) (param i32)
      (local i32 i32)
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
      i32.load 1 offset=8
      ;; [i32]
      local.set 2
      ;; []
      local.get 1
      ;; [i32]
      i32.load8_u 1 offset=2
      ;; [i32]
      if ;; label = @1
        ;; []
        local.get 2
        ;; [i32]
        call_indirect (type 1)
        ;; [i32]
        drop
        ;; []
      else
        ;; []
        local.get 2
        ;; [i32]
        call_indirect (type 0)
        ;; []
      end
      ;; []
    )
    (func (;15;) (type 1) (result i32)
      global.get 3
      ;; [i32]
    )
    (func (;16;) (type 1) (result i32)
      global.get 9
      ;; [i32]
    )
    (func (;17;) (type 3) (param i32) (result i32)
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
    (func (;18;) (type 0)
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
        call 5
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
    (func (;19;) (type 6) (param i32 i32) (result i32)
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
    (func (;20;) (type 6) (param i32 i32) (result i32)
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
      call 19
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
    (func (;21;) (type 3) (param i32) (result i32)
      local.get 0
      ;; [i32]
      i32.const 524
      ;; [i32 i32]
      call 20
      ;; [i32]
    )
    (func (;22;) (type 3) (param i32) (result i32)
      local.get 0
      ;; [i32]
      i32.const 544
      ;; [i32 i32]
      call 20
      ;; [i32]
    )
    (func (;23;) (type 3) (param i32) (result i32)
      (local i32 i32)
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
      i32.const 440
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store
      ;; []
      local.get 1
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 444
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store offset=4
      ;; []
      local.get 0
      ;; [i32]
      i32.const 292
      ;; [i32 i32]
      call 19
      ;; [i32]
      local.set 2
      ;; []
      local.get 1
      ;; [i32]
      local.get 2
      ;; [i32 i32]
      i32.const 0
      ;; [i32 i32 i32]
      i32.ne
      ;; [i32 i32]
      i32.store8 offset=8
      ;; []
      local.get 1
      ;; [i32]
      local.get 2
      ;; [i32 i32]
      i32.store offset=12
      ;; []
      local.get 1
      ;; [i32]
    )
    (func (;24;) (type 3) (param i32) (result i32)
      (local i32 i32 i32)
      i32.const 0
      ;; [i32]
      i32.const 0
      ;; [i32 i32]
      i32.const 0
      ;; [i32 i32 i32]
      i32.const 60
      ;; [i32 i32 i32 i32]
      call 1
      ;; [i32]
      local.set 1
      ;; []
      local.get 1
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 360
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store
      ;; []
      local.get 0
      ;; [i32]
      i32.const 292
      ;; [i32 i32]
      call 19
      ;; [i32]
      local.set 2
      ;; []
      local.get 1
      ;; [i32]
      local.get 2
      ;; [i32 i32]
      i32.const 0
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
      local.get 1
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 352
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store offset=12
      ;; []
      local.get 1
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 348
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store offset=16
      ;; []
      i32.const 0
      ;; [i32]
      i32.const 0
      ;; [i32 i32]
      i32.const 0
      ;; [i32 i32 i32]
      i32.const 12
      ;; [i32 i32 i32 i32]
      call 1
      ;; [i32]
      local.set 3
      ;; []
      local.get 3
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 448
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store
      ;; []
      local.get 3
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 452
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store offset=4
      ;; []
      local.get 3
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 456
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store offset=8
      ;; []
      local.get 1
      ;; [i32]
      local.get 3
      ;; [i32 i32]
      i32.store offset=20
      ;; []
      local.get 1
      ;; [i32]
      i32.const 3
      ;; [i32 i32]
      i32.store offset=24
      ;; []
      i32.const 0
      ;; [i32]
      i32.const 0
      ;; [i32 i32]
      i32.const 0
      ;; [i32 i32 i32]
      i32.const 12
      ;; [i32 i32 i32 i32]
      call 1
      ;; [i32]
      local.set 3
      ;; []
      local.get 3
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 460
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store
      ;; []
      local.get 3
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 464
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store offset=4
      ;; []
      local.get 3
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 468
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store offset=8
      ;; []
      local.get 1
      ;; [i32]
      local.get 3
      ;; [i32 i32]
      i32.store offset=28
      ;; []
      local.get 1
      ;; [i32]
      i32.const 3
      ;; [i32 i32]
      i32.store offset=32
      ;; []
      local.get 0
      ;; [i32]
      i32.const 628
      ;; [i32 i32]
      call 19
      ;; [i32]
      local.set 2
      ;; []
      local.get 1
      ;; [i32]
      local.get 2
      ;; [i32 i32]
      i32.const 0
      ;; [i32 i32 i32]
      i32.ne
      ;; [i32 i32]
      i32.store8 offset=36
      ;; []
      local.get 1
      ;; [i32]
      local.get 2
      ;; [i32 i32]
      i32.store offset=40
      ;; []
      i32.const 0
      ;; [i32]
      i32.const 0
      ;; [i32 i32]
      i32.const 0
      ;; [i32 i32 i32]
      i32.const 24
      ;; [i32 i32 i32 i32]
      call 1
      ;; [i32]
      local.set 3
      ;; []
      local.get 3
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 648
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store
      ;; []
      local.get 3
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 652
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store offset=4
      ;; []
      local.get 3
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 656
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store offset=8
      ;; []
      local.get 3
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 660
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store offset=12
      ;; []
      local.get 3
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 664
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store offset=16
      ;; []
      local.get 3
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 668
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store offset=20
      ;; []
      local.get 1
      ;; [i32]
      local.get 3
      ;; [i32 i32]
      i32.store offset=44
      ;; []
      local.get 1
      ;; [i32]
      i32.const 6
      ;; [i32 i32]
      i32.store offset=48
      ;; []
      i32.const 0
      ;; [i32]
      i32.const 0
      ;; [i32 i32]
      i32.const 0
      ;; [i32 i32 i32]
      i32.const 24
      ;; [i32 i32 i32 i32]
      call 1
      ;; [i32]
      local.set 3
      ;; []
      local.get 3
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 768
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store
      ;; []
      local.get 3
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 772
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store offset=4
      ;; []
      local.get 3
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 776
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store offset=8
      ;; []
      local.get 3
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 780
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store offset=12
      ;; []
      local.get 3
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 784
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store offset=16
      ;; []
      local.get 3
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 788
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store offset=20
      ;; []
      local.get 1
      ;; [i32]
      local.get 3
      ;; [i32 i32]
      i32.store offset=52
      ;; []
      local.get 1
      ;; [i32]
      i32.const 6
      ;; [i32 i32]
      i32.store offset=56
      ;; []
      local.get 1
      ;; [i32]
    )
    (func (;25;) (type 3) (param i32) (result i32)
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
      call 19
      ;; [i32 i32]
      i32.store
      ;; []
      local.get 1
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 4
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store offset=4
      ;; []
      local.get 1
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 12
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store offset=8
      ;; []
      local.get 1
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 16
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store offset=12
      ;; []
      local.get 1
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 20
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.store offset=16
      ;; []
      local.get 1
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 8
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.const 0
      ;; [i32 i32 i32]
      i32.ne
      ;; [i32 i32]
      i32.store8 offset=20
      ;; []
      local.get 1
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 44
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.const 0
      ;; [i32 i32 i32]
      i32.ne
      ;; [i32 i32]
      i32.store8 offset=21
      ;; []
      local.get 1
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      i32.const 40
      ;; [i32 i32 i32]
      call 19
      ;; [i32 i32]
      i32.const 0
      ;; [i32 i32 i32]
      i32.ne
      ;; [i32 i32]
      i32.store8 offset=22
      ;; []
      local.get 1
      ;; [i32]
    )
    (func (;26;) (type 3) (param i32) (result i32)
      local.get 0
      ;; [i32]
      i32.const 16
      ;; [i32 i32]
      i32.mul
      ;; [i32]
      i32.load 1 offset=8
      ;; [i32]
      call_indirect (type 1)
      ;; [i32]
      i32.const 0
      ;; [i32 i32]
      i32.ne
      ;; [i32]
    )
    (func (;27;) (type 6) (param i32 i32) (result i32)
      (local i32 i32 i32)
      local.get 1
      ;; [i32]
      i32.const 8192
      ;; [i32 i32]
      i32.gt_u
      ;; [i32]
      if ;; label = @1
        ;; []
        i32.const 8192
        ;; [i32]
        local.set 1
        ;; []
      end
      ;; []
      global.get 12
      ;; [i32]
      local.get 1
      ;; [i32 i32]
      i32.add
      ;; [i32]
      i32.const 8192
      ;; [i32 i32]
      i32.gt_u
      ;; [i32]
      if ;; label = @1
        ;; []
        i32.const 0
        ;; [i32]
        global.set 12
        ;; []
      end
      ;; []
      global.get 11
      ;; [i32]
      i32.const 64
      ;; [i32 i32]
      i32.rem_u
      ;; [i32]
      local.set 2
      ;; []
      global.get 10
      ;; [i32]
      global.get 12
      ;; [i32 i32]
      i32.add
      ;; [i32]
      local.set 3
      ;; []
      local.get 3
      ;; [i32]
      local.get 0
      ;; [i32 i32]
      local.get 1
      ;; [i32 i32 i32]
      memory.copy
      ;; []
      global.get 9
      ;; [i32]
      local.get 2
      ;; [i32 i32]
      i32.add
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
      local.get 4
      ;; [i32]
      local.get 3
      ;; [i32 i32]
      i32.store
      ;; []
      local.get 4
      ;; [i32]
      local.get 1
      ;; [i32 i32]
      i32.store offset=4
      ;; []
      global.get 11
      ;; [i32]
      i32.const 1
      ;; [i32 i32]
      i32.add
      ;; [i32]
      global.set 11
      ;; []
      global.get 12
      ;; [i32]
      local.get 1
      ;; [i32 i32]
      i32.add
      ;; [i32]
      global.set 12
      ;; []
      global.get 9
      ;; [i32]
      local.get 2
      ;; [i32 i32]
      i32.add
      ;; [i32]
    )
    (func (;28;) (type 7) (result i32)
      (local i32 i32)
      i32.const 0
      ;; [i32]
      if ;; label = @1
        ;; []
        i32.const 42
        ;; [i32]
        return
        ;; []
      end
      ;; []
      i32.const 0
      ;; [i32]
      return
      ;; []
    )
    (data (;0;) (i32.const 16384) "\08B\00\00\01\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00")
    (data (;1;) (i32.const 16904) "0\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00\00")
    (data (;2;) (memory 1) (i32.const 0) "\01\00\00\00\00\00\00\00\ff\ff\ff\ff\ff\ff\ff\ff")
    (@producers
      (processed-by "wit-component" "0.251.0")
    )
  )
  (type (;0;) (enum "float" "int" "str" "class" "function" "prototype" "instance" "variable-argument"))
  (type (;1;) (record (field "npc" s32) (field "order" s32) (field "condition" s32) (field "information" s32) (field "description" s32) (field "important" bool) (field "permanent" bool) (field "trade" bool)))
  (type (;2;) (option s32))
  (type (;3;) (record (field "kind" 0) (field "parent" 2) (field "function" 2)))
  (type (;4;) (record (field "hitpoints" s32) (field "hitpoints-max" s32) (field "name" 2)))
  (type (;5;) (list s32))
  (type (;6;) (record (field "damage-total" s32) (field "name" 2) (field "value" s32) (field "weight" s32) (field "cond-attribute" 5) (field "cond-value" 5) (field "description" 2) (field "text" 5) (field "count" 5)))
  (core instance $main (;0;) (instantiate $main))
  (alias core export $main "memory" (core memory $memory (;0;)))
  (type (;7;) (func (result s32)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#test-func" (core func $mmcomando:zengine-scripts/manage@0.1.0#test-func (;0;)))
  (alias core export $main "cabi_realloc" (core func $cabi_realloc (;1;)))
  (func $test-func (;0;) (type 7) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#test-func)))
  (type (;8;) (func (param "instance-id" s32)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#set-instance" (core func $mmcomando:zengine-scripts/manage@0.1.0#set-instance (;2;)))
  (func $set-instance (;1;) (type 8) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#set-instance)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#set-self" (core func $mmcomando:zengine-scripts/manage@0.1.0#set-self (;3;)))
  (func $set-self (;2;) (type 8) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#set-self)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#set-other" (core func $mmcomando:zengine-scripts/manage@0.1.0#set-other (;4;)))
  (func $set-other (;3;) (type 8) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#set-other)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#set-victim" (core func $mmcomando:zengine-scripts/manage@0.1.0#set-victim (;5;)))
  (func $set-victim (;4;) (type 8) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#set-victim)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#set-item" (core func $mmcomando:zengine-scripts/manage@0.1.0#set-item (;6;)))
  (func $set-item (;5;) (type 8) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#set-item)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#set-hero" (core func $mmcomando:zengine-scripts/manage@0.1.0#set-hero (;7;)))
  (func $set-hero (;6;) (type 8) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#set-hero)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#call-npc-routine" (core func $mmcomando:zengine-scripts/manage@0.1.0#call-npc-routine (;8;)))
  (func $call-npc-routine (;7;) (type 8) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#call-npc-routine)))
  (type (;9;) (func (param "symbol-id" s32) (result string)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#get-name" (core func $mmcomando:zengine-scripts/manage@0.1.0#get-name (;9;)))
  (func $get-name (;8;) (type 9) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#get-name) (memory $memory) string-encoding=utf8))
  (type (;10;) (func (param "symbol-id" s32)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#call-function" (core func $mmcomando:zengine-scripts/manage@0.1.0#call-function (;10;)))
  (func $call-function (;9;) (type 10) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#call-function)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#call-function-discard" (core func $mmcomando:zengine-scripts/manage@0.1.0#call-function-discard (;11;)))
  (func $call-function-discard (;10;) (type 10) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#call-function-discard)))
  (type (;11;) (func (param "symbol-id" s32) (result bool)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#call-function-bool" (core func $mmcomando:zengine-scripts/manage@0.1.0#call-function-bool (;12;)))
  (func $call-function-bool (;11;) (type 11) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#call-function-bool)))
  (type (;12;) (func (param "symbol-name" string) (result 2)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#get-symbol-id" (core func $mmcomando:zengine-scripts/manage@0.1.0#get-symbol-id (;13;)))
  (func $get-symbol-id (;12;) (type 12) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#get-symbol-id) (memory $memory) (realloc $cabi_realloc) string-encoding=utf8))
  (type (;13;) (option string))
  (type (;14;) (func (param "symbol-id" s32) (result 13)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#get-item-visual" (core func $mmcomando:zengine-scripts/manage@0.1.0#get-item-visual (;14;)))
  (func $get-item-visual (;13;) (type 14) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#get-item-visual) (memory $memory) string-encoding=utf8))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#get-item-visual-change" (core func $mmcomando:zengine-scripts/manage@0.1.0#get-item-visual-change (;15;)))
  (func $get-item-visual-change (;14;) (type 14) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#get-item-visual-change) (memory $memory) string-encoding=utf8))
  (type (;15;) (func (param "symbol-id" s32) (result 4)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#get-npc-info" (core func $mmcomando:zengine-scripts/manage@0.1.0#get-npc-info (;16;)))
  (func $get-npc-info (;15;) (type 15) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#get-npc-info) (memory $memory)))
  (type (;16;) (func (param "symbol-id" s32) (result 6)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#get-item-info" (core func $mmcomando:zengine-scripts/manage@0.1.0#get-item-info (;17;)))
  (func $get-item-info (;16;) (type 16) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#get-item-info) (memory $memory)))
  (type (;17;) (func))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#initialize-all-instances" (core func $mmcomando:zengine-scripts/manage@0.1.0#initialize-all-instances (;18;)))
  (func $initialize-all-instances (;17;) (type 17) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#initialize-all-instances)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#get-all-symbols-num" (core func $mmcomando:zengine-scripts/manage@0.1.0#get-all-symbols-num (;19;)))
  (func $get-all-symbols-num (;18;) (type 7) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#get-all-symbols-num)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#get-all-strings-num" (core func $mmcomando:zengine-scripts/manage@0.1.0#get-all-strings-num (;20;)))
  (func $get-all-strings-num (;19;) (type 7) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#get-all-strings-num)))
  (type (;18;) (func (param "symbol-id" s32) (result 1)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#get-dialog-info" (core func $mmcomando:zengine-scripts/manage@0.1.0#get-dialog-info (;21;)))
  (func $get-dialog-info (;20;) (type 18) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#get-dialog-info) (memory $memory)))
  (type (;19;) (func (param "symbol-id" s32) (result 3)))
  (alias core export $main "mmcomando:zengine-scripts/manage@0.1.0#get-symbol-info" (core func $mmcomando:zengine-scripts/manage@0.1.0#get-symbol-info (;22;)))
  (func $get-symbol-info (;21;) (type 19) (canon lift (core func $mmcomando:zengine-scripts/manage@0.1.0#get-symbol-info) (memory $memory)))
  (component $mmcomando:zengine-scripts/manage@0.1.0-shim-component (;0;)
    (type (;0;) (func (result s32)))
    (import "import-func-test-func" (func (;0;) (type 0)))
    (type (;1;) (func (param "instance-id" s32)))
    (import "import-func-set-instance" (func (;1;) (type 1)))
    (import "import-func-set-self" (func (;2;) (type 1)))
    (import "import-func-set-other" (func (;3;) (type 1)))
    (import "import-func-set-victim" (func (;4;) (type 1)))
    (import "import-func-set-item" (func (;5;) (type 1)))
    (import "import-func-set-hero" (func (;6;) (type 1)))
    (import "import-func-call-npc-routine" (func (;7;) (type 1)))
    (type (;2;) (func (param "symbol-id" s32) (result string)))
    (import "import-func-get-name" (func (;8;) (type 2)))
    (type (;3;) (func (param "symbol-id" s32)))
    (import "import-func-call-function" (func (;9;) (type 3)))
    (import "import-func-call-function-discard" (func (;10;) (type 3)))
    (type (;4;) (func (param "symbol-id" s32) (result bool)))
    (import "import-func-call-function-bool" (func (;11;) (type 4)))
    (type (;5;) (option s32))
    (type (;6;) (func (param "symbol-name" string) (result 5)))
    (import "import-func-get-symbol-id" (func (;12;) (type 6)))
    (type (;7;) (option string))
    (type (;8;) (func (param "symbol-id" s32) (result 7)))
    (import "import-func-get-item-visual" (func (;13;) (type 8)))
    (import "import-func-get-item-visual-change" (func (;14;) (type 8)))
    (type (;9;) (record (field "hitpoints" s32) (field "hitpoints-max" s32) (field "name" 5)))
    (import "import-type-npc-info" (type (;10;) (eq 9)))
    (type (;11;) (func (param "symbol-id" s32) (result 10)))
    (import "import-func-get-npc-info" (func (;15;) (type 11)))
    (type (;12;) (list s32))
    (type (;13;) (record (field "damage-total" s32) (field "name" 5) (field "value" s32) (field "weight" s32) (field "cond-attribute" 12) (field "cond-value" 12) (field "description" 5) (field "text" 12) (field "count" 12)))
    (import "import-type-item-info" (type (;14;) (eq 13)))
    (type (;15;) (func (param "symbol-id" s32) (result 14)))
    (import "import-func-get-item-info" (func (;16;) (type 15)))
    (type (;16;) (func))
    (import "import-func-initialize-all-instances" (func (;17;) (type 16)))
    (import "import-func-get-all-symbols-num" (func (;18;) (type 0)))
    (import "import-func-get-all-strings-num" (func (;19;) (type 0)))
    (type (;17;) (record (field "npc" s32) (field "order" s32) (field "condition" s32) (field "information" s32) (field "description" s32) (field "important" bool) (field "permanent" bool) (field "trade" bool)))
    (import "import-type-dialog-info" (type (;18;) (eq 17)))
    (type (;19;) (func (param "symbol-id" s32) (result 18)))
    (import "import-func-get-dialog-info" (func (;20;) (type 19)))
    (type (;20;) (enum "float" "int" "str" "class" "function" "prototype" "instance" "variable-argument"))
    (import "import-type-symbol-kind" (type (;21;) (eq 20)))
    (type (;22;) (record (field "kind" 21) (field "parent" 5) (field "function" 5)))
    (import "import-type-symbol-info" (type (;23;) (eq 22)))
    (type (;24;) (func (param "symbol-id" s32) (result 23)))
    (import "import-func-get-symbol-info" (func (;21;) (type 24)))
    (type (;25;) (enum "float" "int" "str" "class" "function" "prototype" "instance" "variable-argument"))
    (export (;26;) "symbol-kind" (type 25))
    (type (;27;) (record (field "npc" s32) (field "order" s32) (field "condition" s32) (field "information" s32) (field "description" s32) (field "important" bool) (field "permanent" bool) (field "trade" bool)))
    (export (;28;) "dialog-info" (type 27))
    (type (;29;) (option s32))
    (type (;30;) (record (field "kind" 26) (field "parent" 29) (field "function" 29)))
    (export (;31;) "symbol-info" (type 30))
    (type (;32;) (record (field "hitpoints" s32) (field "hitpoints-max" s32) (field "name" 29)))
    (export (;33;) "npc-info" (type 32))
    (type (;34;) (list s32))
    (type (;35;) (record (field "damage-total" s32) (field "name" 29) (field "value" s32) (field "weight" s32) (field "cond-attribute" 34) (field "cond-value" 34) (field "description" 29) (field "text" 34) (field "count" 34)))
    (export (;36;) "item-info" (type 35))
    (type (;37;) (func (result s32)))
    (export (;22;) "test-func" (func 0) (func (type 37)))
    (type (;38;) (func (param "instance-id" s32)))
    (export (;23;) "set-instance" (func 1) (func (type 38)))
    (export (;24;) "set-self" (func 2) (func (type 38)))
    (export (;25;) "set-other" (func 3) (func (type 38)))
    (export (;26;) "set-victim" (func 4) (func (type 38)))
    (export (;27;) "set-item" (func 5) (func (type 38)))
    (export (;28;) "set-hero" (func 6) (func (type 38)))
    (export (;29;) "call-npc-routine" (func 7) (func (type 38)))
    (type (;39;) (func (param "symbol-id" s32) (result string)))
    (export (;30;) "get-name" (func 8) (func (type 39)))
    (type (;40;) (func (param "symbol-id" s32)))
    (export (;31;) "call-function" (func 9) (func (type 40)))
    (export (;32;) "call-function-discard" (func 10) (func (type 40)))
    (type (;41;) (func (param "symbol-id" s32) (result bool)))
    (export (;33;) "call-function-bool" (func 11) (func (type 41)))
    (type (;42;) (func (param "symbol-name" string) (result 29)))
    (export (;34;) "get-symbol-id" (func 12) (func (type 42)))
    (type (;43;) (option string))
    (type (;44;) (func (param "symbol-id" s32) (result 43)))
    (export (;35;) "get-item-visual" (func 13) (func (type 44)))
    (export (;36;) "get-item-visual-change" (func 14) (func (type 44)))
    (type (;45;) (func (param "symbol-id" s32) (result 33)))
    (export (;37;) "get-npc-info" (func 15) (func (type 45)))
    (type (;46;) (func (param "symbol-id" s32) (result 36)))
    (export (;38;) "get-item-info" (func 16) (func (type 46)))
    (type (;47;) (func))
    (export (;39;) "initialize-all-instances" (func 17) (func (type 47)))
    (export (;40;) "get-all-symbols-num" (func 18) (func (type 37)))
    (export (;41;) "get-all-strings-num" (func 19) (func (type 37)))
    (type (;48;) (func (param "symbol-id" s32) (result 28)))
    (export (;42;) "get-dialog-info" (func 20) (func (type 48)))
    (type (;49;) (func (param "symbol-id" s32) (result 31)))
    (export (;43;) "get-symbol-info" (func 21) (func (type 49)))
  )
  (instance $mmcomando:zengine-scripts/manage@0.1.0-shim-instance (;0;) (instantiate $mmcomando:zengine-scripts/manage@0.1.0-shim-component
      (with "import-func-test-func" (func $test-func))
      (with "import-func-set-instance" (func $set-instance))
      (with "import-func-set-self" (func $set-self))
      (with "import-func-set-other" (func $set-other))
      (with "import-func-set-victim" (func $set-victim))
      (with "import-func-set-item" (func $set-item))
      (with "import-func-set-hero" (func $set-hero))
      (with "import-func-call-npc-routine" (func $call-npc-routine))
      (with "import-func-get-name" (func $get-name))
      (with "import-func-call-function" (func $call-function))
      (with "import-func-call-function-discard" (func $call-function-discard))
      (with "import-func-call-function-bool" (func $call-function-bool))
      (with "import-func-get-symbol-id" (func $get-symbol-id))
      (with "import-func-get-item-visual" (func $get-item-visual))
      (with "import-func-get-item-visual-change" (func $get-item-visual-change))
      (with "import-func-get-npc-info" (func $get-npc-info))
      (with "import-func-get-item-info" (func $get-item-info))
      (with "import-func-initialize-all-instances" (func $initialize-all-instances))
      (with "import-func-get-all-symbols-num" (func $get-all-symbols-num))
      (with "import-func-get-all-strings-num" (func $get-all-strings-num))
      (with "import-func-get-dialog-info" (func $get-dialog-info))
      (with "import-func-get-symbol-info" (func $get-symbol-info))
      (with "import-type-npc-info" (type 4))
      (with "import-type-item-info" (type 6))
      (with "import-type-dialog-info" (type 1))
      (with "import-type-symbol-kind" (type 0))
      (with "import-type-symbol-info" (type 3))
    )
  )
  (export $mmcomando:zengine-scripts/manage@0.1.0 (;1;) "mmcomando:zengine-scripts/manage@0.1.0" (instance $mmcomando:zengine-scripts/manage@0.1.0-shim-instance))
  (@producers
    (processed-by "wit-component" "0.251.0")
  )
)
