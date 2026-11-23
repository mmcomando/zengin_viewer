build:
    cargo build

run:
    cargo run

run-renderless:
    cargo run -- --renderless --frames 600

# Record actions into the scenario file of the given scenario ID (without .ron).
record-replay id:
    cargo run -- --fixed-time --save-actions misc/scenario/{{id}}.ron

# Replay the scenario with the given ID (a path under misc/scenario).
run-scenario id:
    cargo run -- --fixed-time --no-debug-gui --scenario {{id}}

# Replay a scenario that starts in the main menu instead of a new game.
run-scenario-menu id:
    cargo run -- --fixed-time --no-debug-gui --main-menu --scenario {{id}} --frames 1000

# Run a scenario containing Test commands, comparing TestScreenshot references.
test-scenario id:
    cargo run -- --fixed-time --no-debug-gui --scenario {{id}}

# Bless the TestScreenshot references of a scenario.
update-test-scenario id:
    cargo run -- --fixed-time --no-debug-gui --scenario {{id}} --update-reference

all-test-scenarios:
    just test-scenario tests/stats
    just test-scenario tests/journal
    just test-scenario tests/dialog_lares
    just test-scenario tests/dialog_guard_entrance
    just test-scenario tests/dialog_lothar
    just test-scenario tests/wolf_combat
    just test-scenario tests/player_use_chair
    just test-scenario tests/player_use_chair_throne
    just test-scenario tests/player_use_forge_bookboard
    just test-scenario tests/player_use_bed
    just test-scenario tests/player_use_lever
    just test-scenario tests/player_use_door
    just test-scenario tests/chest_open
    just test-scenario tests/npc_use_mob
    just test-scenario tests/npc_death_scripts
    just test-scenario tests/npc_falls

# Headless CPU profile: writes a ~1GB chrome trace, then prints heaviest spans.
trace-chrome:
    cargo run --release --features trace_chrome -- --renderless --scenario walk_through_city --frames 600
    python3 misc/scripts/analyze_trace.py "$(ls -t trace-*.json | head -n1)"

# Profile a headless run to target/flamegraph.svg (skips 4s startup; needs perf+cargo-flamegraph).
flamegraph frames="4000":
    perf record -F 299 --call-graph fp -D 4000 -o target/perf.data -- cargo run -- --renderless --scenario walk_through_city --frames {{frames}}
    flamegraph --perfdata target/perf.data -o target/flamegraph.svg

export-symbol-names:
    cargo test -p ZenGinViewer -- --ignored --nocapture debug_zengin_scripts > misc/script_game_symbols.log

generate-animation-graph:
    cargo test -p ZenGinViewer animation_graph -- --ignored

get-submodules:
    git submodule update --init --recursive --checkout
