build:
    cargo build

run:
    cargo run

export_symbol_names:
    cargo test -p ZenGinViewer -- --ignored --nocapture debug_zengin_scripts > misc/script_game_symbols.log

get_submodules:
    git submodule update --init --recursive --checkout
