use super::*;

impl mmcomando::zengine_scripts::snd_scripts::Host for ImportCallState {
    /// Plays a global (non-positional) sound by its `C_SFX` definition name.
    fn snd_play(&mut self, sound: i32) {
        self.push_sound(sound, None);
    }

    /// Plays a sound at the npc's world position.
    fn snd_play3d(&mut self, npc: i32, sound: i32) {
        self.push_sound_at_npc(npc, sound);
    }

    fake_impl!(snd_getdisttosource(_0) -> i32);
    fake_impl!(snd_issourcenpc(_0) -> i32);
    fake_impl!(snd_issourceitem(_0) -> i32);
}
