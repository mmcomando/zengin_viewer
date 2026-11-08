use super::*;

impl mmcomando::zengine_scripts::snd_scripts::Host for ImportCallState {
    fake_impl!(snd_play(_0));
    fake_impl!(snd_play3d(_0, _1));
    fake_impl!(snd_getdisttosource(_0) -> i32);
    fake_impl!(snd_issourcenpc(_0) -> i32);
    fake_impl!(snd_issourceitem(_0) -> i32);
}
