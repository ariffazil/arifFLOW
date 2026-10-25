// arifFlow governance/entity_class.rs
//
// [F-4 repair 2026-10-25] Actor classification for the federation FQ
// aggregate: fixtures are excluded because of a DECLARED, validated
// classification recorded at registration — never because of their name at
// the gate.
//
// Why (named failure class): the previous gate filtered on a hard-coded
// `SYNTHETIC_ACTORS` name deny-list. actor_id is caller-supplied at ingest,
// so any caller naming itself "test" or "attacker" self-exempted from the
// FQ gate. A name deny-list is caller-controlled and therefore spoofable.
//
// Doctrine (fail-closed):
//   - `ActorClass` is stamped on the actor state at registration/ingest from
//     THIS registry only. A receipt or request payload cannot set it.
//   - A caller-supplied "fixture" label does not self-exempt: labels on the
//     wire are not a classification source (DeclarationSource rejects them).
//   - No validated declaration → Unclassified → the actor COUNTS toward the
//     aggregate. Unknown or invalid classification is never an exemption.
//
// Declaration source (single owner): config/entity_classes.yaml — the
// federation's existing entity classification registry. Its `synthetic`
// class ("synthetic generators — no reality, pure noise") is the fixture
// class. No second registry is introduced; this module only reads and
// validates the one that already exists.
//
// Residual limitation (honest): the registry key is the actor id, which is
// caller-supplied at ingest. A caller that adopts the exact name of an
// operator-declared fixture can still inherit its class. Full closure needs
// identity binding (challenge/Ed25519), which is out of scope here. The
// exposure is bounded to operator-declared fixture names only (fail-closed
// for every other name).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Classification of a tracked actor, recorded at registration.
///
/// Deserialization is fail-closed: an unknown class token is `Unclassified`,
/// which COUNTS toward the federation FQ aggregate (`#[serde(other)]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ActorClass {
    /// Declared operational actor. Counts toward the aggregate.
    Operational,
    /// Declared test fixture with validated provenance. The ONLY class
    /// excluded from the federation FQ aggregate.
    Fixture,
    /// No validated declaration. Counts toward the aggregate (fail-closed).
    /// `#[serde(other)]` must sit on the last variant: any unknown class
    /// token deserializes HERE, which is exactly the fail-closed behavior.
    #[default]
    #[serde(other)]
    Unclassified,
}

impl ActorClass {
    /// Fail-closed rule: only a validated `Fixture` declaration exempts an
    /// actor from the federation FQ aggregate. Everything else counts.
    pub fn counts_toward_federation_aggregate(&self) -> bool {
        !matches!(self, ActorClass::Fixture)
    }
}

/// Where a classification declaration came from. Only these two channels are
/// trusted: an operator-authored config file, or in-process test-harness
/// registration. Anything a wire caller can put in a string (a request field,
/// a receipt label, a self-declaration) is rejected at parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeclarationSource {
    /// Operator-authored config (config/entity_classes.yaml).
    OperatorConfig,
    /// In-process test-harness registration (code-side, not an HTTP surface).
    TestHarness,
}

impl DeclarationSource {
    /// Parse a declared source string. Anything caller-influencable
    /// ("caller", "request", "receipt", "self", ...) is REJECTED — that is
    /// the F-4 rule: a caller-supplied label must not self-exempt.
    pub fn parse(s: &str) -> Result<Self, DeclarationError> {
        match s.trim().to_ascii_lowercase().as_str() {
            "operator_config" | "operator-config" | "config" => {
                Ok(DeclarationSource::OperatorConfig)
            }
            "test_harness" | "test-harness" | "harness" => Ok(DeclarationSource::TestHarness),
            other => Err(DeclarationError::UntrustedSource(other.to_string())),
        }
    }
}

/// Why a classification declaration was rejected. Rejections are recorded,
/// never silently applied: the rejected actor stays Unclassified (counts).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeclarationError {
    EmptyActorKey,
    SelfDeclaration(String),
    EmptyDeclarer,
    UntrustedSource(String),
    UnclassifiableClass,
    Conflict(String),
}

impl std::fmt::Display for DeclarationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeclarationError::EmptyActorKey => write!(f, "empty actor key"),
            DeclarationError::SelfDeclaration(a) => {
                write!(f, "actor '{}' declared its own classification", a)
            }
            DeclarationError::EmptyDeclarer => write!(f, "declaration has no declarer"),
            DeclarationError::UntrustedSource(s) => {
                write!(f, "untrusted declaration source '{}'", s)
            }
            DeclarationError::UnclassifiableClass => {
                write!(f, "Unclassified is not a declarable class")
            }
            DeclarationError::Conflict(a) => {
                write!(
                    f,
                    "conflicting classification already registered for '{}'",
                    a
                )
            }
        }
    }
}

/// One validated classification declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassDeclaration {
    /// Canonical (lowercased) actor key the declaration binds to.
    pub actor_key: String,
    pub class: ActorClass,
    /// Who declared it. Must be present and must not be the actor itself.
    pub declared_by: String,
    pub source: DeclarationSource,
}

impl ClassDeclaration {
    /// Validate a declaration. Every rule here exists so that no
    /// caller-supplied string can grant itself a class.
    pub fn validate(
        actor_key: &str,
        class: ActorClass,
        declared_by: &str,
        source: DeclarationSource,
    ) -> Result<Self, DeclarationError> {
        let key = actor_key.trim().to_ascii_lowercase();
        if key.is_empty() {
            return Err(DeclarationError::EmptyActorKey);
        }
        // Unclassified is the fail-closed default, not a declarable exemption.
        if class == ActorClass::Unclassified {
            return Err(DeclarationError::UnclassifiableClass);
        }
        let declarer = declared_by.trim();
        if declarer.is_empty() {
            return Err(DeclarationError::EmptyDeclarer);
        }
        // An actor may not declare its own class. Self-declaration is the
        // spoof this module exists to prevent.
        if declarer.to_ascii_lowercase() == key {
            return Err(DeclarationError::SelfDeclaration(key));
        }
        Ok(ClassDeclaration {
            actor_key: key,
            class,
            declared_by: declarer.to_string(),
            source,
        })
    }
}

/// Validated classification registry. Populated from the operator config or
/// by in-process harness registration — never from a receipt or request.
#[derive(Debug, Clone, Default)]
pub struct ClassRegistry {
    declarations: BTreeMap<String, ClassDeclaration>,
    rejects: Vec<String>,
}

impl ClassRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a validated declaration. A second, conflicting declaration
    /// for the same actor is rejected (first validated declaration wins) and
    /// recorded — silently reclassifying an actor is how gates get gamed.
    pub fn register(&mut self, decl: ClassDeclaration) -> Result<(), DeclarationError> {
        if let Some(existing) = self.declarations.get(&decl.actor_key) {
            if existing.class != decl.class {
                self.rejects.push(format!(
                    "conflict: '{}' already class={:?}, refused re-classification to {:?}",
                    decl.actor_key, existing.class, decl.class
                ));
                return Err(DeclarationError::Conflict(decl.actor_key));
            }
            return Ok(());
        }
        self.declarations.insert(decl.actor_key.clone(), decl);
        Ok(())
    }

    /// Classification of an actor key. Fail-closed: no validated declaration
    /// → `Unclassified` → counts toward the aggregate.
    pub fn classify(&self, actor_key: &str) -> ActorClass {
        let key = actor_key.trim().to_ascii_lowercase();
        self.declarations
            .get(&key)
            .map(|d| d.class)
            .unwrap_or(ActorClass::Unclassified)
    }

    pub fn len(&self) -> usize {
        self.declarations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }

    pub fn fixture_count(&self) -> usize {
        self.declarations
            .values()
            .filter(|d| d.class == ActorClass::Fixture)
            .count()
    }

    /// Rejected declarations/lines — surfaced at boot, kept for audit.
    pub fn rejects(&self) -> &[String] {
        &self.rejects
    }

    /// Parse the federation's EXISTING entity classification registry
    /// (config/entity_classes.yaml shape) into validated declarations.
    ///
    /// Accepted grammar (strict — anything else is rejected, not guessed):
    ///   - full-line comments (`# ...`), blank lines
    ///   - class keys: `^[a-z_]+:` (must be a known class)
    ///   - list items: `  - <actor id>` with optional trailing `# comment`
    ///
    /// Class semantics for the FQ aggregate (deliberate, documented):
    /// only `synthetic` (declared fixtures) becomes `ActorClass::Fixture`.
    /// Every other known class becomes `Operational` and COUNTS. The
    /// registry's own prose ("daemon/infrastructure should not influence FQ")
    /// is a weighting doctrine for the Python MCP's entity report — applying
    /// it here would drop real actors out of a fail-closed min-gate and
    /// WEAKEN the aggregate. Exclusions stay limited to fixtures.
    ///
    /// Ambiguity fails closed: an actor listed under two classes is rejected
    /// entirely and left Unclassified (it counts).
    pub fn from_entity_classes_yaml(text: &str) -> ClassRegistry {
        const KNOWN: [(&str, ActorClass); 5] = [
            ("human_agent", ActorClass::Operational),
            ("interactive_session", ActorClass::Operational),
            ("daemon", ActorClass::Operational),
            ("infrastructure", ActorClass::Operational),
            ("synthetic", ActorClass::Fixture),
        ];
        let mut registry = ClassRegistry::default();
        let mut current: Option<ActorClass> = None;
        // actor key -> (class, line) for ambiguity detection before commit.
        let mut seen: BTreeMap<String, (ActorClass, usize)> = BTreeMap::new();

        for (idx, raw_line) in text.lines().enumerate() {
            let line_no = idx + 1;
            let line = raw_line.trim_end();
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            // Class key line: `name:` possibly with a trailing comment.
            if !line.starts_with(char::is_whitespace) {
                let head = trimmed.split('#').next().unwrap_or("").trim();
                if let Some(name) = head.strip_suffix(':') {
                    let name = name.trim();
                    current = KNOWN.iter().find(|(k, _)| *k == name).map(|(_, c)| *c);
                    if current.is_none() {
                        registry.rejects.push(format!(
                            "line {}: unknown class key '{}' — its entries are NOT classified",
                            line_no, name
                        ));
                    }
                    continue;
                }
                registry.rejects.push(format!(
                    "line {}: unparseable top-level line — ignored",
                    line_no
                ));
                current = None;
                continue;
            }
            // List item: `- name` with optional trailing comment.
            let Some(rest) = trimmed.strip_prefix('-') else {
                registry.rejects.push(format!(
                    "line {}: unparseable entry line — ignored",
                    line_no
                ));
                continue;
            };
            let name = rest
                .split('#')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            let Some(class) = current else {
                registry.rejects.push(format!(
                    "line {}: entry '{}' under an unknown class — NOT classified",
                    line_no, name
                ));
                continue;
            };
            if name.is_empty() {
                registry
                    .rejects
                    .push(format!("line {}: empty entry — ignored", line_no));
                continue;
            }
            if let Some((prev_class, prev_line)) = seen.get(&name) {
                if *prev_class != class {
                    // Ambiguous declaration: fail closed to Unclassified.
                    registry.rejects.push(format!(
                        "line {}: '{}' declared as both {:?} (line {}) and {:?} — ambiguous, actor stays Unclassified and COUNTS",
                        line_no, name, prev_class, prev_line, class
                    ));
                    registry.declarations.remove(&name);
                    continue;
                }
                continue;
            }
            seen.insert(name.clone(), (class, line_no));
            let decl = ClassDeclaration {
                actor_key: name,
                class,
                declared_by: "operator_config:entity_classes.yaml".to_string(),
                source: DeclarationSource::OperatorConfig,
            };
            let _ = registry.register(decl);
        }
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unclassified_counts_toward_aggregate() {
        // Fail-closed default: no exemption without a validated declaration.
        assert!(ActorClass::Unclassified.counts_toward_federation_aggregate());
        assert!(ActorClass::Operational.counts_toward_federation_aggregate());
        assert!(!ActorClass::Fixture.counts_toward_federation_aggregate());
    }

    #[test]
    fn unknown_class_token_deserializes_fail_closed() {
        // A bogus/unknown class on disk or on the wire is NOT an exemption.
        let parsed: ActorClass = serde_json::from_str("\"TotallyBogus\"").unwrap();
        assert_eq!(parsed, ActorClass::Unclassified);
        assert!(parsed.counts_toward_federation_aggregate());
    }

    #[test]
    fn caller_supplied_source_labels_are_rejected() {
        // F-4 rule: a caller-supplied "fixture" label must not self-exempt.
        for bad in ["caller", "request", "receipt", "self", "fixture", ""] {
            assert!(
                DeclarationSource::parse(bad).is_err(),
                "source '{}' must be rejected",
                bad
            );
        }
        assert_eq!(
            DeclarationSource::parse("operator_config").unwrap(),
            DeclarationSource::OperatorConfig
        );
        assert_eq!(
            DeclarationSource::parse("test_harness").unwrap(),
            DeclarationSource::TestHarness
        );
    }

    #[test]
    fn self_declaration_is_rejected() {
        // An actor naming ITSELF in its own declaration gains no class.
        let err = ClassDeclaration::validate(
            "test",
            ActorClass::Fixture,
            "test",
            DeclarationSource::TestHarness,
        )
        .unwrap_err();
        assert_eq!(err, DeclarationError::SelfDeclaration("test".into()));

        let ok = ClassDeclaration::validate(
            "test",
            ActorClass::Fixture,
            "operator:harness-2026-10-25",
            DeclarationSource::TestHarness,
        );
        assert!(ok.is_ok());
    }

    #[test]
    fn empty_or_unclassifiable_declarations_are_rejected() {
        assert!(ClassDeclaration::validate(
            "  ",
            ActorClass::Fixture,
            "operator",
            DeclarationSource::OperatorConfig
        )
        .is_err());
        assert!(ClassDeclaration::validate(
            "test",
            ActorClass::Unclassified,
            "operator",
            DeclarationSource::OperatorConfig
        )
        .is_err());
        assert!(ClassDeclaration::validate(
            "test",
            ActorClass::Fixture,
            "   ",
            DeclarationSource::OperatorConfig
        )
        .is_err());
    }

    #[test]
    fn registry_classifies_only_validated_declarations() {
        let mut reg = ClassRegistry::new();
        assert_eq!(reg.classify("anything"), ActorClass::Unclassified);
        let decl = ClassDeclaration::validate(
            "staged_agent",
            ActorClass::Fixture,
            "operator:scenario-harness",
            DeclarationSource::TestHarness,
        )
        .unwrap();
        reg.register(decl).unwrap();
        assert_eq!(reg.classify("staged_agent"), ActorClass::Fixture);
        // Lookup is on the canonical key, but an UNREGISTERED name stays
        // Unclassified regardless of what it looks like.
        assert_eq!(reg.classify("test"), ActorClass::Unclassified);
        assert_eq!(reg.classify("attacker"), ActorClass::Unclassified);
    }

    #[test]
    fn conflicting_reclassification_is_rejected() {
        let mut reg = ClassRegistry::new();
        reg.register(
            ClassDeclaration::validate(
                "p0-metabolize",
                ActorClass::Fixture,
                "operator",
                DeclarationSource::OperatorConfig,
            )
            .unwrap(),
        )
        .unwrap();
        let err = reg
            .register(
                ClassDeclaration::validate(
                    "p0-metabolize",
                    ActorClass::Operational,
                    "someone-else",
                    DeclarationSource::TestHarness,
                )
                .unwrap(),
            )
            .unwrap_err();
        assert!(matches!(err, DeclarationError::Conflict(_)));
        // First validated declaration stands; the actor is still a fixture.
        assert_eq!(reg.classify("p0-metabolize"), ActorClass::Fixture);
    }

    #[test]
    fn yaml_registry_synthetic_class_is_fixture() {
        let yaml = r#"
# comment at top
human_agent:
  - arif
  - 333-AGI
daemon:
  # indented comment
  - morning-briefing
infrastructure:
  - chron
synthetic:
  - p0-metabolize  # trailing comment
  - chatgpt-arif-audit
"#;
        let reg = ClassRegistry::from_entity_classes_yaml(yaml);
        assert_eq!(reg.classify("p0-metabolize"), ActorClass::Fixture);
        assert_eq!(reg.classify("chatgpt-arif-audit"), ActorClass::Fixture);
        // Non-fixture classes COUNT — excluding them would weaken the gate.
        assert_eq!(reg.classify("arif"), ActorClass::Operational);
        assert_eq!(reg.classify("morning-briefing"), ActorClass::Operational);
        // An ordinary actor literally named "test" is not exempt.
        assert_eq!(reg.classify("test"), ActorClass::Unclassified);
        assert_eq!(reg.fixture_count(), 2);
        assert!(reg.rejects().is_empty());
    }

    #[test]
    fn yaml_ambiguity_and_unknown_structure_fail_closed() {
        let yaml = r#"
synthetic:
  - weird_actor
daemon:
  - weird_actor
not_a_class:
  - ghost_actor
garbage line without dash
"#;
        let reg = ClassRegistry::from_entity_classes_yaml(yaml);
        // Multi-class declaration is ambiguous → Unclassified → COUNTS.
        assert_eq!(reg.classify("weird_actor"), ActorClass::Unclassified);
        // Entries under an unknown class are NOT classified.
        assert_eq!(reg.classify("ghost_actor"), ActorClass::Unclassified);
        assert!(!reg.rejects().is_empty());
    }

    #[test]
    fn empty_registry_never_exempts() {
        let reg = ClassRegistry::from_entity_classes_yaml("");
        assert!(reg.is_empty());
        assert_eq!(reg.classify("test"), ActorClass::Unclassified);
    }
}
