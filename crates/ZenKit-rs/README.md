# About
Somehow safe bindings to [ZenKit](https://github.com/GothicKit/ZenKit) library.

ZenKit library is a C++ library so this library uses C API [ZenKitCAPI](https://github.com/GothicKit/ZenKitCAPI) for bindings.

Raw unsafe bindings are in repository [ZenKit-sys](https://github.com/mmcomando/ZenKit-sys).


# Example usage

```rust
let vfs = mounted_vfs();

let node = vfs
    .resolve_path("/_WORK/DATA/WORLDS/NEWWORLD/NEWWORLD.ZEN")
    .unwrap();
let reader = node.open().unwrap();
let world = World::load(&reader).unwrap();
let mesh = world.mesh();
let materials = mesh.materials();

let node_texture = vfs.find_node("OW_OLDCAMP_WALL_17-C.TEX").unwrap();
let node_texture_read = node_texture.open().unwrap();
let texture = Texture::load(&node_texture_read).unwrap();
```

# Status
This bindings are incomplete. Only entries which I needed were ported.


# Safety
Bindings are marked as safe but it was not verified if all rust safety requirements are met.

Bindings were created in best effort basis, so issues and missing functionality migth be expected.
