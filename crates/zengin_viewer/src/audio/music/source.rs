//! Bevy audio source rendering the shared `DirectMusic` performance.

use std::sync::Arc;

use bevy::audio::{ChannelCount, Decodable, SampleRate, Source};
use bevy::prelude::*;
use bevy::reflect::TypePath;

use super::engine::{SAMPLE_RATE, SharedMusicCore, render};

/// Number of interleaved stereo `i16` samples rendered per fill.
const BUFFER_SAMPLES: usize = 2048;

/// Long-lived music asset. The actual performance is shared with
/// [`MusicEngine`](super::engine::MusicEngine).
#[derive(Asset, TypePath)]
pub(crate) struct MusicSource {
    core: SharedMusicCore,
}

impl MusicSource {
    pub(crate) fn new(core: SharedMusicCore) -> Self {
        Self { core }
    }
}

pub(crate) struct MusicDecoder {
    core: SharedMusicCore,
    buffer: Vec<i16>,
    index: usize,
}

impl MusicDecoder {
    fn new(core: SharedMusicCore) -> Self {
        Self {
            core,
            buffer: vec![0; BUFFER_SAMPLES],
            // Forces a fill before the first sample is returned.
            index: BUFFER_SAMPLES,
        }
    }

    fn fill(&mut self) {
        render(&self.core, &mut self.buffer);
        self.index = 0;
    }
}

impl Iterator for MusicDecoder {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.buffer.len() {
            self.fill();
        }
        let sample = self.buffer[self.index];
        self.index += 1;
        Some(f32::from(sample) / 32768.0)
    }
}

impl Source for MusicDecoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::new(2).expect("stereo output")
    }

    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(SAMPLE_RATE).expect("non-zero sample rate")
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

impl Decodable for MusicSource {
    type Decoder = MusicDecoder;

    fn decoder(&self) -> Self::Decoder {
        MusicDecoder::new(Arc::clone(&self.core))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[test]
    fn silent_source_is_stereo_and_infinite() {
        let source = MusicSource::new(Arc::new(Mutex::new(None)));
        let mut decoder = source.decoder();

        assert_eq!(decoder.channels().get(), 2);
        assert_eq!(decoder.sample_rate().get(), SAMPLE_RATE);
        for _ in 0..1000 {
            assert_eq!(decoder.next(), Some(0.0));
        }
    }
}
