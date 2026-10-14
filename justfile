build:
    cargo build

run:
    cargo run

run-renderless:
    cargo run -- --renderless --frames 600

export_symbol_names:
    cargo test -p ZenGinViewer -- --ignored --nocapture debug_zengin_scripts > misc/script_game_symbols.log

get_submodules:
    git submodule update --init --recursive --checkout
