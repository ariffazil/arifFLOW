// arifFlow — QG.V0.3 VECTOR ENGINE (v0.3.1-AMD, sealed 2026-08-14)
//
// Implements the seven-dimension vector ontology from
// /root/arifFlow/spec/QG_V0_3_VECTOR_SPEC.md:
//   - Band normalization (not monotone) — §3
//   - τ (tau) Reality Freshness half-life decay — §2.5
//   - Geometric (Nash) composition, fail-closed — §4.2
//   - Constellation classifier — §4.3
//   - FEEL anchoring (INV-9) — §2.2
//   - Independence monitoring (INV-3) — §1.1
//
// The vector diagnoses ("HOW am I unhealthy?"). The scalar ranks.
// Scalar never replaces vector. DITEMPA BUKAN DIBERI.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ── Epistemology ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Epistemology {
    Measure,
    Witness,
    Feel,
    Live,
}

impl Epistemology {
    pub fn code(&self) -> &'static str {
        match self {
            Epistemology::Measure => "MEASURE",
            Epistemology::Witness => "WITNESS",
            Epistemology::Feel => "FEEL",
            Epistemology::Live => "LIVE",
        }
    }

    /// Freshness half-life in cycles — spec §2.5.
    pub fn half_life_cycles(&self, feel_anchor_n: u64) -> u64 {
        match self {
            Epistemology::Live => 10,
            Epistemology::Measure => 100,
            Epistemology::Witness => 250,
            Epistemology::Feel => feel_anchor_n.max(1),
        }
    }
}

// ── Dimensions ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum Dimension {
    Fq,
    G,
    J,
    W3,
    CDark,
    DS,
    Omega0,
}

impl Dimension {
    pub fn code(&self) -> &'static str {
        match self {
            Dimension::Fq => "fq",
            Dimension::G => "g",
            Dimension::J => "j",
            Dimension::W3 => "w3",
            Dimension::CDark => "c_dark",
            Dimension::DS => "ds",
            Dimension::Omega0 => "omega",
        }
    }

    pub fn failure(&self) -> &'static str {
        match self {
            Dimension::Fq => "SIMULATION",
            Dimension::G => "GOVERNANCE_COLLAPSE",
            Dimension::J => "BAD_AUTHORIZATION",
            Dimension::W3 => "COHERENCE_FRACTURE",
            Dimension::CDark => "UNSEEN_DEBT",
            Dimension::DS => "THERMO_WASTE",
            Dimension::Omega0 => "STAGNATION",
        }
    }

    pub fn default_epistemology(&self) -> Epistemology {
        match self {
            Dimension::Fq => Epistemology::Live,
            Dimension::G => Epistemology::Witness,
            Dimension::J => Epistemology::Measure,
            Dimension::W3 => Epistemology::Witness,
            Dimension::CDark => Epistemology::Measure,
            Dimension::DS => Epistemology::Measure,
            Dimension::Omega0 => Epistemology::Feel,
        }
    }
}

// ── Band Normalization (spec §3) ─────────────────────────────────────────

/// Normalize a raw dimension value to a [0,1] health score via its BAND.
/// Not monotone — both tails of FQ/Ω₀ are disease.
pub fn band_normalize(dim: Dimension, raw: f64) -> f64 {
    match dim {
        // Triangular band: 1.0 at 1.0; 0 at ≤0.1 (BURNING) and ≥6.0 (VERIFICATION_DOMINANCE)
        Dimension::Fq => {
            if raw <= 0.1 || raw >= 6.0 {
                0.0
            } else if raw <= 1.0 {
                (raw - 0.1) / 0.9
            } else {
                1.0 - (raw - 1.0) / 5.0
            }
        }
        Dimension::G => raw.clamp(0.0, 1.0),
        // |J| ≤ 0.6 → 1.0; →0 as |J|→∞
        Dimension::J => {
            let a = raw.abs();
            if a <= 0.6 {
                1.0
            } else {
                (0.6 / a).clamp(0.0, 1.0)
            }
        }
        Dimension::W3 => raw.clamp(0.0, 1.0),
        // inverse: 0.30 → 1.0 healthy; 0.50 → 0.0
        Dimension::CDark => {
            if raw <= 0.05 {
                1.0 // some visible debt is healthy
            } else if raw <= 0.30 {
                1.0 - (raw - 0.05) / 0.25 * 0.0 // flat healthy band 0.05-0.30
            } else if raw >= 0.50 {
                0.0
            } else {
                (0.50 - raw) / 0.20
            }
        }
        // ΔS ≤ 0 → 1.0; linear decay to 0 at ΔS = +0.2
        Dimension::DS => {
            if raw <= 0.0 {
                1.0
            } else if raw >= 0.2 {
                0.0
            } else {
                (0.2 - raw) / 0.2
            }
        }
        // band 0.03–0.05 → 1.0; 0.0 or >0.10 → 0.0
        Dimension::Omega0 => {
            if raw <= 0.0 || raw > 0.10 {
                0.0
            } else if raw >= 0.03 && raw <= 0.05 {
                1.0
            } else if raw < 0.03 {
                raw / 0.03
            } else {
                (0.10 - raw) / 0.05
            }
        }
    }
}

// ── τ Freshness Decay (spec §2.5) ────────────────────────────────────────

/// Decay law: h_eff = h × 2^(−τ/τ₁/₂)
pub fn decay(h: f64, tau_cycles: u64, half_life: u64) -> f64 {
    if half_life == 0 {
        return 0.0;
    }
    h * 2f64.powf(-(tau_cycles as f64) / (half_life as f64))
}

/// Freshness band from age vs half-life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Freshness {
    Fresh,
    Aging,
    Stale,
    Dead,
}

pub fn freshness_band(tau_cycles: u64, half_life: u64) -> Freshness {
    if half_life == 0 {
        return Freshness::Dead;
    }
    let hl = half_life as f64;
    let t = tau_cycles as f64;
    if t <= hl {
        Freshness::Fresh
    } else if t <= 3.0 * hl {
        Freshness::Aging
    } else if t <= 6.0 * hl {
        Freshness::Stale
    } else {
        Freshness::Dead
    }
}

// ── Dimension State & Vector Store ───────────────────────────────────────

/// A single dimension's live reading.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionState {
    pub value: f64,
    pub epistemology: Epistemology,
    pub method_id: String,
    pub producer: String,
    /// Cycle when this reading was last updated
    pub last_update_cycle: u64,
    /// FEEL dimensions: whether a WITNESS/MEASURE anchor is present
    pub anchor_present: bool,
    /// Cycle of the last valid anchor (WITNESS/MEASURE), if any
    pub last_anchor_cycle: Option<u64>,
}

/// The live vector store — one slot per dimension.
#[derive(Debug, Clone, Default)]
pub struct VectorStore {
    pub dims: BTreeMap<Dimension, DimensionState>,
    pub cycle: u64,
    /// FEEL anchor window N cycles (INV-9). Default 10.
    pub feel_anchor_n: u64,
}

impl VectorStore {
    pub fn new() -> Self {
        Self {
            dims: BTreeMap::new(),
            cycle: 0,
            feel_anchor_n: 10,
        }
    }

    /// Advance the metabolic cycle (call on every /health and /ingest).
    pub fn tick(&mut self) {
        self.cycle = self.cycle.saturating_add(1);
    }

    /// Ingest a dimension reading.
    pub fn ingest(
        &mut self,
        dim: Dimension,
        value: f64,
        epistemology: Epistemology,
        method_id: &str,
        producer: &str,
        anchor_present: bool,
    ) {
        let mut anchor_present = anchor_present;
        // WITNESS/MEASURE readings themselves act as anchors for FEEL dims.
        if epistemology == Epistemology::Witness || epistemology == Epistemology::Measure {
            anchor_present = true;
        }
        self.dims.insert(
            dim,
            DimensionState {
                value,
                epistemology,
                method_id: method_id.to_string(),
                producer: producer.to_string(),
                last_update_cycle: self.cycle,
                anchor_present,
                last_anchor_cycle: if anchor_present {
                    Some(self.cycle)
                } else {
                    None
                },
            },
        );
    }

    /// Effective health for a dimension, τ-discounted and FEEL-anchored.
    ///
    /// Returns (h_eff, freshness_band, is_unmeasured).
    pub fn health(&self, dim: Dimension) -> (f64, Freshness, bool) {
        let Some(st) = self.dims.get(&dim) else {
            return (0.0, Freshness::Dead, true);
        };
        let hl = st.epistemology.half_life_cycles(self.feel_anchor_n);
        let tau = self.cycle.saturating_sub(st.last_update_cycle);

        // INV-9: FEEL without a recent WITNESS/MEASURE anchor is UNANCHORED.
        // An anchor is the most recent WITNESS or MEASURE reading anywhere in
        // the store (e.g. W³) — FEEL can never anchor itself.
        if st.epistemology == Epistemology::Feel {
            let anchor_cycle = self
                .dims
                .iter()
                .filter(|(d, s)| {
                    *d != &dim
                        && (s.epistemology == Epistemology::Witness
                            || s.epistemology == Epistemology::Measure)
                })
                .map(|(_, s)| s.last_update_cycle)
                .max();
            let Some(anchor) = anchor_cycle else {
                return (0.0, Freshness::Stale, true);
            };
            let anchor_age = self.cycle.saturating_sub(anchor);
            if anchor_age > self.feel_anchor_n {
                return (0.0, Freshness::Stale, true);
            }
            // FEEL health is anchored: the self-report is a lead indicator, its
            // trust derives from how fresh the WITNESS/MEASURE anchor is. The
            // self-report's own age does not decay it — the anchor does.
            let h_band = band_normalize(dim, st.value);
            let h_eff = decay(h_band, anchor_age, hl);
            return (h_eff, freshness_band(anchor_age, hl), false);
        }

        let h_band = band_normalize(dim, st.value);
        let h_eff = decay(h_band, tau, hl);
        let fb = freshness_band(tau, hl);
        (h_eff, fb, false)
    }

    /// FQ is computed live from the receipt store — inject it as a reading
    /// before calling health().
    pub fn inject_fq(&mut self, quotient: Option<f64>) {
        let q = quotient.unwrap_or(0.0);
        let st = self.dims.get_mut(&Dimension::Fq);
        if let Some(st) = st {
            st.value = q;
            st.last_update_cycle = self.cycle;
        } else {
            self.dims.insert(
                Dimension::Fq,
                DimensionState {
                    value: q,
                    epistemology: Epistemology::Live,
                    method_id: "flow_quotient_v2.1".into(),
                    producer: "arifFlow".into(),
                    last_update_cycle: self.cycle,
                    anchor_present: true,
                    last_anchor_cycle: Some(self.cycle),
                },
            );
        }
    }

    /// Per-dimension vector state with band labels (spec §4.1).
    pub fn vector_state(&self) -> BTreeMap<String, serde_json::Value> {
        let mut out = BTreeMap::new();
        for dim in [
            Dimension::Fq,
            Dimension::G,
            Dimension::J,
            Dimension::W3,
            Dimension::CDark,
            Dimension::DS,
            Dimension::Omega0,
        ] {
            let (h, fb, unmeasured) = self.health(dim);
            let (band, pathological) = if unmeasured {
                ("UNMEASURED".to_string(), true)
            } else {
                let (b, p) = band_of(h);
                (b.to_string(), p)
            };
            let st = self.dims.get(&dim);
            out.insert(
                dim.code().to_string(),
                serde_json::json!({
                    "h": (h * 100.0).round() / 100.0,
                    "band": band,
                    "pathological": pathological,
                    "freshness": format!("{:?}", fb),
                    "epistemic": st.map_or("UNMEASURED".to_string(), |s| s.epistemology.code().to_string()),
                    "value": st.map_or(serde_json::Value::Null, |s| serde_json::json!(s.value)),
                    "producer": st.map_or(serde_json::Value::Null, |s| serde_json::json!(s.producer)),
                }),
            );
        }
        out
    }

    /// Fused rank — weighted geometric mean, fail-closed (spec §4.2).
    pub fn rank(&self) -> f64 {
        let hs: Vec<f64> = [
            Dimension::Fq,
            Dimension::G,
            Dimension::J,
            Dimension::W3,
            Dimension::CDark,
            Dimension::DS,
            Dimension::Omega0,
        ]
        .iter()
        .map(|d| {
            let (h, _, unmeasured) = self.health(*d);
            if unmeasured {
                0.0
            } else {
                h
            }
        })
        .collect();

        // Any zero dimension → rank 0.0 (fail-closed).
        if hs.iter().any(|h| *h <= 0.0) {
            return 0.0;
        }
        let product: f64 = hs.iter().map(|h| h.ln()).sum();
        (product / hs.len() as f64).exp()
    }

    /// Primary pathology — argmin health (the diagnosis, spec §4.1).
    pub fn primary_pathology(&self) -> Option<Dimension> {
        [
            Dimension::Fq,
            Dimension::G,
            Dimension::J,
            Dimension::W3,
            Dimension::CDark,
            Dimension::DS,
            Dimension::Omega0,
        ]
        .iter()
        .map(|d| {
            let (h, _, unmeasured) = self.health(*d);
            (*d, if unmeasured { 0.0 } else { h })
        })
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(d, _)| d)
    }

    /// Constellation classifier (spec §4.3).
    pub fn constellation(&self) -> String {
        // Unanchored FEEL
        if let Some(st) = self.dims.get(&Dimension::Omega0) {
            if st.epistemology == Epistemology::Feel && !self.omega_anchored() {
                return "FEEL_UNANCHORED".into();
            }
        }
        // Reality lag — any dimension STALE or DEAD
        for d in [
            Dimension::Fq,
            Dimension::G,
            Dimension::J,
            Dimension::W3,
            Dimension::CDark,
            Dimension::DS,
            Dimension::Omega0,
        ] {
            let (_, fb, unmeasured) = self.health(d);
            if unmeasured {
                return "ONTOLOGY_BREACH".into();
            }
            if fb == Freshness::Stale || fb == Freshness::Dead {
                return "REALITY_LAG".into();
            }
        }
        // Pathological dimension detection
        let mut pathological: Vec<Dimension> = Vec::new();
        for d in [
            Dimension::Fq,
            Dimension::G,
            Dimension::J,
            Dimension::W3,
            Dimension::CDark,
            Dimension::DS,
            Dimension::Omega0,
        ] {
            let (h, _, _) = self.health(d);
            if h < 0.5 {
                pathological.push(d);
            }
        }
        if pathological.is_empty() {
            return "FLOWING".into();
        }
        // Single pathology → named constellation
        if pathological.len() == 1 {
            return pathological[0].failure().into();
        }
        // Multi-pathology → the most severe + PARADOX flag
        if pathological.len() >= 2 {
            let primary = self
                .primary_pathology()
                .map_or("UNKNOWN".to_string(), |d| d.failure().to_string());
            return format!("PARADOX:{}", primary);
        }
        "UNKNOWN".into()
    }

    fn omega_anchored(&self) -> bool {
        let Some(st) = self.dims.get(&Dimension::Omega0) else {
            return false;
        };
        if st.epistemology != Epistemology::Feel {
            return true;
        }
        self.dims
            .iter()
            .filter(|(d, s)| {
                *d != &Dimension::Omega0
                    && (s.epistemology == Epistemology::Witness
                        || s.epistemology == Epistemology::Measure)
            })
            .map(|(_, s)| s.last_update_cycle)
            .max()
            .map_or(false, |anchor_cycle| {
                self.cycle.saturating_sub(anchor_cycle) <= self.feel_anchor_n
            })
    }
}

/// Band label + pathological flag from effective health h ∈ [0,1].
fn band_of(h: f64) -> (&'static str, bool) {
    if h >= 0.75 {
        ("HEALTHY", false)
    } else if h >= 0.5 {
        ("CAUTION", false)
    } else {
        ("PATHOLOGICAL", true)
    }
}

// ── Independence Monitor (INV-3, spec §1.1) ──────────────────────────────

/// Running window of per-dimension health for independence tracking.
#[derive(Debug, Clone, Default)]
pub struct IndependenceMonitor {
    /// dimension code → recent h history
    pub history: BTreeMap<Dimension, Vec<f64>>,
    pub max_window: usize,
}

impl IndependenceMonitor {
    pub fn new(max_window: usize) -> Self {
        Self {
            history: BTreeMap::new(),
            max_window,
        }
    }

    pub fn record(&mut self, vector: &VectorStore) {
        for dim in [
            Dimension::Fq,
            Dimension::G,
            Dimension::J,
            Dimension::W3,
            Dimension::CDark,
            Dimension::DS,
            Dimension::Omega0,
        ] {
            let (h, _, unmeasured) = vector.health(dim);
            let entry = self.history.entry(dim).or_default();
            if entry.len() >= self.max_window {
                entry.remove(0);
            }
            // Unmeasured → drop from independence math (absence is not a value)
            if !unmeasured {
                entry.push(h);
            }
        }
    }

    /// Pairwise |ρ| between dimensions; returns pairs exceeding the collapse
    /// threshold (0.85) that ALSO reached pathological while the other stayed
    /// healthy — the full independence test.
    pub fn collapse_pairs(&self) -> Vec<(Dimension, Dimension, f64)> {
        let dims = [
            Dimension::Fq,
            Dimension::G,
            Dimension::J,
            Dimension::W3,
            Dimension::CDark,
            Dimension::DS,
            Dimension::Omega0,
        ];
        let mut out = Vec::new();
        for (i, d1) in dims.iter().enumerate() {
            for d2 in dims.iter().skip(i + 1) {
                if let (Some(a), Some(b)) = (self.history.get(d1), self.history.get(d2)) {
                    if a.len() < 3 || b.len() < 3 {
                        continue;
                    }
                    let r = pearson(a, b);
                    if r.abs() > 0.85 {
                        out.push((*d1, *d2, r));
                    }
                }
            }
        }
        out
    }
}

fn pearson(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len()) as f64;
    if n < 2.0 {
        return 0.0;
    }
    let a_mean = a.iter().take(n as usize).sum::<f64>() / n;
    let b_mean = b.iter().take(n as usize).sum::<f64>() / n;
    let mut num = 0.0;
    let mut da = 0.0;
    let mut db = 0.0;
    for i in 0..n as usize {
        let x = a[i] - a_mean;
        let y = b[i] - b_mean;
        num += x * y;
        da += x * x;
        db += y * y;
    }
    if da == 0.0 || db == 0.0 {
        0.0
    } else {
        num / (da.sqrt() * db.sqrt())
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_band_fq_both_tails_disease() {
        // FQ = 6.5 (VERIFICATION_DOMINANCE) → 0.0, not 1.0 (scalar trap killed)
        assert!(band_normalize(Dimension::Fq, 6.5) < 0.1);
        assert!(band_normalize(Dimension::Fq, 0.05) == 0.0); // BURNING
        assert!(band_normalize(Dimension::Fq, 1.0) > 0.99); // peak
        assert!(band_normalize(Dimension::Fq, 0.5) > 0.4); // lower healthy edge
        assert!(band_normalize(Dimension::Fq, 2.0) > 0.79); // upper healthy edge
    }

    #[test]
    fn test_band_g_monotone() {
        assert!(band_normalize(Dimension::G, 0.9) > band_normalize(Dimension::G, 0.6));
        assert!(band_normalize(Dimension::G, 0.95) <= 1.0);
    }

    #[test]
    fn test_band_j_small_good() {
        assert!(band_normalize(Dimension::J, 0.3) > 0.99);
        assert!(band_normalize(Dimension::J, 0.6) > 0.99);
        assert!(band_normalize(Dimension::J, 3.0) < 0.25);
    }

    #[test]
    fn test_band_cdark_low_good() {
        assert!(band_normalize(Dimension::CDark, 0.2) > 0.9);
        assert!(band_normalize(Dimension::CDark, 0.6) == 0.0);
    }

    #[test]
    fn test_band_omega_zero_is_dead() {
        assert!(band_normalize(Dimension::Omega0, 0.0) == 0.0);
        assert!(band_normalize(Dimension::Omega0, 0.04) > 0.99);
        assert!(band_normalize(Dimension::Omega0, 0.12) == 0.0);
    }

    #[test]
    fn test_tau_decay() {
        // At half-life, health halves
        let h = decay(1.0, 100, 100);
        assert!((h - 0.5).abs() < 0.01);
        // Fresh: no meaningful decay
        let h2 = decay(1.0, 1, 100);
        assert!(h2 > 0.99);
        // Dead after 6 half-lives
        let h3 = decay(1.0, 700, 100);
        assert!(h3 < 0.01);
    }

    #[test]
    fn test_freshness_bands() {
        assert_eq!(freshness_band(50, 100), Freshness::Fresh);
        assert_eq!(freshness_band(200, 100), Freshness::Aging);
        assert_eq!(freshness_band(400, 100), Freshness::Stale);
        assert_eq!(freshness_band(700, 100), Freshness::Dead);
    }

    #[test]
    fn test_feel_unanchored_demotes_to_unmeasured() {
        let mut vs = VectorStore::new();
        vs.tick();
        // Ω₀ ingested as FEEL without anchor
        vs.ingest(
            Dimension::Omega0,
            0.04,
            Epistemology::Feel,
            "humility",
            "333-AGI",
            false,
        );
        let (h, _, unmeasured) = vs.health(Dimension::Omega0);
        assert!(unmeasured);
        assert_eq!(h, 0.0);
        // After 20 cycles (beyond N=10), still unanchored
        for _ in 0..20 {
            vs.tick();
        }
        let (_, _, unmeasured) = vs.health(Dimension::Omega0);
        assert!(unmeasured);
        // Anchoring via W³ (WITNESS) within N cycles restores it
        vs.ingest(
            Dimension::W3,
            0.8,
            Epistemology::Witness,
            "forge_witness",
            "A-FORGE",
            true,
        );
        vs.tick();
        let (h, _, unmeasured) = vs.health(Dimension::Omega0);
        assert!(!unmeasured);
        assert!(h > 0.5);
    }

    #[test]
    fn test_rank_fail_closed() {
        // GENUINELY unanchored: no WITNESS/MEASURE in the store at all.
        let mut vs = VectorStore::new();
        vs.tick();
        vs.inject_fq(Some(1.0));
        vs.ingest(
            Dimension::Omega0,
            0.04,
            Epistemology::Feel,
            "humility",
            "333-AGI",
            false,
        );
        // Ω₀ unanchored → rank must collapse to 0.0 (fail-closed)
        let rank = vs.rank();
        assert_eq!(rank, 0.0);
        // Now build a full healthy vector — W3 (WITNESS) anchors Ω₀
        vs.ingest(
            Dimension::G,
            0.9,
            Epistemology::Witness,
            "evaluate",
            "A-FORGE",
            true,
        );
        vs.ingest(
            Dimension::J,
            0.3,
            Epistemology::Measure,
            "apex",
            "A-FORGE",
            true,
        );
        vs.ingest(
            Dimension::W3,
            0.85,
            Epistemology::Witness,
            "witness",
            "A-FORGE",
            true,
        );
        vs.ingest(
            Dimension::CDark,
            0.2,
            Epistemology::Measure,
            "evaluate",
            "A-FORGE",
            true,
        );
        vs.ingest(
            Dimension::DS,
            -0.05,
            Epistemology::Measure,
            "entropy",
            "arifFlow",
            true,
        );
        vs.tick();
        let rank = vs.rank();
        assert!(rank > 0.5, "rank was {}", rank);
    }

    #[test]
    fn test_constellation_detects_simulation() {
        let mut vs = VectorStore::new();
        vs.tick();
        vs.inject_fq(Some(0.05)); // BURNING → simulation
        vs.ingest(
            Dimension::G,
            0.9,
            Epistemology::Witness,
            "evaluate",
            "A-FORGE",
            true,
        );
        vs.ingest(
            Dimension::J,
            0.3,
            Epistemology::Measure,
            "apex",
            "A-FORGE",
            true,
        );
        vs.ingest(
            Dimension::W3,
            0.85,
            Epistemology::Witness,
            "witness",
            "A-FORGE",
            true,
        );
        vs.ingest(
            Dimension::CDark,
            0.2,
            Epistemology::Measure,
            "evaluate",
            "A-FORGE",
            true,
        );
        vs.ingest(
            Dimension::DS,
            -0.05,
            Epistemology::Measure,
            "entropy",
            "arifFlow",
            true,
        );
        vs.ingest(
            Dimension::Omega0,
            0.04,
            Epistemology::Feel,
            "humility",
            "333-AGI",
            true,
        );
        let c = vs.constellation();
        assert_eq!(c, "SIMULATION");
    }

    #[test]
    fn test_constellation_flowing() {
        let mut vs = VectorStore::new();
        vs.tick();
        vs.inject_fq(Some(1.0));
        vs.ingest(
            Dimension::G,
            0.9,
            Epistemology::Witness,
            "evaluate",
            "A-FORGE",
            true,
        );
        vs.ingest(
            Dimension::J,
            0.3,
            Epistemology::Measure,
            "apex",
            "A-FORGE",
            true,
        );
        vs.ingest(
            Dimension::W3,
            0.85,
            Epistemology::Witness,
            "witness",
            "A-FORGE",
            true,
        );
        vs.ingest(
            Dimension::CDark,
            0.2,
            Epistemology::Measure,
            "evaluate",
            "A-FORGE",
            true,
        );
        vs.ingest(
            Dimension::DS,
            -0.05,
            Epistemology::Measure,
            "entropy",
            "arifFlow",
            true,
        );
        vs.ingest(
            Dimension::Omega0,
            0.04,
            Epistemology::Feel,
            "humility",
            "333-AGI",
            true,
        );
        vs.ingest(
            Dimension::W3,
            0.85,
            Epistemology::Witness,
            "witness",
            "A-FORGE",
            true,
        );
        let c = vs.constellation();
        assert_eq!(c, "FLOWING");
    }

    #[test]
    fn test_independence_monitor() {
        let mut mon = IndependenceMonitor::new(10);
        let mut vs = VectorStore::new();
        // Two dims that always move together
        for _ in 0..10 {
            vs.tick();
            vs.inject_fq(Some(1.0));
            vs.ingest(
                Dimension::G,
                0.9,
                Epistemology::Witness,
                "e",
                "A-FORGE",
                true,
            );
            vs.ingest(
                Dimension::W3,
                0.9,
                Epistemology::Witness,
                "w",
                "A-FORGE",
                true,
            );
            vs.ingest(
                Dimension::J,
                0.3,
                Epistemology::Measure,
                "a",
                "A-FORGE",
                true,
            );
            vs.ingest(
                Dimension::CDark,
                0.2,
                Epistemology::Measure,
                "e",
                "A-FORGE",
                true,
            );
            vs.ingest(
                Dimension::DS,
                -0.05,
                Epistemology::Measure,
                "e",
                "arifFlow",
                true,
            );
            vs.ingest(
                Dimension::Omega0,
                0.04,
                Epistemology::Feel,
                "h",
                "333-AGI",
                true,
            );
            mon.record(&vs);
        }
        // Perfectly correlated dims → collapse detected
        let pairs = mon.collapse_pairs();
        assert!(!pairs.is_empty());
    }
}
