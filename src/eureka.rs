// arifFlow — Eureka Insight Tracker
// Real-time metabolic tracking of Eureka moments — when paradox tension
// resolves into new structure. The ATLAS333 → EUREKA777 → CUBE777 loop
// made observable.
//
// DITEMPA BUKAN DIBERI

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use uuid::Uuid;

use crate::receipt::FlowQuotient;

// ── Eureka Severity ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EurekaState {
    /// Raw insight, unverified
    Candidate,
    /// Verified by evidence, contradiction resolved
    Resolved,
    /// Major architectural insight — doctrine-grade
    Major,
    /// Minor improvement or observation
    Minor,
    /// Superseded by a later Eureka
    Superseded,
}

impl fmt::Display for EurekaState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EurekaState::Candidate => write!(f, "CANDIDATE"),
            EurekaState::Resolved => write!(f, "RESOLVED"),
            EurekaState::Major => write!(f, "MAJOR"),
            EurekaState::Minor => write!(f, "MINOR"),
            EurekaState::Superseded => write!(f, "SUPERSEDED"),
        }
    }
}

// ── Eureka Record ─────────────────────────────────────────────────────────

/// A single Eureka insight — the unit atom of metabolic cognition.
/// When paradox tension resolves, this is what gets recorded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EurekaRecord {
    /// Unique identifier
    pub eureka_id: String,
    /// Short label
    pub title: String,
    /// The core insight — what was realized
    pub insight: String,
    /// Domain: arifOS, A-FORGE, arifFlow, AAA, GEOX, WEALTH, WELL, CROSS
    pub domain: String,
    /// State: CANDIDATE → RESOLVED → MAJOR or SUPERSEDED
    #[serde(default = "default_state")]
    pub state: EurekaState,
    /// ATLAS333 paradox IDs this resolves (e.g. ["P12", "P24"])
    #[serde(default)]
    pub paradox_ids: Vec<String>,
    /// FQ at the moment of emergence
    #[serde(default)]
    pub fq_at_emergence: Option<f64>,
    /// FQ verdict at emergence
    #[serde(default)]
    pub fq_verdict_at_emergence: Option<String>,
    /// Entropy reduction (ΔS). Negative = entropy reduced. 0 = no change.
    #[serde(default)]
    pub delta_s: Option<f64>,
    /// Session that produced this
    #[serde(default)]
    pub session_id: Option<String>,
    /// Who recorded it
    pub actor_id: String,
    /// When it was recorded
    pub created_at: String,
    /// Tags for semantic search
    #[serde(default)]
    pub tags: Vec<String>,
    /// Links to evidence: files, receipts, vault entries
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    /// Optional payload — raw context, conversation excerpt, etc.
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
}

fn default_state() -> EurekaState {
    EurekaState::Candidate
}

impl EurekaRecord {
    pub fn new(title: String, insight: String, domain: String, actor_id: String) -> Self {
        Self {
            eureka_id: Uuid::new_v4().to_string(),
            title,
            insight,
            domain,
            state: EurekaState::Candidate,
            paradox_ids: vec![],
            fq_at_emergence: None,
            fq_verdict_at_emergence: None,
            delta_s: None,
            session_id: None,
            actor_id,
            created_at: Utc::now().to_rfc3339(),
            tags: vec![],
            evidence_refs: vec![],
            payload: None,
        }
    }

    /// Attach FQ context from the current metabolic state
    pub fn with_fq(mut self, fq: &FlowQuotient) -> Self {
        self.fq_at_emergence = Some(fq.quotient);
        self.fq_verdict_at_emergence = Some(format!("{}", fq.verdict));
        self
    }

    /// Attach session context
    pub fn with_session(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Attach entropy measurement
    pub fn with_delta_s(mut self, delta_s: f64) -> Self {
        self.delta_s = Some(delta_s);
        self
    }

    /// Attach paradox IDs
    pub fn with_paradoxes(mut self, ids: Vec<String>) -> Self {
        self.paradox_ids = ids;
        self
    }

    /// Attach tags
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Attach evidence references
    pub fn with_evidence(mut self, refs: Vec<String>) -> Self {
        self.evidence_refs = refs;
        self
    }

    /// Attach raw payload
    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = Some(payload);
        self
    }
}

// ── Eureka Store ──────────────────────────────────────────────────────────

/// Persistent store for Eureka insights.
/// Append-only JSONL file, survives restarts.
pub struct EurekaStore {
    records: Vec<EurekaRecord>,
    persist_path: Option<PathBuf>,
    capacity: usize,
}

impl EurekaStore {
    /// Create a new in-memory-only store
    pub fn new(capacity: usize) -> Self {
        Self {
            records: Vec::with_capacity(capacity),
            persist_path: None,
            capacity,
        }
    }

    /// Create a new store with file-backed persistence
    pub fn new_with_persistence(capacity: usize, persist_path: PathBuf) -> Self {
        let mut store = Self {
            records: Vec::with_capacity(capacity),
            persist_path: Some(persist_path.clone()),
            capacity,
        };
        store.load_from_disk();
        store
    }

    /// Load existing records from JSONL file
    fn load_from_disk(&mut self) {
        if let Some(ref path) = self.persist_path {
            if let Ok(file) = File::open(path) {
                let reader = BufReader::new(file);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        if let Ok(record) = serde_json::from_str::<EurekaRecord>(&line) {
                            if self.records.len() >= self.capacity {
                                self.records.remove(0);
                            }
                            self.records.push(record);
                        }
                    }
                }
            }
        }
    }

    /// Persist a single record to disk
    fn persist_one(&self, record: &EurekaRecord) {
        if let Some(ref path) = self.persist_path {
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
                if let Ok(json) = serde_json::to_string(record) {
                    let _ = writeln!(file, "{}", json);
                }
            }
        }
    }

    /// Record a new Eureka insight
    pub fn record(&mut self, record: EurekaRecord) -> &EurekaRecord {
        self.persist_one(&record);
        if self.records.len() >= self.capacity {
            self.records.remove(0);
        }
        self.records.push(record);
        self.records.last().unwrap()
    }

    /// List recent Eurekas (newest first)
    pub fn list(&self, limit: usize) -> Vec<&EurekaRecord> {
        let actual_limit = limit.min(self.records.len());
        self.records.iter().rev().take(actual_limit).collect()
    }

    /// List Eurekas filtered by domain
    pub fn list_by_domain(&self, domain: &str, limit: usize) -> Vec<&EurekaRecord> {
        self.records
            .iter()
            .rev()
            .filter(|r| r.domain.eq_ignore_ascii_case(domain))
            .take(limit)
            .collect()
    }

    /// List Eurekas filtered by state
    pub fn list_by_state(&self, state: &EurekaState, limit: usize) -> Vec<&EurekaRecord> {
        self.records
            .iter()
            .rev()
            .filter(|r| r.state == *state)
            .take(limit)
            .collect()
    }

    /// Get a specific Eureka by ID
    pub fn get(&self, eureka_id: &str) -> Option<&EurekaRecord> {
        self.records.iter().find(|r| r.eureka_id == eureka_id)
    }

    /// Update the state of an existing Eureka
    pub fn update_state(
        &mut self,
        eureka_id: &str,
        new_state: EurekaState,
    ) -> Option<EurekaRecord> {
        let pos = self.records.iter().position(|r| r.eureka_id == eureka_id)?;
        self.records[pos].state = new_state;
        let updated = self.records[pos].clone();
        // Re-persist all records on state change (simplest approach for JSONL)
        self.rebuild_persist();
        Some(updated)
    }

    /// Rebuild the persist file from current in-memory state
    fn rebuild_persist(&self) {
        if let Some(ref path) = self.persist_path {
            if let Ok(mut file) = File::create(path) {
                for record in &self.records {
                    if let Ok(json) = serde_json::to_string(record) {
                        let _ = writeln!(file, "{}", json);
                    }
                }
            }
        }
    }

    /// Number of records
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Get all records (for serialization)
    pub fn all(&self) -> &[EurekaRecord] {
        &self.records
    }

    /// Get persist path
    pub fn persist_path(&self) -> Option<&PathBuf> {
        self.persist_path.as_ref()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eureka_record_new() {
        let r = EurekaRecord::new(
            "Test Eureka".into(),
            "Something was discovered".into(),
            "arifFlow".into(),
            "333-AGI".into(),
        );
        assert_eq!(r.title, "Test Eureka");
        assert_eq!(r.state, EurekaState::Candidate);
        assert!(!r.eureka_id.is_empty());
    }

    #[test]
    fn test_eureka_record_builder() {
        let fq = FlowQuotient {
            quotient: 1.5,
            verdict: crate::receipt::FlowVerdict::Balanced,
            execute_count: 10,
            verify_count: 8,
            execute_cost_ns: 1000,
            verify_cost_ns: 800,
            raw_ratio: 1.25,
            is_smoothed: false,
            alpha: 0.0,
            window_s: 300,
            window_size: 20,
            cost_clamp_ns: (1000, 300_000_000_000),
            by_actor: Default::default(),
            worst_actor: None,
            actor_count: 0,
            trend: crate::receipt::FqTrend {
                direction: crate::receipt::TrendDirection::Stable,
                rate_per_min: 0.0,
                volatility: 0.0,
                samples: 1,
                window_s: 600,
            },
        };
        let r = EurekaRecord::new(
            "Builder Test".into(),
            "Chained construction".into(),
            "arifOS".into(),
            "333-AGI".into(),
        )
        .with_fq(&fq)
        .with_session("SEAL-abc".into())
        .with_delta_s(-1.0)
        .with_paradoxes(vec!["P12".into(), "P24".into()])
        .with_tags(vec!["consent".into(), "agent-api".into()])
        .with_evidence(vec![
            "/root/arifFlow/forge_work/2026-08-01/EUREKA-arifFlow-2026-08-01.md".into(),
        ]);

        assert_eq!(r.fq_at_emergence, Some(1.5));
        assert_eq!(r.session_id, Some("SEAL-abc".into()));
        assert_eq!(r.delta_s, Some(-1.0));
        assert_eq!(r.paradox_ids.len(), 2);
        assert_eq!(r.tags.len(), 2);
    }

    #[test]
    fn test_eureka_store_record_and_list() {
        let mut store = EurekaStore::new(100);
        let r = EurekaRecord::new(
            "Test".into(),
            "Insight".into(),
            "arifFlow".into(),
            "test".into(),
        );
        store.record(r);
        assert_eq!(store.len(), 1);

        let list = store.list(10);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "Test");
    }

    #[test]
    fn test_eureka_store_capacity() {
        let mut store = EurekaStore::new(3);
        for i in 0..5 {
            store.record(EurekaRecord::new(
                format!("Eureka {}", i),
                "insight".into(),
                "test".into(),
                "test".into(),
            ));
        }
        assert_eq!(store.len(), 3);
        assert_eq!(store.list(10)[0].title, "Eureka 4");
    }

    #[test]
    fn test_eureka_store_filter_by_domain() {
        let mut store = EurekaStore::new(100);
        store.record(EurekaRecord::new(
            "A".into(),
            "a".into(),
            "arifFlow".into(),
            "t".into(),
        ));
        store.record(EurekaRecord::new(
            "B".into(),
            "b".into(),
            "arifOS".into(),
            "t".into(),
        ));
        store.record(EurekaRecord::new(
            "C".into(),
            "c".into(),
            "arifFlow".into(),
            "t".into(),
        ));

        let flow = store.list_by_domain("arifFlow", 10);
        assert_eq!(flow.len(), 2);
        let os = store.list_by_domain("arifOS", 10);
        assert_eq!(os.len(), 1);
    }

    #[test]
    fn test_eureka_store_update_state() {
        let mut store = EurekaStore::new(100);
        let r = EurekaRecord::new("Test".into(), "i".into(), "arifFlow".into(), "t".into());
        let id = r.eureka_id.clone();
        store.record(r);

        let updated = store.update_state(&id, EurekaState::Resolved);
        assert!(updated.is_some());
        assert_eq!(store.get(&id).unwrap().state, EurekaState::Resolved);
    }
}
