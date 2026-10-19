//! Integration tests for ORI-T-0197 document registry.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use ori_core::types::{Actor, DocumentSet, DocumentState, Id, Seat, Timestamp};
use ori_flows::ports::PortError;
use ori_flows::registry::{
    DOCUMENT_APPROVED_KIND, DOCUMENT_CHANGES_REQUESTED_KIND, DOCUMENT_CREATED_KIND,
    DOCUMENT_DRIFTED_KIND, DOCUMENT_REGISTERED_KIND, DOCUMENT_SUBMITTED_KIND, DocumentRegistry,
    DocumentWire, EventDefect, FOUNDATION, PRODUCT_CREATED_KIND, ProductCreatedWire, RegistryError,
    SeatWire, owner_for_kind,
};
use ori_store::db::ProductDb;
use ori_store::event_log::EventLog;

const PRODUCT: &str = "01BX5ZZKBKACTAV9WEVGEMMVRY";
const AT: i64 = 1_700_000_000_000;

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "ori-t-0197-{label}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("scratch");
        Self { path }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct Home {
    db: ProductDb,
    _scratch: Scratch,
}

impl Home {
    fn new(label: &str) -> Self {
        let scratch = Scratch::new(label);
        let db = ProductDb::open(&scratch.path, PRODUCT, ts(1)).expect("open");
        Self {
            db,
            _scratch: scratch,
        }
    }

    fn root(&self) -> &Path {
        &self._scratch.path
    }

    fn reopen(mut self) -> Self {
        let path = self._scratch.path.clone();
        drop(self.db);
        self.db = ProductDb::open(&path, PRODUCT, ts(2)).expect("reopen");
        self
    }
}

fn ts(m: i64) -> Timestamp {
    Timestamp::from_millis(m)
}

fn ulid(n: u64) -> Id {
    Id::parse(&format!("01{n:024}")).expect("ulid")
}

fn human() -> Id {
    ulid(9_001)
}

fn agent() -> Id {
    ulid(9_002)
}

fn pid() -> Id {
    Id::parse(PRODUCT).expect("product")
}

fn foundation_docs() -> Vec<DocumentWire> {
    FOUNDATION
        .iter()
        .enumerate()
        .map(|(i, row)| DocumentWire {
            id: ulid(100 + i as u64).as_str().to_owned(),
            kind: row.kind.to_owned(),
            path: row.path.to_owned(),
            set: "foundation".to_owned(),
        })
        .collect()
}

fn all_seats() -> Vec<SeatWire> {
    Seat::ALL
        .iter()
        .enumerate()
        .map(|(i, seat)| SeatWire {
            seat: seat.as_str().to_owned(),
            holder: ulid(200 + i as u64).as_str().to_owned(),
        })
        .collect()
}

fn abs_repo(root: &Path) -> String {
    let repo = root.join("repo");
    fs::create_dir_all(&repo).expect("repo");
    fs::canonicalize(&repo).unwrap().display().to_string()
}

fn good_wire(root: &Path) -> ProductCreatedWire {
    ProductCreatedWire {
        name: "demo".to_owned(),
        repo_path: abs_repo(root),
        repo_remote: Some("git@github.com:org/repo.git".to_owned()),
        aicd_version: "0.3".to_owned(),
        origin: "new".to_owned(),
        seats: all_seats(),
        documents: foundation_docs(),
    }
}

fn open_reg(home: &mut Home) -> DocumentRegistry {
    DocumentRegistry::read(&mut home.db).expect("read")
}

fn plant(home: &mut Home, actor: Actor, kind: &str, payload: &str) {
    EventLog::append(
        home.db.connection(),
        pid(),
        ts(AT),
        actor,
        kind,
        None,
        payload.to_owned(),
    )
    .expect("plant");
}

fn setup_new_product(home: &mut Home) -> DocumentRegistry {
    let wire = good_wire(home.root());
    let mut reg = open_reg(home);
    reg.create_product(&mut home.db, ts(AT), Actor::Human(human()), wire)
        .expect("create");
    reg
}

fn brief_id() -> String {
    ulid(100).as_str().to_owned()
}

fn created_payload(doc: &str, pr: &str) -> String {
    format!(r#"{{"document_id":"{doc}","pr":"{pr}","content_len":1,"citations_unresolved":0}}"#)
}

fn approved_payload(doc: &str, seat: &str) -> String {
    format!(r#"{{"document_id":"{doc}","seat":"{seat}"}}"#)
}

#[test]
fn ori_t_0197_foundation_rows_match_plan() {
    assert_eq!(FOUNDATION.len(), 11);
    assert_eq!(FOUNDATION[0].kind, "brief");
    assert_eq!(FOUNDATION[0].path, "spec/PROJECT_BRIEF.md");
    assert_eq!(FOUNDATION[10].kind, "agent_instructions");
}

#[test]
fn ori_t_0197_create_product_new_origin_writes_one_event() {
    let mut home = Home::new("cp-one");
    let wire = good_wire(home.root());
    let mut reg = open_reg(&mut home);
    reg.create_product(&mut home.db, ts(AT), Actor::Human(human()), wire)
        .expect("ok");
    assert_eq!(
        EventLog::verify(home.db.connection()).unwrap().tip_seq,
        Some(1)
    );
}

#[test]
fn ori_t_0197_create_product_refuses_remote_with_password() {
    let mut home = Home::new("cp-pass");
    let wire = good_wire(home.root());
    let mut wire = wire;
    wire.repo_remote = Some("https://user:pass@github.com/x.git".to_owned());
    let mut reg = open_reg(&mut home);
    assert!(matches!(
        reg.create_product(&mut home.db, ts(AT), Actor::Human(human()), wire),
        Err(RegistryError::Refused {
            defect: EventDefect::SecretInUrl,
            ..
        })
    ));
}

#[test]
fn ori_t_0197_create_product_accepts_git_at_host() {
    let mut home = Home::new("cp-git");
    let wire = good_wire(home.root());
    let mut reg = open_reg(&mut home);
    reg.create_product(&mut home.db, ts(AT), Actor::Human(human()), wire)
        .expect("ok");
}

#[test]
fn ori_t_0197_create_product_refuses_non_human_actor() {
    let mut home = Home::new("cp-agent");
    let wire = good_wire(home.root());
    let mut reg = open_reg(&mut home);
    assert!(matches!(
        reg.create_product(&mut home.db, ts(AT), Actor::Agent(agent()), wire),
        Err(RegistryError::Refused {
            defect: EventDefect::NotHuman,
            ..
        })
    ));
}

#[test]
fn ori_t_0197_create_product_refuses_duplicate_seat() {
    let mut home = Home::new("cp-dup");
    let wire = good_wire(home.root());
    let mut wire = wire;
    wire.seats[3] = wire.seats[0].clone();
    let mut reg = open_reg(&mut home);
    assert!(matches!(
        reg.create_product(&mut home.db, ts(AT), Actor::Human(human()), wire),
        Err(RegistryError::Refused {
            defect: EventDefect::Seats,
            ..
        })
    ));
}

#[test]
fn ori_t_0197_create_product_refuses_non_absolute_repo_path() {
    let mut home = Home::new("cp-rel");
    let wire = good_wire(home.root());
    let mut wire = wire;
    wire.repo_path = "relative/not/abs".to_owned();
    let mut reg = open_reg(&mut home);
    assert!(matches!(
        reg.create_product(&mut home.db, ts(AT), Actor::Human(human()), wire),
        Err(RegistryError::Refused {
            defect: EventDefect::RepoPath,
            ..
        })
    ));
}

#[test]
fn ori_t_0197_create_product_refuses_when_registry_locked() {
    let mut home = Home::new("cp-lock");
    let mut reg = setup_new_product(&mut home);
    plant(
        &mut home,
        Actor::Human(human()),
        PRODUCT_CREATED_KIND,
        r#"{"name":"x","repo_path":"/tmp","repo_remote":"https://u:p@h","aicd_version":"0.3","origin":"new","seats":[],"documents":[]}"#,
    );
    let wire = good_wire(home.root());
    assert!(matches!(
        reg.create_product(&mut home.db, ts(AT), Actor::Human(human()), wire),
        Err(RegistryError::Event { .. })
    ));
}

#[test]
fn ori_t_0197_record_created_from_missing() {
    let mut home = Home::new("rec-created");
    let mut reg = setup_new_product(&mut home);
    let doc = brief_id();
    reg.record(
        &mut home.db,
        ts(AT),
        Actor::Human(human()),
        DOCUMENT_CREATED_KIND,
        &created_payload(&doc, "42"),
    )
    .expect("created");
    let doc = reg.documents().find(|d| d.id.as_str() == doc).expect("doc");
    assert_eq!(doc.state, DocumentState::Draft);
}

#[test]
fn ori_t_0197_record_refuses_agent_approved() {
    let mut home = Home::new("rec-ag-ap");
    let mut reg = setup_new_product(&mut home);
    let doc = brief_id();
    assert!(matches!(
        reg.record(
            &mut home.db,
            ts(AT),
            Actor::Agent(agent()),
            DOCUMENT_APPROVED_KIND,
            &approved_payload(&doc, "product_owner"),
        ),
        Err(RegistryError::Refused {
            defect: EventDefect::NotHuman,
            ..
        })
    ));
}

#[test]
fn ori_t_0197_record_refuses_approver_not_holding_seat() {
    let mut home = Home::new("rec-seat");
    let mut reg = setup_new_product(&mut home);
    let doc = brief_id();
    reg.record(
        &mut home.db,
        ts(AT),
        Actor::Human(human()),
        DOCUMENT_CREATED_KIND,
        &created_payload(&doc, "1"),
    )
    .expect("draft");
    // wrong holder for architect seat
    assert!(matches!(
        reg.record(
            &mut home.db,
            ts(AT),
            Actor::Human(ulid(999)),
            DOCUMENT_APPROVED_KIND,
            &approved_payload(&doc, "architect"),
        ),
        Err(RegistryError::Refused {
            defect: EventDefect::SeatHolder,
            ..
        })
    ));
}

#[test]
fn ori_t_0197_record_approver_holding_seat_succeeds() {
    let mut home = Home::new("rec-ok-ap");
    let mut reg = setup_new_product(&mut home);
    let doc = brief_id();
    reg.record(
        &mut home.db,
        ts(AT),
        Actor::Human(human()),
        DOCUMENT_CREATED_KIND,
        &created_payload(&doc, "1"),
    )
    .expect("created");
    // product_owner owns brief
    reg.record(
        &mut home.db,
        ts(AT + 1),
        Actor::Human(ulid(203)),
        DOCUMENT_APPROVED_KIND,
        &approved_payload(&doc, "product_owner"),
    )
    .expect("approved");
}

#[test]
fn ori_t_0197_record_refuses_changes_requested_by_agent() {
    let mut home = Home::new("rec-cr-ag");
    let mut reg = setup_new_product(&mut home);
    let doc = brief_id();
    assert!(matches!(
        reg.record(
            &mut home.db,
            ts(AT),
            Actor::Agent(agent()),
            DOCUMENT_CHANGES_REQUESTED_KIND,
            &format!(r#"{{"document_id":"{doc}","instructions":"fix"}}"#),
        ),
        Err(RegistryError::Refused {
            defect: EventDefect::NotHuman,
            ..
        })
    ));
}

#[test]
fn ori_t_0197_record_refuses_overlong_instructions() {
    let mut home = Home::new("rec-long");
    let mut reg = setup_new_product(&mut home);
    let doc = brief_id();
    let long = "a".repeat(8193);
    assert!(matches!(
        reg.record(
            &mut home.db,
            ts(AT),
            Actor::Human(human()),
            DOCUMENT_CHANGES_REQUESTED_KIND,
            &format!(r#"{{"document_id":"{doc}","instructions":"{long}"}}"#),
        ),
        Err(RegistryError::Refused {
            defect: EventDefect::Payload,
            ..
        })
    ));
}

#[test]
fn ori_t_0197_record_refuses_pr_with_password_userinfo() {
    let mut home = Home::new("rec-pr");
    let mut reg = setup_new_product(&mut home);
    let doc = brief_id();
    assert!(matches!(
        reg.record(
            &mut home.db,
            ts(AT),
            Actor::Human(human()),
            DOCUMENT_CREATED_KIND,
            &created_payload(&doc, "https://u:p@example.com/1"),
        ),
        Err(RegistryError::Refused {
            defect: EventDefect::SecretInUrl,
            ..
        })
    ));
}

#[test]
fn ori_t_0197_migrated_origin_empty_documents() {
    let mut home = Home::new("mig");
    let mut reg = open_reg(&mut home);
    let wire = ProductCreatedWire {
        name: "m".to_owned(),
        repo_path: abs_repo(home.root()),
        repo_remote: None,
        aicd_version: "0.3".to_owned(),
        origin: "migrated".to_owned(),
        seats: vec![],
        documents: vec![],
    };
    reg.create_product(&mut home.db, ts(AT), Actor::Human(human()), wire)
        .expect("migrated");
}

#[test]
fn ori_t_0197_migrated_origin_refuses_as_built_approval_without_seats() {
    let mut home = Home::new("mig-ap");
    let mut reg = open_reg(&mut home);
    let wire = ProductCreatedWire {
        name: "m".to_owned(),
        repo_path: abs_repo(home.root()),
        repo_remote: None,
        aicd_version: "0.3".to_owned(),
        origin: "migrated".to_owned(),
        seats: vec![],
        documents: vec![],
    };
    reg.create_product(&mut home.db, ts(AT), Actor::Human(human()), wire)
        .expect("migrated");
    let doc = ulid(50).as_str().to_owned();
    reg.record(
        &mut home.db,
        ts(AT),
        Actor::Human(human()),
        DOCUMENT_REGISTERED_KIND,
        &format!(
            r#"{{"document_id":"{doc}","kind":"as_built_architecture","path":"spec/x.md","set":"migration"}}"#
        ),
    )
    .expect("registered");
    assert!(matches!(
        reg.record(
            &mut home.db,
            ts(AT + 1),
            Actor::Human(human()),
            DOCUMENT_APPROVED_KIND,
            &approved_payload(&doc, "architect"),
        ),
        Err(RegistryError::Refused {
            defect: EventDefect::MigratedSeats,
            ..
        })
    ));
}

#[test]
fn ori_t_0197_payload_refuses_json_array() {
    let mut home = Home::new("pay-arr");
    let mut reg = setup_new_product(&mut home);
    assert!(matches!(
        reg.record(
            &mut home.db,
            ts(AT),
            Actor::Human(human()),
            DOCUMENT_CREATED_KIND,
            "[]",
        ),
        Err(RegistryError::Refused {
            defect: EventDefect::Payload,
            ..
        })
    ));
}

#[test]
fn ori_t_0197_payload_refuses_duplicate_key() {
    let mut home = Home::new("pay-dup");
    let mut reg = setup_new_product(&mut home);
    let doc = brief_id();
    let payload = format!(
        r#"{{"document_id":"{doc}","document_id":"{doc}","pr":"1","content_len":1,"citations_unresolved":0}}"#
    );
    assert!(matches!(
        reg.record(
            &mut home.db,
            ts(AT),
            Actor::Human(human()),
            DOCUMENT_CREATED_KIND,
            &payload,
        ),
        Err(RegistryError::Refused {
            defect: EventDefect::Payload,
            ..
        })
    ));
}

#[test]
fn ori_t_0197_payload_refuses_trailing_text() {
    let mut home = Home::new("pay-trail");
    let mut reg = setup_new_product(&mut home);
    let doc = brief_id();
    let payload = format!("{} trailing", created_payload(&doc, "1"));
    assert!(matches!(
        reg.record(
            &mut home.db,
            ts(AT),
            Actor::Human(human()),
            DOCUMENT_CREATED_KIND,
            &payload,
        ),
        Err(RegistryError::Refused {
            defect: EventDefect::Payload,
            ..
        })
    ));
}

#[test]
fn ori_t_0197_fold_refuses_document_submitted_kind() {
    let mut home = Home::new("sub");
    setup_new_product(&mut home);
    plant(
        &mut home,
        Actor::Human(human()),
        DOCUMENT_SUBMITTED_KIND,
        &format!(r#"{{"document_id":"{}"}}"#, brief_id()),
    );
    assert!(DocumentRegistry::read(&mut home.db).is_err());
}

#[test]
fn ori_t_0197_fold_refuses_document_drifted_kind() {
    let mut home = Home::new("drift");
    setup_new_product(&mut home);
    plant(
        &mut home,
        Actor::Human(human()),
        DOCUMENT_DRIFTED_KIND,
        &format!(r#"{{"document_id":"{}","at":1}}"#, brief_id()),
    );
    assert!(DocumentRegistry::read(&mut home.db).is_err());
}

#[test]
fn ori_t_0197_fold_lockout_on_planted_product_created_bad_remote() {
    let mut home = Home::new("fold-remote");
    setup_new_product(&mut home);
    plant(
        &mut home,
        Actor::Human(human()),
        PRODUCT_CREATED_KIND,
        r#"{"name":"x","repo_path":"/tmp","repo_remote":"https://u:p@h","aicd_version":"0.3","origin":"new","seats":[],"documents":[]}"#,
    );
    assert!(matches!(
        DocumentRegistry::read(&mut home.db),
        Err(RegistryError::Event {
            defect: EventDefect::SecretInUrl,
            ..
        })
    ));
}

#[test]
fn ori_t_0197_fold_lockout_on_planted_product_created_non_absolute_path() {
    let mut home = Home::new("fold-path");
    setup_new_product(&mut home);
    plant(
        &mut home,
        Actor::Human(human()),
        PRODUCT_CREATED_KIND,
        r#"{"name":"x","repo_path":"relative","aicd_version":"0.3","origin":"new","seats":[],"documents":[]}"#,
    );
    assert!(matches!(
        DocumentRegistry::read(&mut home.db),
        Err(RegistryError::Event {
            defect: EventDefect::RepoPath,
            ..
        })
    ));
}

#[test]
fn ori_t_0197_fold_refuses_planted_approval_without_seat_holder() {
    let mut home = Home::new("fold-ap");
    setup_new_product(&mut home);
    let doc = brief_id();
    plant(
        &mut home,
        Actor::Human(human()),
        DOCUMENT_CREATED_KIND,
        &created_payload(&doc, "1"),
    );
    plant(
        &mut home,
        Actor::Human(ulid(999)),
        DOCUMENT_APPROVED_KIND,
        &approved_payload(&doc, "product_owner"),
    );
    assert!(matches!(
        DocumentRegistry::read(&mut home.db),
        Err(RegistryError::Event {
            defect: EventDefect::SeatHolder,
            ..
        })
    ));
}

#[test]
fn ori_t_0197_fold_refuses_planted_changes_requested_control_char() {
    let mut home = Home::new("fold-cc");
    setup_new_product(&mut home);
    let doc = brief_id();
    plant(
        &mut home,
        Actor::Human(human()),
        DOCUMENT_CREATED_KIND,
        &created_payload(&doc, "1"),
    );
    plant(
        &mut home,
        Actor::Human(human()),
        DOCUMENT_CHANGES_REQUESTED_KIND,
        &format!(
            r#"{{"document_id":"{doc}","instructions":"bad{}"}}"#,
            '\u{0001}'
        ),
    );
    assert!(matches!(
        DocumentRegistry::read(&mut home.db),
        Err(RegistryError::Event {
            defect: EventDefect::Payload,
            ..
        })
    ));
}

#[test]
fn ori_t_0197_lockout_blocks_record_after_bad_event() {
    let mut home = Home::new("lock-rec");
    let mut reg = setup_new_product(&mut home);
    plant(
        &mut home,
        Actor::Human(human()),
        PRODUCT_CREATED_KIND,
        r#"{"name":"x","repo_path":"/tmp","repo_remote":"https://u:p@h","aicd_version":"0.3","origin":"new","seats":[],"documents":[]}"#,
    );
    assert!(matches!(
        reg.record(
            &mut home.db,
            ts(AT),
            Actor::Human(human()),
            DOCUMENT_CREATED_KIND,
            &created_payload(&brief_id(), "1"),
        ),
        Err(RegistryError::Event { .. })
    ));
}

#[test]
fn ori_t_0197_lockout_blocks_create_product_after_bad_event() {
    let mut home = Home::new("lock-cp2");
    let mut reg = setup_new_product(&mut home);
    plant(
        &mut home,
        Actor::Human(human()),
        PRODUCT_CREATED_KIND,
        r#"{"name":"x","repo_path":"/tmp","repo_remote":"https://u:p@h","aicd_version":"0.3","origin":"new","seats":[],"documents":[]}"#,
    );
    let wire = good_wire(home.root());
    assert!(matches!(
        reg.create_product(&mut home.db, ts(AT), Actor::Human(human()), wire),
        Err(RegistryError::Event { .. })
    ));
}

#[test]
fn ori_t_0197_lockout_survives_reload() {
    let mut home = Home::new("lock-re");
    setup_new_product(&mut home);
    plant(
        &mut home,
        Actor::Human(human()),
        PRODUCT_CREATED_KIND,
        r#"{"name":"x","repo_path":"/tmp","repo_remote":"https://u:p@h","aicd_version":"0.3","origin":"new","seats":[],"documents":[]}"#,
    );
    let mut home = home.reopen();
    assert!(DocumentRegistry::read(&mut home.db).is_err());
}

#[test]
fn ori_t_0197_owner_derived_matches_kind_for_apply() {
    use ori_core::document::DocumentEvent;
    use ori_core::types::DocumentKind;
    let kind = DocumentKind::Brief;
    let owner = owner_for_kind(&kind);
    assert_eq!(owner, Seat::ProductOwner);
    let doc = ori_core::types::Document {
        id: ulid(1),
        product_id: pid(),
        path: "spec/PROJECT_BRIEF.md".to_owned(),
        kind,
        set: DocumentSet::Foundation,
        state: DocumentState::Missing,
        approved_by: None,
        approved_at: None,
        verified_against_code_at: None,
    };
    assert!(DocumentEvent::Created.fires_from(doc.state));
    doc.apply(&DocumentEvent::Created, owner).expect("apply");
}

#[test]
fn ori_t_0197_document_registered_birth_edge() {
    let mut home = Home::new("reg-edge");
    let mut reg = setup_new_product(&mut home);
    let id = ulid(77).as_str().to_owned();
    reg.record(
        &mut home.db,
        ts(AT),
        Actor::Human(human()),
        DOCUMENT_REGISTERED_KIND,
        &format!(
            r#"{{"document_id":"{id}","kind":"api_spec","path":"spec/API_SPEC.md","set":"phase"}}"#
        ),
    )
    .expect("registered");
    let doc = reg.documents().find(|d| d.id.as_str() == id).expect("doc");
    assert_eq!(doc.state, DocumentState::Missing);
}

#[test]
fn ori_t_0197_ports_port_error_denied_carries_methodology_ref() {
    let err = PortError::Denied {
        code: "example",
        methodology_ref: ori_core::error::MethodologyRef {
            section: 18,
            subsection: None,
        },
    };
    assert_eq!(err.methodology_ref().unwrap().section, 18);
}
