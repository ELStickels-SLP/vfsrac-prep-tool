# `oxifft` crate notes

Reference notes on the parts of `oxifft` (v0.3.2) that `pitch_shift.rs`
uses, so this doesn't need re-deriving from the vendored source each time.
Source lives locally at
`~/.cargo/registry/src/index.crates.io-*/oxifft-0.3.2/` if deeper digging
is ever needed.

## `Complex<T>` (`src/kernel/complex.rs`)

Plain `{ re: T, im: T }` struct. Useful methods:

- `norm_sqr(self) -> T` — squared magnitude `re*re + im*im` (cheap, no
  sqrt).
- `norm(self) -> T` — magnitude `sqrt(norm_sqr())`.
- `arg(self) -> T` — phase angle via `atan2(im, re)`.
- `conj`, `scale`, `inv`, arithmetic operator overloads.

No built-in "total energy of a spectrum" helper — sum `norm_sqr()` (or
`norm()`) over bins yourself.

## `rfft` (`src/api/plan/functions.rs`)

```rust
pub fn rfft<T: Float>(input: &[T]) -> Vec<Complex<T>>
```

Real-to-complex forward FFT, **unnormalized** (no `1/N` scaling — unlike
`ifft`, which does normalize). N real samples -> N/2+1 complex bins,
bin 0 is DC. Magnitudes scale with both input amplitude and window length,
so any energy/magnitude threshold compared against `rfft` output needs to
account for `analysis_win_length`, not just be a fixed constant.

## `resample` (`src/signal/resample.rs`)

```rust
pub fn resample<T: Float>(signal: &[T], new_len: usize) -> Vec<T>
```

Simple resampling to `new_len` samples (used in `shift_pitch_window` to
stretch/compress the analysis window before the interpolation pass).
