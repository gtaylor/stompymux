//! Seeded value noise and fractal sums of it, sampled at hex centers to shape elevation,
//! moisture and other smooth fields.
use crate::rng::mix;

/// Smooth 2D value noise with a fixed seed.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Noise {
    seed: u64,
}

impl Noise {
    /// Noise derived from a map seed and a field label.
    pub(crate) fn new(seed: u64, label: &str) -> Self {
        let mut rng = crate::rng::Rng::stream(seed, label);
        Self {
            seed: rng.next_u64(),
        }
    }

    /// Lattice value in `[0, 1)` at integer point `(x, y)`.
    fn lattice(self, x: i64, y: i64) -> f64 {
        let hash = mix(self.seed ^ mix(x as u64 ^ mix(y as u64)));
        (hash >> 11) as f64 / (1_u64 << 53) as f64
    }

    /// Value noise in `[0, 1)` at `(x, y)`, smoothly interpolated between lattice points.
    pub(crate) fn value(self, x: f64, y: f64) -> f64 {
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (smooth(x - x0), smooth(y - y0));
        let (ix, iy) = (x0 as i64, y0 as i64);
        let top = lerp(self.lattice(ix, iy), self.lattice(ix + 1, iy), fx);
        let bottom = lerp(self.lattice(ix, iy + 1), self.lattice(ix + 1, iy + 1), fx);
        lerp(top, bottom, fy)
    }

    /// Fractal noise in `[0, 1)`: `octaves` layers of value noise, each at twice the frequency
    /// and half the weight of the last. `scale` is the size of the largest features.
    pub(crate) fn fractal(self, x: f64, y: f64, scale: f64, octaves: u32) -> f64 {
        let (mut total, mut weight, mut frequency, mut amplitude) = (0.0, 0.0, 1.0 / scale, 1.0);
        for octave in 0..octaves {
            // Offset each octave so lattice points do not line up.
            let shift = f64::from(octave) * 17.31;
            total += amplitude * self.value(x * frequency + shift, y * frequency - shift);
            weight += amplitude;
            frequency *= 2.0;
            amplitude *= 0.5;
        }
        total / weight
    }
}

/// Linear interpolation from `a` to `b`.
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Smoothstep easing so noise has no visible lattice creases.
fn smooth(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_bounded_smooth_and_seeded() {
        let noise = Noise::new(3, "elevation");
        for step in 0..200 {
            let x = f64::from(step) * 0.37;
            let value = noise.fractal(x, x * 0.5, 8.0, 4);
            assert!((0.0..1.0).contains(&value));
            let near = noise.fractal(x + 0.01, x * 0.5, 8.0, 4);
            assert!((value - near).abs() < 0.05);
        }
        let other = Noise::new(4, "elevation");
        assert_ne!(noise.value(1.5, 2.5), other.value(1.5, 2.5));
    }
}
