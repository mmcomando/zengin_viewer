use super::*;

// This probably do nothing in original game
impl mmcomando::zengine_scripts::game_scripts::Host for ImportCallState {
    fn game_initgerman(&mut self) -> i32 {
        1
    }
    fn game_initenglish(&mut self) -> i32 {
        1
    }
    fn game_initengintl(&mut self) -> i32 {
        1
    }
}
