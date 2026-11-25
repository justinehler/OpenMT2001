//! The final 48b490 outcome call selects a result song after constructing
//! the result scene. Music addresses are pointers to v1.4 mc.exe strings.
use openmt2001::pe::PeImage;
use rodio::Source;
use std::{path::Path, process::Command};

pub struct Assets {
    winner: Vec<f32>,
    loser: Vec<f32>,
}

fn filename(image: &PeImage<'_>, address: u32) -> Result<&'static str, String> {
    let expected = match address {
        0x541d0c => "music_winner.wma",
        0x541cdc => "music_loser.wma",
        _ => return Err(format!("unsupported result music address {address:08x}")),
    };
    let bytes = image.at(address, expected.len() + 1)?;
    if bytes != [expected.as_bytes(), &[0]].concat() {
        return Err(format!("v1.4 result music string changed at {address:08x}"));
    }
    Ok(expected)
}

fn decode(ffmpeg: &Path, path: &Path) -> Result<Vec<f32>, String> {
    let mut command = Command::new(ffmpeg);
    command.args(["-v", "error", "-nostdin", "-i"]).arg(path)
        .args(["-vn", "-ac", "2", "-ar", "44100", "-f", "f32le", "pipe:1"]);
    #[cfg(windows)] {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command.output().map_err(|error| format!("{}: {error}", path.display()))?;
    if !output.status.success() || output.stdout.is_empty() || output.stdout.len() % 8 != 0 {
        return Err(format!("{}: result music decode failed: {}", path.display(),
            String::from_utf8_lossy(&output.stderr)));
    }
    Ok(output.stdout.chunks_exact(4)
        .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap())).collect())
}

impl Assets {
    pub fn load(image: &PeImage<'_>, installer: &Path, ffmpeg: &Path) -> Result<Self, String> {
        let directory = installer.join("Standard/sound/music");
        Ok(Self {
            winner: decode(ffmpeg, &directory.join(filename(image, 0x541d0c)?))?,
            loser: decode(ffmpeg, &directory.join(filename(image, 0x541cdc)?))?,
        })
    }

    fn samples(&self, address: u32) -> Result<&[f32], String> {
        match address {
            0x541d0c => Ok(&self.winner),
            0x541cdc => Ok(&self.loser),
            _ => Err(format!("unsupported result music address {address:08x}")),
        }
    }

    pub fn play(&self, address: u32, music_level: u8) -> Result<(rodio::OutputStream, rodio::Sink), String> {
        let samples = self.samples(address)?;
        let stream = rodio::OutputStreamBuilder::open_default_stream()
            .map_err(|error| format!("result audio device unavailable: {error}"))?;
        let sink = connect(samples, stream.mixer(), music_level);
        Ok((stream, sink))
    }
}

// Share the native nominal music transfer with the era music master.
pub fn set_level(sink: &rodio::Sink, music_level: u8) {
    sink.set_volume(crate::music_gain::amplitude(music_level));
}

fn connect(samples: &[f32], mixer: &rodio::mixer::Mixer, music_level: u8) -> rodio::Sink {
    let sink = rodio::Sink::connect_new(mixer);
    set_level(&sink, music_level);
    // 4ad670 supplies repeat=1 for both final result songs. Keep the one
    // decoded source and Sink alive until their existing owner stops or drops.
    sink.append(rodio::buffer::SamplesBuffer::new(2, 44100, samples.to_vec()).repeat_infinite());
    sink
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_result_music_pointers_and_files() {
        let Some(exe) = std::env::var_os("OPENMT2001_EXE") else { return };
        let Some(suncols) = std::env::var_os("OPENMT2001_SUNCOLS") else { return };
        let exe = std::fs::read(exe).unwrap();
        let image = PeImage::parse(&exe).unwrap();
        let standard = Path::new(&suncols).parent().unwrap();
        for address in [0x541d0c, 0x541cdc] {
            let name = filename(&image, address).unwrap();
            assert!(standard.join("sound/music").join(name).is_file());
        }
        assert!(filename(&image, 0x542c20).is_err());
        // 48b490 selects the final song after constructing the result scene
        // in all three branches. The constructor's optional music_intro.wma
        // call at 4b190a precedes this final selection.
        for (construct, music, song) in [
            (0x48b4d2, 0x48b4f0, 0x541d0c),
            (0x48b500, 0x48b50f, 0x541d0c),
            (0x48b54b, 0x48b55a, 0x541cdc),
        ] {
            let call_target = |address: u32| {
                let code = image.at(address, 5).unwrap();
                assert_eq!(code[0], 0xe8);
                address.wrapping_add(5).wrapping_add_signed(
                    i32::from_le_bytes(code[1..5].try_into().unwrap()))
            };
            assert_eq!(call_target(construct), 0x4b1800);
            assert_eq!(call_target(music), 0x4ad670);
            let push = image.at(music - 10, 5).unwrap();
            assert_eq!(push[0], 0x68);
            assert_eq!(u32::from_le_bytes(push[1..5].try_into().unwrap()), song);
        }
        let installer = standard.parent().unwrap();
        let ffmpeg = std::env::var_os("OPENMT2001_FFMPEG").map(std::path::PathBuf::from)
            .unwrap_or_else(|| if cfg!(windows) {
                "work/python/Lib/site-packages/imageio_ffmpeg/binaries/ffmpeg-win-x86_64-v7.1.exe".into()
            } else { "ffmpeg".into() });
        let assets = Assets::load(&image, installer, &ffmpeg).unwrap();
        let seconds = |samples: &[f32]| samples.len() as f64 / (44100. * 2.);
        assert!((17.0..19.0).contains(&seconds(&assets.winner)), "winner: {}", seconds(&assets.winner));
        assert!((11.0..13.0).contains(&seconds(&assets.loser)), "loser: {}", seconds(&assets.loser));
    }
}

#[cfg(test)]
mod level_tests {
    use super::*;

    #[test]
    fn both_results_apply_native_gain_before_first_queued_sample() {
        let assets = Assets { winner: vec![0.5; 32768], loser: vec![0.25; 32768] };
        // Original music25 -> -2920, music50 -> -1787 hundredths dB.
        // These fixed PCM expectations come from those commands and the
        // synthetic source amplitudes, not from production gain helpers.
        for (address, level, expected) in [
            (0x541d0c, 25, 0.017336843),
            (0x541cdc, 25, 0.008668421),
            (0x541d0c, 50, 0.0638955),
            (0x541cdc, 50, 0.03194775),
        ] {
            let (output, mut rendered) = rodio::mixer::mixer(2, 44100);
            let sink = connect(assets.samples(address).unwrap(), &output, level);
            let first: Vec<_> = rendered.by_ref().take(4096).collect();
            assert_eq!(first.len(), 4096);
            assert!(first.iter().all(|sample| *sample == 0. || (*sample - expected).abs() < 1e-6));
            assert!(first[2048..].iter().all(|sample| (*sample - expected).abs() < 1e-6));
            assert!(!sink.empty());
        }
    }

    #[test]
    fn muted_result_is_silent_from_first_sample_for_both_songs() {
        let assets = Assets { winner: vec![0.5; 32768], loser: vec![0.25; 32768] };
        for address in [0x541d0c, 0x541cdc] {
            let (output, mut rendered) = rodio::mixer::mixer(2, 44100);
            let sink = connect(assets.samples(address).unwrap(), &output, 0);
            let first: Vec<_> = rendered.by_ref().take(4096).collect();
            assert_eq!(first.len(), 4096);
            assert!(first.iter().all(|sample| *sample == 0.));
            assert!(!sink.empty(), "muting preserves the selected stream");
        }
        assert!(assets.samples(0x542c20).is_err());
    }

    #[test]
    fn live_result_gain_changes_output_on_the_same_sink() {
        let (output, mut rendered) = rodio::mixer::mixer(2, 44100);
        let sink = connect(&vec![0.5; 32768], &output, 100);
        let first: Vec<_> = rendered.by_ref().take(4096).collect();
        assert_eq!(first.len(), 4096);
        // rodio's connected Mixer can prebuffer silence while its queue is
        // still empty. Require bounded startup and the actual subsequent gain.
        assert!(first.iter().all(|sample| *sample == 0. || (*sample - 0.5).abs() < 1e-6));
        assert!(first[2048..].iter().all(|sample| (*sample - 0.5).abs() < 1e-6));
        let first_position = sink.get_pos();
        set_level(&sink, 0);
        let muted: Vec<_> = rendered.by_ref().take(4096).collect();
        assert_eq!(muted.len(), 4096);
        assert!(muted[2048..].iter().all(|sample| *sample == 0.));
        assert!(sink.get_pos() > first_position, "muting advances the same queued source");
        let muted_position = sink.get_pos();
        set_level(&sink, 25);
        let quieter: Vec<_> = rendered.by_ref().take(4096).collect();
        assert_eq!(quieter.len(), 4096);
        // Original g25 -> command -2920 -> nominal gain 0.034673685.
        assert!(quieter[2048..].iter().all(|sample| (*sample - 0.017336843).abs() < 1e-7));
        assert!(sink.get_pos() > muted_position, "changing gain preserves forward playback");
        let quieter_position = sink.get_pos();
        set_level(&sink, 50);
        let middle: Vec<_> = rendered.by_ref().take(4096).collect();
        assert_eq!(middle.len(), 4096);
        assert!(middle[2048..].iter().all(|sample| (*sample - 0.0638955).abs() < 1e-6));
        assert!(sink.get_pos() > quieter_position, "interior gain changes preserve forward playback");
        set_level(&sink, 255);
        let clamped: Vec<_> = rendered.by_ref().take(4096).collect();
        assert_eq!(clamped.len(), 4096);
        assert!(clamped[2048..].iter().all(|sample| (*sample - 0.5).abs() < 1e-6));
        sink.stop();
        let stopped: Vec<_> = rendered.by_ref().take(4096).collect();
        assert!(stopped[stopped.len().min(2048)..].iter().all(|sample| *sample == 0.));
    }
}

#[cfg(test)]
mod repeat_tests {
    use super::*;

    // Each song is 512 stereo frames (1024 interleaved samples), with a
    // different second half. These sources end several times within each
    // fixed observation window. The loser fixture is half the winner.
    const SONG_SAMPLES: usize = 1024;
    const WINDOW: usize = 4096;
    const SETTLED: usize = 2048;

    fn assets() -> Assets {
        Assets {
            winner: [[0.5, -0.25].repeat(256), [-0.125, 0.375].repeat(256)].concat(),
            loser: [[0.25, -0.125].repeat(256), [-0.0625, 0.1875].repeat(256)].concat(),
        }
    }

    fn close(actual: f32, expected: f32) -> bool {
        (actual - expected).abs() < 1e-6
    }

    fn assert_phase(pcm: &[f32], halves: [[f32; 2]; 2], phase: usize) {
        for (index, actual) in pcm.iter().enumerate() {
            let position = (phase + index) % SONG_SAMPLES;
            let expected = halves[position / (SONG_SAMPLES / 2)][position % 2];
            assert!(close(*actual, expected),
                "sample {index}, phase {phase}: {actual} != {expected}");
        }
    }

    // Find the second-half -> first-half stereo boundary within one fixed
    // period; then validate EVERY sample of this already-settled window.
    // This admits rodio's initial queue metadata conversion without choosing
    // a phase from production gain/source helpers or accepting filler silence.
    fn assert_repeat(pcm: &[f32], halves: [[f32; 2]; 2], after_boundary: usize) -> usize {
        let boundary = (2..=SONG_SAMPLES).step_by(2).find(|&index| {
            pcm[index - 2..index].iter().zip(halves[1]).all(|(a, b)| close(*a, b))
                && pcm[index..index + 2].iter().zip(halves[0]).all(|(a, b)| close(*a, b))
        }).expect("nonzero stereo song must wrap within one source length");
        assert!(pcm.len() >= boundary + after_boundary);
        let phase = (SONG_SAMPLES - boundary % SONG_SAMPLES) % SONG_SAMPLES;
        assert_phase(pcm, halves, phase);
        (phase + pcm.len()) % SONG_SAMPLES
    }

    fn take(rendered: &mut rodio::mixer::MixerSource, count: usize) -> Vec<f32> {
        let pcm: Vec<_> = rendered.by_ref().take(count).collect();
        assert_eq!(pcm.len(), count, "the retained playing queue must stay live");
        pcm
    }

    #[test]
    fn both_result_songs_repeat_with_live_gain_on_one_sink_until_stop() {
        let assets = assets();
        // Literal expected PCM: g25 -> -2920 hundredths dB, nominal gain
        // 0.034673685; no call to the production gain function for the oracle.
        for (address, full, quiet) in [
            (0x541d0c, [[0.5, -0.25], [-0.125, 0.375]],
                [[0.017336843, -0.008668421], [-0.0043342105, 0.013002632]]),
            (0x541cdc, [[0.25, -0.125], [-0.0625, 0.1875]],
                [[0.008668421, -0.0043342105], [-0.0021671053, 0.006501316]]),
        ] {
            let (output, mut rendered) = rodio::mixer::mixer(2, 44100);
            let sink = connect(assets.samples(address).unwrap(), &output, 25);
            let owner = std::ptr::from_ref(&sink);
            assert!(close(sink.volume(), 0.034673685));
            assert_eq!(sink.len(), 1);
            let startup = take(&mut rendered, WINDOW);
            let peak = quiet.iter().flatten().copied().map(f32::abs).fold(0.0_f32, f32::max);
            assert!(startup.iter().all(|sample| sample.is_finite() && sample.abs() <= peak + 1e-6),
                "gain must apply before append, including the interpolated metadata prefix");
            // Three periods allow one period to locate a boundary and at
            // least two complete source traversals after it. Finite append
            // or keep-alive silence cannot satisfy the literal PCM oracle.
            let repeated = take(&mut rendered, 3 * SONG_SAMPLES);
            let phase = assert_repeat(&repeated, quiet, 2 * SONG_SAMPLES);
            assert_eq!(sink.len(), 1, "repetition is one queued source");
            let position = sink.get_pos();
            set_level(&sink, 0);
            assert_eq!(std::ptr::from_ref(&sink), owner);
            assert_eq!(sink.get_pos(), position, "setting gain must not seek");
            assert_eq!(sink.len(), 1);
            let muted = take(&mut rendered, WINDOW);
            assert!(muted[SETTLED..].iter().all(|sample| *sample == 0.));
            assert!(sink.get_pos() > position, "muted repeated source still advances");
            let position = sink.get_pos();
            set_level(&sink, 100);
            assert_eq!(std::ptr::from_ref(&sink), owner);
            assert_eq!(sink.get_pos(), position);
            assert_eq!(sink.len(), 1);
            let restored = take(&mut rendered, WINDOW);
            // WINDOW and SETTLED are whole source periods: preserving the
            // established phase catches restarting at a volume change.
            assert_phase(&restored[SETTLED..], full, phase);
            assert!(sink.get_pos() > position);
            assert_eq!(sink.len(), 1);
            sink.stop();
            let stopped = take(&mut rendered, WINDOW);
            assert!(stopped[SETTLED..].iter().all(|sample| *sample == 0.));
            assert!(sink.empty());
            assert_eq!(sink.len(), 0);
            drop(sink);
            assert!(rendered.by_ref().take(WINDOW).count() < WINDOW,
                "dropping the stopped owner must terminate its mixer queue");
            assert_eq!(rendered.next(), None);
        }
    }

    #[test]
    fn both_result_songs_repeat_while_initially_muted_and_drop_terminates() {
        let assets = assets();
        for (address, full) in [
            (0x541d0c, [[0.5, -0.25], [-0.125, 0.375]]),
            (0x541cdc, [[0.25, -0.125], [-0.0625, 0.1875]]),
        ] {
            let (output, mut rendered) = rodio::mixer::mixer(2, 44100);
            let sink = connect(assets.samples(address).unwrap(), &output, 0);
            assert_eq!(sink.volume(), 0.);
            let first = take(&mut rendered, WINDOW);
            assert!(first.iter().all(|sample| *sample == 0.));
            let position = sink.get_pos();
            let beyond_ends = take(&mut rendered, 3 * SONG_SAMPLES);
            assert!(beyond_ends.iter().all(|sample| *sample == 0.));
            assert_eq!(sink.len(), 1);
            assert!(sink.get_pos() > position);
            let position = sink.get_pos();
            set_level(&sink, 100);
            assert_eq!(sink.get_pos(), position);
            let restored = take(&mut rendered, WINDOW);
            assert_repeat(&restored[SETTLED..], full, SONG_SAMPLES);
            assert_eq!(sink.len(), 1);
            assert!(sink.get_pos() > position);
            // Drop an actively repeating owner, without calling stop first.
            drop(sink);
            let tail: Vec<_> = rendered.by_ref().take(WINDOW).collect();
            assert!(tail.len() < WINDOW, "active owner drop must settle within the fixed window");
            assert!(tail[tail.len().min(SETTLED)..].iter().all(|sample| *sample == 0.));
            assert_eq!(rendered.next(), None, "no orphan repeat or silence queue survives");
        }
    }

    #[test]
    fn unsupported_result_address_is_rejected_before_opening_output() {
        let assets = assets();
        match assets.play(0x542c20, 100) {
            Err(error) => assert_eq!(error, "unsupported result music address 00542c20"),
            Ok(_) => panic!("intro address must not be accepted as a result song"),
        }
    }
}
