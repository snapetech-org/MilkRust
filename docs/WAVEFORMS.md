# Stereo and waveform source contract

Native and browser hosts share the bounded 512-frame PCM analyzer. Left/right
waveforms and spectra survive analysis; the band detector averages channel
energy so anti-phase stereo does not disappear. Native generation changes clear
history. The browser uses a separate, inaudible analysis branch, mixes to stereo,
resamples the latest window to 44.1 kHz, and sends a bounded `Float32Array` through
`setPcmStereo`. It disconnects only connections that it owns. The old render API
still accepts mono samples/spectra and duplicates mono into both wave channels.

The classic geometry path implements modes 0–8: circle, spiral, two spirograph
variants, horizontal wave, complex-product wave, angled mono, angled stereo and
spectrum. Stereo lines retain separate strips. It applies scale, recursive audio
smoothing, mystery, position/aspect, volume alpha, color maximization, additive
blend, four-tap path interpolation, pixel-sized dots and thick strokes. Dots use
quads and thick lines use four pixel-offset draws, avoiding driver line-width
extensions. Classic `wave_y` keeps its historical upward orientation.

Custom waves cap the scripted sample count at 512 before allocation or equation
execution. Point inputs are normalized `sample` (and the historical `i` alias),
left/right `value1`/`value2`, default x/y and frame color. Scaling, channel
separation, spectrum selection and forward/backward smoothing run before point
code. Point temporaries persist between points; coordinates/colors reset for each
point. The emitted color follows point r/g/b/a, including alpha. Custom y is
converted from top-down EEL coordinates. Non-finite geometry and evaluation
failures fail the checked frame rather than drawing a silently substituted wave.
A single-point dotted wave is valid.

This is source implementation, not an executed conformance report. Regression
sources cover anti-phase energy, all modes, stereo strip separation, custom point
inputs/colors, hostile counts and portable thick/dot geometry. The existing
browser mock suite now also covers PCM ordering and analysis-graph ownership.
No tests, builds or captures were run for this checkpoint. Reference capture
work must assess amplitude calibration (especially spectrum and mode 3), audio
window alignment, resampling and raster fidelity on each target. The legacy
wave formulas were independently expressed from the MPL-2.0 foo_vis_milk2 source
at `5b44cea79a8678a8f7e516e8c0cce0e4681f299b`; no renderer source from the
noncommercial reference collections was incorporated.
