//! The memory service: AICD §8, AICD §25.
//!
//! Owns `Layer`, `Indexer`, `CodeMap`, `Barrier` (sanitize), `ScopeEnforcer`,
//! `Retrieval` (context package), `Freshness`, `DriftAudit` and `CitationChecker`
//! (`spec/LLD.md` section 2).
//!
//! Must not: return unsanitized production content in a package
//! (`spec/LLD.md` section 2).
