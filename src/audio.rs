//! Procedurally synthesized sound effects, so the game ships without asset files.

use macroquad::audio::{load_sound_from_bytes, play_sound, PlaySoundParams, Sound};

const SAMPLE_RATE: u32 = 22050;

#[derive(Clone, Copy)]
pub enum Sfx {
    Select,
    Start,
    Coin,
    PowerUp,
    NearMiss,
    Dash,
    Hit,
    GameOver,
}

pub struct Audio {
    sounds: Vec<Option<Sound>>,
    pub muted: bool,
}

impl Audio {
    pub async fn load(muted: bool) -> Self {
        let mut sounds = Vec::new();
        for sfx in [
            Sfx::Select,
            Sfx::Start,
            Sfx::Coin,
            Sfx::PowerUp,
            Sfx::NearMiss,
            Sfx::Dash,
            Sfx::Hit,
            Sfx::GameOver,
        ] {
            let wav = to_wav(&synth(sfx));
            sounds.push(load_sound_from_bytes(&wav).await.ok());
        }
        Audio { sounds, muted }
    }

    pub fn play(&self, sfx: Sfx) {
        if self.muted {
            return;
        }
        if let Some(Some(sound)) = self.sounds.get(sfx as usize) {
            play_sound(
                sound,
                PlaySoundParams {
                    looped: false,
                    volume: 0.5,
                },
            );
        }
    }
}

/// A tone segment: frequency sweep from `f0` to `f1` over `dur` seconds.
struct Tone {
    f0: f32,
    f1: f32,
    dur: f32,
    wave: Wave,
}

#[derive(Clone, Copy)]
enum Wave {
    Square,
    Triangle,
    Noise,
}

fn synth(sfx: Sfx) -> Vec<f32> {
    use Wave::*;
    let tones: &[Tone] = match sfx {
        Sfx::Select => &[Tone {
            f0: 660.0,
            f1: 880.0,
            dur: 0.05,
            wave: Square,
        }],
        Sfx::Start => &[
            Tone {
                f0: 440.0,
                f1: 440.0,
                dur: 0.07,
                wave: Square,
            },
            Tone {
                f0: 660.0,
                f1: 660.0,
                dur: 0.07,
                wave: Square,
            },
            Tone {
                f0: 880.0,
                f1: 880.0,
                dur: 0.12,
                wave: Square,
            },
        ],
        Sfx::Coin => &[
            Tone {
                f0: 988.0,
                f1: 988.0,
                dur: 0.05,
                wave: Square,
            },
            Tone {
                f0: 1319.0,
                f1: 1319.0,
                dur: 0.12,
                wave: Square,
            },
        ],
        Sfx::PowerUp => &[
            Tone {
                f0: 300.0,
                f1: 600.0,
                dur: 0.08,
                wave: Triangle,
            },
            Tone {
                f0: 450.0,
                f1: 900.0,
                dur: 0.08,
                wave: Triangle,
            },
            Tone {
                f0: 600.0,
                f1: 1200.0,
                dur: 0.12,
                wave: Triangle,
            },
        ],
        Sfx::NearMiss => &[Tone {
            f0: 1400.0,
            f1: 1800.0,
            dur: 0.06,
            wave: Triangle,
        }],
        Sfx::Dash => &[Tone {
            f0: 2000.0,
            f1: 400.0,
            dur: 0.15,
            wave: Noise,
        }],
        Sfx::Hit => &[
            Tone {
                f0: 3000.0,
                f1: 200.0,
                dur: 0.12,
                wave: Noise,
            },
            Tone {
                f0: 140.0,
                f1: 60.0,
                dur: 0.2,
                wave: Square,
            },
        ],
        Sfx::GameOver => &[
            Tone {
                f0: 440.0,
                f1: 440.0,
                dur: 0.15,
                wave: Triangle,
            },
            Tone {
                f0: 370.0,
                f1: 370.0,
                dur: 0.15,
                wave: Triangle,
            },
            Tone {
                f0: 311.0,
                f1: 311.0,
                dur: 0.15,
                wave: Triangle,
            },
            Tone {
                f0: 262.0,
                f1: 200.0,
                dur: 0.4,
                wave: Triangle,
            },
        ],
    };

    let mut out = Vec::new();
    let mut phase = 0.0f32;
    let mut noise_state: u32 = 0x1234_5678;
    let mut noise_val = 0.0f32;
    for tone in tones {
        let n = (tone.dur * SAMPLE_RATE as f32) as usize;
        for i in 0..n {
            let t = i as f32 / n as f32;
            let freq = tone.f0 + (tone.f1 - tone.f0) * t;
            let prev_phase = phase;
            phase = (phase + freq / SAMPLE_RATE as f32).fract();
            let s = match tone.wave {
                Square => {
                    if phase < 0.5 {
                        1.0
                    } else {
                        -1.0
                    }
                }
                Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
                Noise => {
                    // Sample-and-hold noise: pitch controls how often it changes.
                    if phase < prev_phase {
                        noise_state ^= noise_state << 13;
                        noise_state ^= noise_state >> 17;
                        noise_state ^= noise_state << 5;
                        noise_val = (noise_state as f32 / u32::MAX as f32) * 2.0 - 1.0;
                    }
                    noise_val
                }
            };
            // Short attack, linear decay to avoid clicks.
            let env = (i as f32 / 60.0).min(1.0) * (1.0 - t).powf(0.7);
            out.push(s * env * 0.35);
        }
    }
    out
}

fn to_wav(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
    b.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    b.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        b.extend_from_slice(&v.to_le_bytes());
    }
    b
}
