use super::*;

impl mmcomando::zengine_scripts::mob_scripts::Host for ImportCallState {
    fake_impl!(mob_createitems(_0, _1, _2));
    fake_impl!(mob_hasitems(_0, _1) -> i32);
}
