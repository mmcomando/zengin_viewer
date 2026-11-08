use super::*;

impl mmcomando::zengine_scripts::game_scripts::Host for ImportCallState {
    fake_impl!(game_initgerman() -> i32);
    fake_impl!(game_initenglish() -> i32);
    fake_impl!(game_initengintl() -> i32);
}
