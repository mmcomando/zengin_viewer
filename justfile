build:
    cargo build

run:
    cargo run

run-renderless:
    cargo run -- --renderless --frames 600

# Record actions into the scenario replay file of the given scenario name.
record-replay name:
    cargo run -- --save-actions misc/scenarios/{{name}}.ron

# Replay the commands from the given scenario
run-replay name:
    cargo run -- --replay-actions misc/scenarios/{{name}}.ron --frames 600

# Headless CPU profile: writes a ~1GB chrome trace, then prints heaviest spans.
trace-chrome:
    cargo run --release --features trace_chrome -- --renderless --frames 600
    python3 misc/analyze_trace.py "$(ls -t trace-*.json | head -n1)"

# Profile a headless run to target/flamegraph.svg (skips 4s startup; needs perf+cargo-flamegraph).
flamegraph frames="2500":
    perf record -F 299 --call-graph fp -D 4000 -o target/perf.data -- cargo run --release -- --renderless --frames {{frames}}
    flamegraph --perfdata target/perf.data -o target/flamegraph.svg

export_symbol_names:
    cargo test -p ZenGinViewer -- --ignored --nocapture debug_zengin_scripts > misc/script_game_symbols.log

get_submodules:
    git submodule update --init --recursive --checkout
