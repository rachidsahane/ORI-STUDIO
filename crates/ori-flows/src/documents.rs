//! Document review entry points: AICD §9.
//!
//! `flows.documents.submit` (`ops/batch-12-plan.md` Q7, answered yes) sends a
//! `Draft` or `Stale` document to its owning seat by appending
//! `document.submitted`, the edge into `UnderReview` of `spec/DATA_MODEL.md`
//! section 3. Every append goes through [`DocumentRegistry::record`], so the
//! registry's pre-append checks and fold are the only control (batch 12 D1, D9).
//!
//! ```mermaid
//! sequenceDiagram
//!   participant C as caller
//!   participant D as Documents
//!   participant R as DocumentRegistry
//!   C->>D: submit(db, at, actor, document_id)
//!   D->>R: record(document.submitted, {document_id})
//!   R-->>D: seq, or a refusal with its defect
//!   D-->>C: the same
//! ```

use ori_core::types::{Actor, Id, Timestamp};
use ori_store::db::ProductDb;

use crate::registry::{
    DOCUMENT_SUBMITTED_KIND, DocumentRegistry, DocumentSubmittedWire, RegistryError,
};

/// Review transitions on one product's document registry: AICD §9.
#[derive(Debug)]
pub struct Documents<'r> {
    registry: &'r mut DocumentRegistry,
}

impl<'r> Documents<'r> {
    /// Wraps a registry read from the product database: AICD §9.
    #[must_use]
    pub fn new(registry: &'r mut DocumentRegistry) -> Self {
        Self { registry }
    }

    /// Appends `document.submitted` for `document_id`: AICD §9.
    ///
    /// Moves the document from `Draft` or `Stale` to `UnderReview`. Human or
    /// agent actors are accepted, as `DocumentEvent::Submitted` places no seat
    /// rule on the edge.
    ///
    /// # Errors
    ///
    /// [`RegistryError::Event`] when the registry is blocked by a refused fold
    /// event, nothing appended; [`RegistryError::Refused`] with
    /// `UnknownDocument` or `Transition` when the document is not registered or
    /// is not in a state the edge leaves, nothing appended; any other error of
    /// [`DocumentRegistry::record`].
    pub fn submit(
        &mut self,
        db: &mut ProductDb,
        at: Timestamp,
        actor: Actor,
        document_id: &Id,
    ) -> Result<u64, RegistryError> {
        if let Some((seq, defect)) = self.registry.blocked() {
            return Err(RegistryError::Event {
                seq,
                defect: defect.clone(),
            });
        }
        let payload = serde_json::to_string(&DocumentSubmittedWire {
            document_id: document_id.as_str().to_owned(),
        })
        .map_err(|_| RegistryError::Encode)?;
        self.registry
            .record(db, at, actor, DOCUMENT_SUBMITTED_KIND, &payload)
    }
}
