//! Gray–Scott model, forward Euler (dt = 1), periodic boundary conditions.
//! Nine-point Laplacian: center -1, cardinal 0.2, diagonal 0.05.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parameters {
    pub feed: f32,
    pub kill: f32,
    pub diffusion_u: f32,
    pub diffusion_v: f32,
}

impl Parameters {
    pub fn is_valid(self) -> bool {
        [
            (self.feed, 0.0, 0.1),
            (self.kill, 0.0, 0.1),
            (self.diffusion_u, 0.05, 1.0),
            (self.diffusion_v, 0.01, 0.5),
        ]
        .iter()
        .all(|&(v, low, high)| v.is_finite() && (low..=high).contains(&v))
    }
}

pub const PRESETS: [(&str, &str, Parameters); 5] = [
    (
        "迷路",
        "曲線がつながり、有機的な迷路に。",
        Parameters {
            feed: 0.029,
            kill: 0.057,
            diffusion_u: 1.0,
            diffusion_v: 0.5,
        },
    ),
    (
        "斑点",
        "小さな粒が分裂して広がります。",
        Parameters {
            feed: 0.0367,
            kill: 0.0649,
            diffusion_u: 1.0,
            diffusion_v: 0.5,
        },
    ),
    (
        "珊瑚",
        "枝分かれする繊細な成長模様。",
        Parameters {
            feed: 0.0545,
            kill: 0.062,
            diffusion_u: 1.0,
            diffusion_v: 0.5,
        },
    ),
    (
        "波紋",
        "輪が広がり、互いに干渉します。",
        Parameters {
            feed: 0.022,
            kill: 0.051,
            diffusion_u: 1.0,
            diffusion_v: 0.5,
        },
    ),
    (
        "細胞",
        "丸い境界が生まれる細胞状の模様。",
        Parameters {
            feed: 0.039,
            kill: 0.058,
            diffusion_u: 1.0,
            diffusion_v: 0.5,
        },
    ),
];

pub struct Simulation {
    pub size: usize,
    pub cells: Vec<[f32; 2]>,
    next: Vec<[f32; 2]>,
    pub steps: u64,
}

impl Simulation {
    pub fn new(size: usize, seed: u64) -> Self {
        assert!((16..=512).contains(&size), "grid size must be 16..=512");
        let mut sim = Self {
            size,
            cells: vec![[1.0, 0.0]; size * size],
            next: vec![[0.0; 2]; size * size],
            steps: 0,
        };
        let mut rng = seed.max(1);
        let mut random = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            (rng >> 40) as f32 / (1u32 << 24) as f32
        };
        // A jittered field of seeds avoids an empty canvas and exact symmetry.
        let count = (size / 22).max(2);
        let spacing = size as f32 / count as f32;
        for row in 0..count {
            for col in 0..count {
                let x = (col as f32 + 0.2 + random() * 0.6) * spacing;
                let y = (row as f32 + 0.2 + random() * 0.6) * spacing;
                sim.paint(x, y, 2.5 + random() * 2.5, false);
            }
        }
        sim
    }

    pub fn step(&mut self, p: Parameters) {
        assert!(p.is_valid(), "parameters outside supported stable ranges");
        let n = self.size;
        for y in 0..n {
            let north = if y == 0 { n - 1 } else { y - 1 } * n;
            let south = if y == n - 1 { 0 } else { y + 1 } * n;
            let row = y * n;
            for x in 0..n {
                let left = if x == 0 { n - 1 } else { x - 1 };
                let right = if x == n - 1 { 0 } else { x + 1 };
                let i = row + x;
                let [u, v] = self.cells[i];
                let mut lap = [-u, -v];
                for c in [
                    self.cells[row + left],
                    self.cells[row + right],
                    self.cells[north + x],
                    self.cells[south + x],
                ] {
                    lap[0] += c[0] * 0.2;
                    lap[1] += c[1] * 0.2;
                }
                for c in [
                    self.cells[north + left],
                    self.cells[north + right],
                    self.cells[south + left],
                    self.cells[south + right],
                ] {
                    lap[0] += c[0] * 0.05;
                    lap[1] += c[1] * 0.05;
                }
                let reaction = u * v * v;
                self.next[i] = [
                    (u + p.diffusion_u * lap[0] - reaction + p.feed * (1.0 - u)).clamp(0.0, 1.0),
                    (v + p.diffusion_v * lap[1] + reaction - (p.feed + p.kill) * v).clamp(0.0, 1.0),
                ];
            }
        }
        std::mem::swap(&mut self.cells, &mut self.next);
        self.steps += 1;
    }

    pub fn paint(&mut self, x: f32, y: f32, radius: f32, erase: bool) {
        if !x.is_finite() || !y.is_finite() || !radius.is_finite() {
            return;
        }
        let radius = radius.clamp(1.0, 40.0);
        let x = x.clamp(0.0, self.size as f32 - 1.0);
        let y = y.clamp(0.0, self.size as f32 - 1.0);
        for dy in -(radius.ceil() as i32)..=radius.ceil() as i32 {
            for dx in -(radius.ceil() as i32)..=radius.ceil() as i32 {
                if (dx * dx + dy * dy) as f32 > radius * radius {
                    continue;
                }
                let px = (x as i32 + dx).rem_euclid(self.size as i32) as usize;
                let py = (y as i32 + dy).rem_euclid(self.size as i32) as usize;
                self.cells[py * self.size + px] = if erase { [1.0, 0.0] } else { [0.5, 0.25] };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_equilibrium_stays_uniform() {
        let mut sim = Simulation::new(32, 1);
        sim.cells.fill([1.0, 0.0]);
        for _ in 0..100 {
            sim.step(PRESETS[0].2);
        }
        assert!(
            sim.cells
                .iter()
                .all(|c| (c[0] - 1.0).abs() < 1e-6 && c[1] == 0.0)
        );
    }

    #[test]
    fn diffusion_wraps_across_both_edges() {
        let mut sim = Simulation::new(16, 1);
        sim.cells.fill([1.0, 0.0]);
        sim.cells[0] = [1.0, 0.2];
        sim.step(PRESETS[0].2);
        assert!((sim.cells[15][1] - 0.02).abs() < 1e-6);
        assert!((sim.cells[15 * 16][1] - 0.02).abs() < 1e-6);
        assert!((sim.cells[255][1] - 0.005).abs() < 1e-6);
    }

    #[test]
    fn presets_evolve_and_remain_finite() {
        for (_, _, p) in PRESETS {
            let mut sim = Simulation::new(64, 42);
            let initial = sim.cells.clone();
            for _ in 0..1000 {
                sim.step(p);
            }
            assert_ne!(sim.cells, initial);
            assert!(
                sim.cells
                    .iter()
                    .flatten()
                    .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
            );
            let max = sim.cells.iter().map(|c| c[1]).fold(0.0f32, f32::max);
            assert!(max > 0.1, "preset died out: {p:?}");
        }
    }

    #[test]
    fn seeds_are_reproducible_and_brush_wraps() {
        let mut a = Simulation::new(32, 42);
        assert_eq!(a.cells, Simulation::new(32, 42).cells);
        assert_ne!(a.cells, Simulation::new(32, 43).cells);
        a.paint(0.0, 0.0, 3.0, false);
        assert_eq!(a.cells[31], [0.5, 0.25]);
        a.paint(0.0, 0.0, 3.0, true);
        assert_eq!(a.cells[31], [1.0, 0.0]);
    }

    #[test]
    fn invalid_parameters_are_rejected() {
        let mut p = PRESETS[0].2;
        p.feed = f32::NAN;
        assert!(!p.is_valid());
        p.feed = -0.01;
        assert!(!p.is_valid());
        p.feed = 0.03;
        p.diffusion_u = 1.01;
        assert!(!p.is_valid());
    }
}
