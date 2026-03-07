# About
Experimental project to check how hard it would be to create world viewer for Gothic 1/2 maps in [bevy](https://github.com/bevyengine/bevy) game engine.

Maybe it will evolve to a full game reimplementation?

# Screen
![Farm](/misc/screenshots/farm.jpg "Farm")

![Khorinis at night](/misc/screenshots/khorinis.png "Khorinis at night")

# Quick start on Linux
```bash
git clone https://github.com/mmcomando/zengin_viewer.git
cd zengin_viewer
export GOTHIC2_DIR="$HOME/.local/share/Steam/steamapps/common/Gothic II"
cargo run
```

# What somehow works
- Rendering of world mesh
- Rendering of MRS meshes
- Placing NPCs
- Lights

# TODO
- Display NPCs armor
- Place monsters
- Scripts
- Animations
- ZenGin skybox
- Weather
- Dialogs
- Quests
- Combat
- Everything else

# Plans
I plan to support scripts by recompiling zengin binary scripts to wasm and support host functions using wasm component model.

Other TODO items will be done if I will have time and fun doing so.
