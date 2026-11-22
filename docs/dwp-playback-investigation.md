# DWP playback pitch investigation

**Status: code-path audit complete; reported FL Studio Mobile discrepancy is not yet reproduced. No pitch fix made.** The four referenced recordings and the corresponding DWP/sample source files are not present in this checkout, so a controlled cross-application comparison and the requested multi-instrument validation cannot be performed yet.

## Findings from source

### DWP parsing and zones

`crates/audio/src/dwp.rs` parses each `.dwp` zone's `0x1F4` record into `root`, key/velocity bounds, gain and pan. It reads loop bounds/mode from `0x1F7`, attack/release/envelope values from `0x1FD`, and an embedded audio payload from `0x205` or `0x206`. The parser leaves DWP `tune` at 0 and `keytrack` at 100 cents/key. There is no DWP octave/semitone offset applied during parsing. `0x1F7`'s declared sample rate is used for embedded PCM; external sample files use the sample decoder's rate. Loop/sample span is normalized in `crates/audio/src/dw_sampler.rs::Playable::new`.

### MIDI pitch and sample playback

The DW sampler selects the zone using the note plus the configured Pitch offset (`DwSampler::pick`). During playback, the sample increment is computed in `dw_sampler.rs::process` as:

`sample_rate / output_rate × 2^(((MIDI key − zone root) × keytrack + zone tune) / 1200 + pitch-envelope octaves + LFO octaves) × Pitch/Fine/bend ratio`.

At the zone root, default Pitch/Fine/bend, and a neutral pitch envelope/LFO, this yields the sample's native rate corrected only for output sample rate. No global octave factor is apparent in this path. Zone root and keytrack are correctly used by the implementation as MIDI-note units and cents per key, respectively. The CSTM PB-range affects bend range, not ordinary note pitch.

The current code may still differ from FL Studio Mobile because the DWP field interpretation, a source instrument's stored metadata, a preset/module parameter, or a note-number mapping could be wrong. The code audit alone cannot distinguish those possibilities. In particular, the app's “Set middle C to C4” setting changes note labels, not the MIDI pitch; comparing displayed octave labels between apps is not sufficient to establish equal MIDI note numbers.

### Existing tests and their limits

`crates/audio/tests/dw_sampler.rs::pitch_follows_the_step_law` uses synthetic 44.1 kHz sine samples and checks native/root pitch, semitone pitch, a zone change and Pitch/Fine behavior at a 48 kHz output rate. Other DW sampler tests cover zone selection, velocity, loops/envelopes and rendering. `crates/audio/tests/dwp.rs` tests parser fields and sample decoding. These establish internal behavior against the test fixture's assumed DWP layout; they do **not** validate real factory DWP metadata or match FL Studio Mobile's rendering. No actual DWP presets, WAV/MP3 comparison files or attached recordings were available in the working tree.

## What is not established

- Whether either recording pair contains the same exact source instrument/program, zone, MIDI key number, velocity, note length, pitch/fine settings and sample rate.
- Whether the approximately 130 Hz vs 261 Hz representative sections are fundamental frequencies of equivalent notes/sections, rather than a different octave/zone/attack or harmonic selection.
- Whether the issue is DWP root parsing, source preset metadata, note/octave selection, sample-rate metadata, or an audio-engine regression.
- Whether WAV/MP3 playback is unchanged on device. The code has distinct regular audio-clip and DW-sampler paths, but no supplied controlled test sources or device playback were available here.

## Next evidence required before an audio change

Provide the exact `.dwp` files for at least two affected instruments plus every referenced sample file (or the self-contained files if embedded), the four original audio recordings, and ideally the equivalent FL Studio Mobile project/export with MIDI note numbers or a MIDI file. Include each app's pitch/fine/bend settings, velocity, sample rate/output rate and note-on/off times. With those, compare the same source, zone, MIDI key and duration at multiple keys, including keys on both sides of zone boundaries, then compare looped/enveloped presets. Until then, do not add a global +/−12 semitone correction or change DWP root semantics: either can break correctly mapped instruments.

## Playtest APK

No APK was built. `android/RELEASE_HOLD.md` currently forbids building/distributing an APK pending reviewed migration/update safety and exact-candidate device validation. This investigation does not satisfy those gates; do not bypass or remove the hold to produce a link. The Android private/public build and testing requirements are recorded in that hold and `ANDROID_NEW_IDENTITY_PLAN.md`.
