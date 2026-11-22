# DWP playback pitch investigation

**Status: the supplied recording comparison reproduces an approximately one-octave output difference; the cause is not yet localized. No pitch fix made.** The Drive ZIP supplied the four recordings, but did not include the corresponding DWP programs, source samples or project/MIDI files. The output-pitch difference is measured below; the requested controlled same-source, multi-instrument validation is therefore still outstanding.

## Findings from source

### DWP parsing and zones

`crates/audio/src/dwp.rs` parses each `.dwp` zone's `0x1F4` record into `root`, key/velocity bounds, gain and pan. It reads loop bounds/mode from `0x1F7`, attack/release/envelope values from `0x1FD`, and an embedded audio payload from `0x205` or `0x206`. The parser leaves DWP `tune` at 0 and `keytrack` at 100 cents/key. There is no DWP octave/semitone offset applied during parsing. `0x1F7`'s declared sample rate is used for embedded PCM; external sample files use the sample decoder's rate. Loop/sample span is normalized in `crates/audio/src/dw_sampler.rs::Playable::new`.

### MIDI pitch and sample playback

The DW sampler selects the zone using the note plus the configured Pitch offset (`DwSampler::pick`). During playback, the sample increment is computed in `dw_sampler.rs::process` as:

`sample_rate / output_rate × 2^(((MIDI key − zone root) × keytrack + zone tune) / 1200 + pitch-envelope octaves + LFO octaves) × Pitch/Fine/bend ratio`.

At the zone root, default Pitch/Fine/bend, and a neutral pitch envelope/LFO, this yields the sample's native rate corrected only for output sample rate. No global octave factor is apparent in this path. Zone root and keytrack are correctly used by the implementation as MIDI-note units and cents per key, respectively. The CSTM PB-range affects bend range, not ordinary note pitch.

The current code may still differ from FL Studio Mobile because the DWP field interpretation, a source instrument's stored metadata, a preset/module parameter, or a note-number mapping could be wrong. The code audit alone cannot distinguish those possibilities. In particular, the app's “Set middle C to C4” setting changes note labels, not the MIDI pitch; comparing displayed octave labels between apps is not sufficient to establish equal MIDI note numbers.

### Existing tests and their limits

`crates/audio/tests/dw_sampler.rs::pitch_follows_the_step_law` uses synthetic 44.1 kHz sine samples and checks native/root pitch, semitone pitch, a zone change and Pitch/Fine behavior at a 48 kHz output rate. Other DW sampler tests cover zone selection, velocity, loops/envelopes and rendering. `crates/audio/tests/dwp.rs` tests parser fields and sample decoding. These establish internal behavior against the test fixture's assumed DWP layout; they do **not** validate real factory DWP metadata or match FL Studio Mobile's rendering.

## Supplied recording analysis

The Drive ZIP contains four recordings, not raw DWP samples: Bana Forge outputs are 48 kHz stereo PCM WAV; FL Studio Mobile outputs are 44.1 kHz stereo MP3. I decoded the files and measured Hann-windowed 0.4-second FFT peaks in sustained sections. Results (strongest spectral peak in 100–600 Hz):

| Pair | FL Studio Mobile peak | Bana Forge peak | Ratio |
|---|---:|---:|---:|
| Default, 3.50–3.90 s | 130.74 Hz | 260.74 Hz | 1.994× |
| Default, 4.00–4.40 s | 130.53 Hz | 260.97 Hz | 2.000× |
| Trance Delay, 3.50–3.90 s | 130.74 Hz | 260.74 Hz | 1.994× |
| Trance Delay, 4.00–4.40 s | 130.53 Hz | 260.97 Hz | 2.000× |

The same near-exact 2× difference in both pairs confirms that the supplied outputs sound about one octave apart in these sustained sections. The effect setting does not materially change the pitch discrepancy. It is **not proof of a sampler pitch bug**: recordings alone cannot establish identical MIDI note numbers, source preset/sample-zone, tuning, velocity, sample rate, note timing or app settings; MP3 and WAV can have different latency/length. Default recording duration is 4.546 s (FLM MP3) vs 4.626 s (Bana Forge WAV); the delay pair is 6.610 s vs 6.689 s.

## What is not established

- Whether the two apps rendered the exact same source instrument/program and sample zone at identical MIDI note number, velocity, note length, pitch/fine/bend settings and output sample rate.
- Whether the approximately 130 Hz vs 261 Hz peaks correspond to the same fundamental and musical section rather than a different octave/zone/attack/harmonic selection in the test setup. The stable near-2× ratio makes an octave discrepancy in the supplied output likely, but does not identify its cause.
- Whether the issue is DWP root parsing, source preset metadata, note-number mapping, sample-rate metadata, or an audio-engine regression.
- Whether ordinary WAV/MP3 playback is unchanged on device. The code has distinct regular audio-clip and DW-sampler paths, but these recordings are not raw controlled test assets and no device playback validation was performed.

## Next evidence required before an audio change

Provide the exact `.dwp` files for at least two affected instruments plus every referenced sample file (or the self-contained files if embedded), the four original audio recordings, and ideally the equivalent FL Studio Mobile project/export with MIDI note numbers or a MIDI file. Include each app's pitch/fine/bend settings, velocity, sample rate/output rate and note-on/off times. With those, compare the same source, zone, MIDI key and duration at multiple keys, including keys on both sides of zone boundaries, then compare looped/enveloped presets. Until then, do not add a global +/−12 semitone correction or change DWP root semantics: either can break correctly mapped instruments.

## Playtest APK

No APK was built. `android/RELEASE_HOLD.md` currently forbids building/distributing an APK pending reviewed migration/update safety and exact-candidate device validation. This investigation does not satisfy those gates; do not bypass or remove the hold to produce a link. The Android private/public build and testing requirements are recorded in that hold and `ANDROID_NEW_IDENTITY_PLAN.md`.
