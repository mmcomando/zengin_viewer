use super::*;

impl mmcomando::zengine_scripts::apply_scripts::Host for ImportCallState {
    // Menu script (MENU.DAT) externals: options apply callbacks and the
    // choicebox update used by settings menus. Not implemented yet.
    fake_impl!(apply_options_audio());
    fake_impl!(apply_options_controls());
    fake_impl!(apply_options_game());
    fake_impl!(apply_options_performance());
    fake_impl!(apply_options_video());
}
