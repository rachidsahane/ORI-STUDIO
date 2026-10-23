//! Integration tests for ORI-T-0199: `document.submitted` and `flows.documents.submit`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use ori_core::types::{Actor, DocumentState, Id, Seat, Timestamp};
use ori_flows::documents::Documents;
use ori_flows::registry::{
    DOCUMENT_APPROVED_KIND, DOCUMENT_CREATED_KIND, DOCUMENT_SUBMITTED_KIND, DocumentRegistry,
    DocumentWire, EventDefect, FOUNDATION, PRODUCT_CREATED_KIND, ProductCreatedWire, RegistryError,
    SeatWire,
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
            "ori-t-0199-{label}-{}-{unique}",
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
    scratch: Scratch,
}

impl Home {
    fn new(label: &str) -> Self {
        let scratch = Scratch::new(label);
        let db = ProductDb::open(&scratch.path, PRODUCT, ts(1)).expect("open");
        Self { db, scratch }
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

/// Holder of `product_owner`, the seat that owns the brief.
fn product_owner() -> Id {
    let index = Seat::ALL
        .iter()
        .position(|seat| *seat == Seat::ProductOwner)
        .expect("seat");
    ulid(200 + index as u64)
}

fn pid() -> Id {
    Id::parse(PRODUCT).expect("product")
}

fn good_wire(root: &Path) -> ProductCreatedWire {
    let repo = root.join("repo");
    fs::create_dir_all(&repo).expect("repo");
    ProductCreatedWire {
        name: "demo".to_owned(),
        repo_path: fs::canonicalize(&repo).unwrap().display().to_string(),
        repo_remote: None,
        aicd_version: "0.3".to_owned(),
        origin: "new".to_owned(),
        seats: Seat::ALL
            .iter()
            .enumerate()
            .map(|(i, seat)| SeatWire {
                seat: seat.as_str().to_owned(),
                holder: ulid(200 + i as u64).as_str().to_owned(),
            })
            .collect(),
        documents: FOUNDATION
            .iter()
            .enumerate()
            .map(|(i, row)| DocumentWire {
                id: ulid(100 + i as u64).as_str().to_owned(),
                kind: row.kind.to_owned(),
                path: row.path.to_owned(),
                set: "foundation".to_owned(),
            })
            .collect(),
    }
}

fn setup(home: &mut Home) -> DocumentRegistry {
    let wire = good_wire(&home.scratch.path);
    let mut reg = DocumentRegistry::read(&mut home.db).expect("read");
    reg.create_product(&mut home.db, ts(AT), Actor::Human(human()), wire)
        .expect("create");
    reg
}

fn brief() -> Id {
    ulid(100)
}

fn created_payload(doc: &Id) -> String {
    format!(
        r#"{{"document_id":"{}","pr":"1","content_len":1,"citations_unresolved":0}}"#,
        doc.as_str()
    )
}

fn submitted_payload(doc: &Id) -> String {
    format!(r#"{{"document_id":"{}"}}"#, doc.as_str())
}

fn approved_payload(doc: &Id) -> String {
    format!(
        r#"{{"document_id":"{}","seat":"product_owner"}}"#,
        doc.as_str()
    )
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

fn tip(home: &mut Home) -> Option<u64> {
    EventLog::verify(home.db.connection()).unwrap().tip_seq
}

fn state_of(reg: &DocumentRegistry, doc: &Id) -> DocumentState {
    reg.documents().find(|d| d.id == *doc).expect("doc").state
}

fn draft(home: &mut Home, reg: &mut DocumentRegistry) {
    reg.record(
        &mut home.db,
        ts(AT),
        Actor::Human(human()),
        DOCUMENT_CREATED_KIND,
        &created_payload(&brief()),
    )
    .expect("created");
}

#[test]
fn ori_t_0199_submit_from_draft_appends_submitted() {
    let mut home = Home::new("sub-draft");
    let mut reg = setup(&mut home);
    draft(&mut home, &mut reg);
    let seq = Documents::new(&mut reg)
        .submit(&mut home.db, ts(AT + 1), Actor::Agent(agent()), &brief())
        .expect("submit");
    assert_eq!(seq, 3);
    assert_eq!(tip(&mut home), Some(3));
    let events = EventLog::read_range(home.db.connection(), 3, 3).unwrap();
    assert_eq!(events[0].kind(), DOCUMENT_SUBMITTED_KIND);
    assert_eq!(state_of(&reg, &brief()), DocumentState::UnderReview);
}

#[test]
fn ori_t_0199_submit_refuses_unknown_document() {
    let mut home = Home::new("sub-unknown");
    let mut reg = setup(&mut home);
    assert_eq!(
        Documents::new(&mut reg).submit(&mut home.db, ts(AT), Actor::Human(human()), &ulid(999)),
        Err(RegistryError::Refused {
            defect: EventDefect::UnknownDocument,
        })
    );
    assert_eq!(tip(&mut home), Some(1));
    assert!(reg.blocked().is_none());
    // A registered document still in Missing is not submittable either.
    assert_eq!(
        Documents::new(&mut reg).submit(&mut home.db, ts(AT), Actor::Human(human()), &brief()),
        Err(RegistryError::Refused {
            defect: EventDefect::Transition,
        })
    );
    assert_eq!(tip(&mut home), Some(1));
    assert!(reg.blocked().is_none());
}

#[test]
fn ori_t_0199_submit_payload_refuses_array() {
    let mut home = Home::new("sub-arr");
    let mut reg = setup(&mut home);
    draft(&mut home, &mut reg);
    let array = format!(r#"["{}"]"#, brief().as_str());
    assert_eq!(
        reg.record(
            &mut home.db,
            ts(AT),
            Actor::Human(human()),
            DOCUMENT_SUBMITTED_KIND,
            &array,
        ),
        Err(RegistryError::Refused {
            defect: EventDefect::Payload,
        })
    );
    assert_eq!(tip(&mut home), Some(2));
    plant(
        &mut home,
        Actor::Human(human()),
        DOCUMENT_SUBMITTED_KIND,
        &array,
    );
    assert_eq!(
        DocumentRegistry::read(&mut home.db).map(|_| ()),
        Err(RegistryError::Event {
            seq: 3,
            defect: EventDefect::Payload,
        })
    );
}

#[test]
fn ori_t_0199_submit_payload_refuses_unknown_field() {
    let mut home = Home::new("sub-field");
    let mut reg = setup(&mut home);
    draft(&mut home, &mut reg);
    let extra = format!(
        r#"{{"document_id":"{}","seat":"product_owner"}}"#,
        brief().as_str()
    );
    assert_eq!(
        reg.record(
            &mut home.db,
            ts(AT),
            Actor::Human(human()),
            DOCUMENT_SUBMITTED_KIND,
            &extra,
        ),
        Err(RegistryError::Refused {
            defect: EventDefect::Payload,
        })
    );
    assert_eq!(tip(&mut home), Some(2));
    plant(
        &mut home,
        Actor::Human(human()),
        DOCUMENT_SUBMITTED_KIND,
        &extra,
    );
    assert_eq!(
        DocumentRegistry::read(&mut home.db).map(|_| ()),
        Err(RegistryError::Event {
            seq: 3,
            defect: EventDefect::Payload,
        })
    );
}

#[test]
fn ori_t_0199_fold_submitted_after_created() {
    let mut home = Home::new("fold-sub");
    setup(&mut home);
    plant(
        &mut home,
        Actor::Human(human()),
        DOCUMENT_CREATED_KIND,
        &created_payload(&brief()),
    );
    plant(
        &mut home,
        Actor::Agent(agent()),
        DOCUMENT_SUBMITTED_KIND,
        &submitted_payload(&brief()),
    );
    let reg = DocumentRegistry::read(&mut home.db).expect("fold");
    assert!(reg.blocked().is_none());
    assert_eq!(state_of(&reg, &brief()), DocumentState::UnderReview);
    // Submitted from Missing is not an edge of the diagram.
    let mut home = Home::new("fold-sub-missing");
    setup(&mut home);
    plant(
        &mut home,
        Actor::Human(human()),
        DOCUMENT_SUBMITTED_KIND,
        &submitted_payload(&brief()),
    );
    assert_eq!(
        DocumentRegistry::read(&mut home.db).map(|_| ()),
        Err(RegistryError::Event {
            seq: 2,
            defect: EventDefect::Transition,
        })
    );
}

#[test]
fn ori_t_0199_record_refuses_approved_from_draft_without_submit() {
    let mut home = Home::new("ap-draft");
    let mut reg = setup(&mut home);
    draft(&mut home, &mut reg);
    assert_eq!(
        reg.record(
            &mut home.db,
            ts(AT + 1),
            Actor::Human(product_owner()),
            DOCUMENT_APPROVED_KIND,
            &approved_payload(&brief()),
        ),
        Err(RegistryError::Refused {
            defect: EventDefect::Transition,
        })
    );
    assert_eq!(tip(&mut home), Some(2));
    assert!(reg.blocked().is_none());
    assert_eq!(state_of(&reg, &brief()), DocumentState::Draft);
}

#[test]
fn ori_t_0199_approve_after_submit_succeeds() {
    let mut home = Home::new("ap-sub");
    let mut reg = setup(&mut home);
    draft(&mut home, &mut reg);
    Documents::new(&mut reg)
        .submit(&mut home.db, ts(AT + 1), Actor::Human(human()), &brief())
        .expect("submit");
    reg.record(
        &mut home.db,
        ts(AT + 2),
        Actor::Human(product_owner()),
        DOCUMENT_APPROVED_KIND,
        &approved_payload(&brief()),
    )
    .expect("approved");
    let doc = reg.documents().find(|d| d.id == brief()).expect("doc");
    assert_eq!(doc.state, DocumentState::Approved);
    assert_eq!(doc.approved_by, Some(Seat::ProductOwner));
    assert_eq!(doc.approved_at, Some(ts(AT + 2)));
    let reread = DocumentRegistry::read(&mut home.db).expect("reread");
    assert_eq!(state_of(&reread, &brief()), DocumentState::Approved);
}

#[test]
fn ori_t_0199_submit_refuses_when_registry_locked() {
    let mut home = Home::new("sub-lock");
    let mut reg = setup(&mut home);
    draft(&mut home, &mut reg);
    plant(
        &mut home,
        Actor::Human(human()),
        PRODUCT_CREATED_KIND,
        r#"{"name":"x","repo_path":"/tmp","repo_remote":"https://u:p@h","aicd_version":"0.3","origin":"new","seats":[],"documents":[]}"#,
    );
    let locked = Err(RegistryError::Event {
        seq: 3,
        defect: EventDefect::SecretInUrl,
    });
    assert_eq!(
        Documents::new(&mut reg).submit(&mut home.db, ts(AT), Actor::Human(human()), &brief()),
        locked
    );
    assert_eq!(reg.blocked(), Some((3, &EventDefect::SecretInUrl)));
    assert_eq!(
        Documents::new(&mut reg).submit(&mut home.db, ts(AT), Actor::Human(human()), &brief()),
        locked
    );
    assert_eq!(tip(&mut home), Some(3));
}

#[test]
fn ori_t_0199_fold_refuses_approved_from_draft_planted() {
    let mut home = Home::new("fold-ap-draft");
    setup(&mut home);
    plant(
        &mut home,
        Actor::Human(human()),
        DOCUMENT_CREATED_KIND,
        &created_payload(&brief()),
    );
    plant(
        &mut home,
        Actor::Human(product_owner()),
        DOCUMENT_APPROVED_KIND,
        &approved_payload(&brief()),
    );
    assert_eq!(
        DocumentRegistry::read(&mut home.db).map(|_| ()),
        Err(RegistryError::Event {
            seq: 3,
            defect: EventDefect::Transition,
        })
    );
}

#[test]
fn ori_t_0199_documents_submit_delegates_to_record() {
    let mut home = Home::new("sub-delegate");
    let mut reg = setup(&mut home);
    draft(&mut home, &mut reg);
    let actor = Actor::Agent(agent());
    Documents::new(&mut reg)
        .submit(&mut home.db, ts(AT + 5), actor.clone(), &brief())
        .expect("submit");
    let event = EventLog::read_range(home.db.connection(), 3, 3)
        .unwrap()
        .remove(0);
    assert_eq!(event.kind(), DOCUMENT_SUBMITTED_KIND);
    assert_eq!(event.payload(), submitted_payload(&brief()));
    assert_eq!(event.actor(), &actor);
    assert_eq!(event.at(), ts(AT + 5));
    assert_eq!(event.ticket_id(), None);
    // The appended event is the one record would fold: a fresh read agrees.
    let reread = DocumentRegistry::read(&mut home.db).expect("reread");
    assert_eq!(state_of(&reread, &brief()), DocumentState::UnderReview);
    assert_eq!(state_of(&reg, &brief()), DocumentState::UnderReview);
    // A second submit from UnderReview is refused before append.
    assert_eq!(
        Documents::new(&mut reg).submit(&mut home.db, ts(AT + 6), actor, &brief()),
        Err(RegistryError::Refused {
            defect: EventDefect::Transition,
        })
    );
    assert_eq!(tip(&mut home), Some(3));
}
