# About
Experimental project to check how hard it would be to create world viewer for Gothic 1/2 maps in [bevy](https://github.com/bevyengine/bevy) game engine.

Maybe it will evolve to a full game reimplementation?

# Screen
![Party](/misc/screenshots/party.jpg "Party")

![Farm](/misc/screenshots/farm.jpg "Farm")

![Khorinis at night](/misc/screenshots/khorinis.jpg "Khorinis at night")

# Quick start on Linux
```bash
git clone https://github.com/mmcomando/zengin_viewer.git
cd zengin_viewer
export GOTHIC2_DIR="$HOME/.local/share/Steam/steamapps/common/Gothic II"
cargo run
```

# What somehow works
- Rendering of world mesh
- Rendering of items, npcs, monsters, armors
- Animations
- Placing NPCs
- Lights

# TODO
- Scripts
- ZenGin skybox
- Weather
- Dialogs
- Quests
- Combat
- Everything else

# Note on performance
Currently rendering shadow textures takes a LOT of time because it draws almos all objects on the map.
All game objects are drawn and computed every frame so bad performance is expected.

Due to shadows performance hit they are disabled by default. To enable them, change this constant to false:
```rust
const PREFER_PERF: bool = true;
```

# Plans
I plan to support scripts by recompiling zengin binary scripts to wasm and support host functions using wasm component model.

Other TODO items will be done if I will have time and fun doing so.
