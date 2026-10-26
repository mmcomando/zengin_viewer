# About
Experimental project to check how hard it would be to create world viewer for Gothic 1/2 maps in [bevy](https://github.com/bevyengine/bevy) game engine.

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
- Scripts
- Dialogs
- Movement
- Voices
- Quests
- Journal
- Inventory
- Day/Night cycle

# TODO
- ZenGin skybox
- Weather
- Combat
- States Menu
- Save/Load
- Everything else
