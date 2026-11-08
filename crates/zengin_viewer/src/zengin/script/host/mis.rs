use super::*;

impl mmcomando::zengine_scripts::mis_scripts::Host for ImportCallState {
    fake_impl!(mis_getstatus(_0) -> i32);
    fake_impl!(mis_setstatus(_0, _1));
    fake_impl!(mis_ontime(_0) -> i32);
    fake_impl!(mis_addmissionentry(_0, _1));
    fake_impl!(mis_removemission(_0));
}
