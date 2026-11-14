//! Falling-particle overlay for seasonal themes (snow, petals, leaves), a port of
//! the iOS app's `ParticleOverlay`: each particle falls linearly from above the
//! window to below it, sways sideways and rocks with eased sine motion, and
//! starts after a random delay. Positions are a pure function of time, so the
//! shell only has to repaint on a timer.

use crate::theme::Overlay;

pub struct Particle {
    pub text: &'static str,
    pub size: f32,
    x: f32,
    delay: f32,
    fall: f32,
    sway: f32,
    sway_period: f32,
    rotation: f32,
    rotation_period: f32,
    pub opacity: f32,
}

/// A particle's state at one instant, in window coordinates (DIPs).
#[derive(Debug, Clone, Copy)]
pub struct Sprite {
    pub index: usize,
    pub x: f32,
    pub y: f32,
    pub rotation: f32,
}

pub struct Particles {
    pub items: Vec<Particle>,
    top: f32,
    pub color: Option<(u32, u32)>,
}

/// xorshift32 — deterministic, no dependency.
struct Rng(u32);
impl Rng {
    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }
    fn range(&mut self, (a, b): (f32, f32)) -> f32 {
        a + self.next() * (b - a)
    }
}

/// Eased ping-pong between 0 and 1 (`withRepeat(withTiming(..., inOut(sin)), -1, true)`).
fn swing(t: f32, period: f32) -> f32 {
    if period <= 0.0 {
        return 0.0;
    }
    let cycle = (t / period).rem_euclid(2.0);
    let p = if cycle < 1.0 { cycle } else { 2.0 - cycle };
    (1.0 - (std::f32::consts::PI * p).cos()) / 2.0
}

impl Particles {
    pub fn new(cfg: &Overlay, width: f32, seed: u32) -> Self {
        let mut rng = Rng(seed | 1);
        let items = (0..cfg.count)
            .map(|_| {
                let fall = rng.range(cfg.duration);
                let pick = ((rng.next() * cfg.particles.len() as f32) as usize).min(cfg.particles.len() - 1);
                let p = Particle {
                    text: cfg.particles[pick],
                    size: rng.range(cfg.size),
                    x: rng.next() * width,
                    delay: rng.next() * 10_000.0,
                    fall,
                    sway: rng.range(cfg.sway),
                    sway_period: fall / cfg.sway_divisor,
                    rotation: rng.range(cfg.rotation),
                    rotation_period: fall / cfg.rotation_divisor,
                    opacity: rng.range(cfg.opacity),
                };
                rng.next(); // was "behind the text?": keeps the following particles as they were
                p
            })
            .collect();
        Particles { items, top: cfg.size.1 + 10.0, color: cfg.color }
    }

    /// Visible sprites at `ms` milliseconds since the overlay started.
    pub fn sprites(&self, ms: f32, height: f32) -> Vec<Sprite> {
        self.items
            .iter()
            .enumerate()
            .filter_map(|(index, p)| {
                let t = ms - p.delay;
                if t < 0.0 {
                    return None;
                }
                let travel = height + self.top;
                let y = -self.top + (t % p.fall) / p.fall * (travel + self.top);
                Some(Sprite { index, x: p.x + p.sway * swing(t, p.sway_period), y, rotation: p.rotation * swing(t, p.rotation_period) })
            })
            .collect()
    }

    /// Each particle's motion from `ms` on, as the segments a compositor can play by
    /// itself (DirectComposition animations): `sprites` at `ms + τ` equals
    /// [`Track::at`]`(τ)` for every visible sprite.
    pub fn tracks(&self, ms: f32, height: f32) -> Vec<Track> {
        let span = height + 2.0 * self.top;
        self.items
            .iter()
            .map(|p| {
                let t = ms - p.delay;
                Track {
                    wait: (-t).max(0.0),
                    phase: t.max(0.0) % p.fall,
                    fall: p.fall,
                    top: -self.top,
                    speed: span / p.fall,
                    x: Wave::swing(p.x, p.sway, p.sway_period, t),
                    rotation: Wave::swing(0.0, p.rotation, p.rotation_period, t),
                }
            })
            .collect()
    }
}

/// `bias + amplitude · sin(2π · frequency · τ + phase)`, τ in ms, phase in degrees.
#[derive(Debug, Clone, Copy)]
pub struct Wave {
    pub bias: f32,
    pub amplitude: f32,
    /// Cycles per millisecond.
    pub frequency: f32,
    pub phase: f32,
}

impl Wave {
    /// `base + amount · swing(t + τ, period)`: swing's eased ping-pong is exactly
    /// (1 − cos(π t / period)) / 2, a sinusoid of period 2 · period.
    fn swing(base: f32, amount: f32, period: f32, t: f32) -> Wave {
        if period <= 0.0 {
            return Wave { bias: base, amplitude: 0.0, frequency: 0.0, phase: 0.0 };
        }
        // −cos θ = sin(θ − 90°); θ at τ = 0 is π t / period.
        let phase = ((t / period * 180.0 - 90.0) as f64).rem_euclid(360.0) as f32;
        Wave { bias: base + amount / 2.0, amplitude: amount / 2.0, frequency: 1.0 / (2.0 * period), phase }
    }

    #[cfg(test)]
    pub fn at(&self, tau: f32) -> f32 {
        self.bias + self.amplitude * (std::f32::consts::TAU * self.frequency * tau + self.phase.to_radians()).sin()
    }
}

/// One particle's motion (window DIPs, τ in ms from the start): it waits above the
/// window, then falls at `speed` from `top` and starts over every `fall` ms, the
/// first fall already `phase` ms along; x and rotation (degrees) swing all the while.
#[derive(Debug, Clone, Copy)]
pub struct Track {
    pub wait: f32,
    pub phase: f32,
    pub fall: f32,
    pub top: f32,
    pub speed: f32,
    pub x: Wave,
    pub rotation: Wave,
}

impl Track {
    /// Position (x, y) and rotation at τ.
    #[cfg(test)]
    pub fn at(&self, tau: f32) -> (f32, f32, f32) {
        let y = if tau < self.wait { self.top } else { self.top + (self.phase + tau - self.wait) % self.fall * self.speed };
        (self.x.at(tau), y, self.rotation.at(tau))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::theme_def;

    #[test]
    fn particles_fall_and_wrap() {
        let cfg = theme_def("winter").unwrap().overlay.unwrap();
        let p = Particles::new(&cfg, 800.0, 42);
        assert_eq!(p.items.len(), cfg.count);
        let late = p.sprites(30_000.0, 600.0);
        assert_eq!(late.len(), cfg.count); // all delays (< 10 s) have passed
        assert!(late.iter().all(|s| s.y >= -40.0 && s.y <= 640.0));
        assert!(p.sprites(0.0, 600.0).len() < cfg.count);
    }

    #[test]
    fn tracks_follow_the_sprites() {
        for name in ["winter", "spring", "autumn"] {
            let cfg = theme_def(name).unwrap().overlay.unwrap();
            let p = Particles::new(&cfg, 900.0, 7);
            for start in [0.0, 3_456.0, 61_000.0] {
                let tracks = p.tracks(start, 700.0);
                for tau in [0.0, 16.0, 999.0, 7_500.0, 40_000.0] {
                    for s in p.sprites(start + tau, 700.0) {
                        let (x, y, r) = tracks[s.index].at(tau);
                        let close = |a: f32, b: f32| (a - b).abs() < 0.05 + b.abs() * 1e-4;
                        assert!(close(x, s.x) && close(y, s.y) && close(r, s.rotation), "{name} {start} {tau}: {:?} vs {s:?}", (x, y, r));
                    }
                }
            }
        }
    }

    #[test]
    fn swing_is_smooth_ping_pong() {
        assert_eq!(swing(0.0, 100.0), 0.0);
        assert!((swing(100.0, 100.0) - 1.0).abs() < 1e-5);
        assert!((swing(200.0, 100.0)).abs() < 1e-5);
    }
}
