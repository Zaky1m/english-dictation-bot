//! Cleans artifacts the TTS model adds around the speech: a short pop at the
//! start and a full-scale burst at the end, each separated from the speech by
//! silence. Both sound like a radio being switched on and off.

const WINDOW: usize = 240; // 10 ms at 24 kHz
const SILENCE_RMS: f64 = 150.0;
// The burst is always clipped at full scale; the pop is lower but still above normal speech.
const POP_MIN_PEAK: i32 = 15_000;
const BURST_MIN_PEAK: i32 = 30_000;
const MIN_GAP_WINDOWS: usize = 12;
const MAX_POP_WINDOWS: usize = 8;
const MAX_BURST_WINDOWS: usize = 20;
const KEEP_LEAD_WINDOWS: usize = 10;
const KEEP_TAIL_WINDOWS: usize = 20;

#[derive(Clone, Copy)]
struct Win {
    loud: bool,
    peak: i32,
}

fn windows(samples: &[i16]) -> Vec<Win> {
    samples
        .as_chunks::<WINDOW>()
        .0
        .iter()
        .map(|w| {
            let sum: f64 = w.iter().map(|&x| f64::from(x).powi(2)).sum();
            Win {
                loud: (sum / WINDOW as f64).sqrt() > SILENCE_RMS,
                peak: w.iter().map(|&x| i32::from(x).abs()).max().unwrap_or(0),
            }
        })
        .collect()
}

fn max_peak(wins: &[Win]) -> i32 {
    wins.iter().map(|w| w.peak).max().unwrap_or(0)
}

/// Loud runs as half-open window ranges.
fn loud_runs(wins: &[Win]) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let mut i = 0;
    while i < wins.len() {
        if wins[i].loud {
            let start = i;
            while i < wins.len() && wins[i].loud {
                i += 1;
            }
            runs.push((start, i));
        } else {
            i += 1;
        }
    }
    runs
}

/// Short, isolated and above `min_peak`: normal speech never peaks this high in such a blip.
fn is_artifact(run: &[Win], min_peak: i32) -> bool {
    max_peak(run) >= min_peak
}

/// Sample range worth keeping.
fn keep_range(samples: &[i16]) -> (usize, usize) {
    let wins = windows(samples);
    let runs = loud_runs(&wins);
    let whole = (0, samples.len());
    if runs.len() < 2 {
        return whole;
    }
    let (first, last) = (runs[0], runs[runs.len() - 1]);
    let lead_candidate =
        first.1 - first.0 <= MAX_POP_WINDOWS && runs[1].0 - first.1 >= MIN_GAP_WINDOWS;
    let tail_candidate =
        last.1 - last.0 <= MAX_BURST_WINDOWS && last.0 - runs[runs.len() - 2].1 >= MIN_GAP_WINDOWS;
    let speech_start = runs[usize::from(lead_candidate)].0;
    let speech_end = runs[runs.len() - 1 - usize::from(tail_candidate)].1;

    let mut start = 0;
    let mut end = samples.len();
    if lead_candidate && is_artifact(&wins[first.0..first.1], POP_MIN_PEAK) {
        start = first.1.max(speech_start.saturating_sub(KEEP_LEAD_WINDOWS)) * WINDOW;
    }
    if tail_candidate && is_artifact(&wins[last.0..last.1], BURST_MIN_PEAK) {
        end = last.0.min(speech_end + KEEP_TAIL_WINDOWS) * WINDOW;
    }
    (start, end.max(start))
}

/// Returns the PCM (s16le) with the start pop and end burst removed.
pub fn clean(pcm: &[u8]) -> Vec<u8> {
    let samples: Vec<i16> = pcm
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_le_bytes(*b))
        .collect();
    let (start, end) = keep_range(&samples);
    pcm[start * 2..end * 2].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(ms: usize, amp: f64) -> Vec<i16> {
        (0..ms * 24)
            .map(|i| (amp * (i as f64 * 0.3).sin()) as i16)
            .collect()
    }

    fn quiet(ms: usize) -> Vec<i16> {
        vec![3; ms * 24]
    }

    fn bytes(parts: &[Vec<i16>]) -> Vec<u8> {
        parts
            .iter()
            .flatten()
            .flat_map(|s| s.to_le_bytes())
            .collect()
    }

    fn ms(pcm: &[u8]) -> usize {
        pcm.len() / 2 / 24
    }

    #[test]
    fn removes_start_pop_and_end_burst() {
        let pcm = bytes(&[
            quiet(20),
            tone(40, 25_000.0),
            quiet(250),
            tone(600, 5_000.0),
            quiet(300),
            tone(140, 32_000.0),
        ]);
        let cleaned = clean(&pcm);
        // 100 ms lead silence + 600 ms speech + 200 ms tail silence
        assert_eq!(ms(&cleaned), 100 + 600 + 200);
        let samples: Vec<i16> = cleaned
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| i16::from_le_bytes(*b))
            .collect();
        assert!(samples.iter().all(|s| s.abs() <= 5_000));
    }

    #[test]
    fn short_silence_is_kept_whole() {
        let pcm = bytes(&[
            quiet(20),
            tone(40, 25_000.0),
            quiet(60),
            tone(600, 5_000.0),
            quiet(60),
            tone(140, 32_000.0),
        ]);
        // Gaps under 80 ms mean this is not a pop or burst; leave it alone.
        assert_eq!(clean(&pcm), pcm);
    }

    #[test]
    fn clean_speech_is_unchanged() {
        let pcm = bytes(&[quiet(200), tone(800, 5_000.0), quiet(300)]);
        assert_eq!(clean(&pcm), pcm);
    }

    #[test]
    fn loud_speech_ending_is_not_mistaken_for_a_burst() {
        // The last word is only slightly louder than the rest: speech, not an artifact.
        let pcm = bytes(&[
            quiet(100),
            tone(500, 8_000.0),
            quiet(300),
            tone(150, 12_000.0),
        ]);
        assert_eq!(clean(&pcm), pcm);
    }

    #[test]
    fn only_the_burst_is_removed_when_there_is_no_pop() {
        let pcm = bytes(&[
            quiet(300),
            tone(500, 5_000.0),
            quiet(400),
            tone(140, 32_000.0),
        ]);
        let cleaned = clean(&pcm);
        assert_eq!(ms(&cleaned), 300 + 500 + 200);
    }

    #[test]
    fn empty_and_silent_input_do_not_panic() {
        assert!(clean(&[]).is_empty());
        let silent = bytes(&[quiet(500)]);
        assert_eq!(clean(&silent), silent);
    }
}
