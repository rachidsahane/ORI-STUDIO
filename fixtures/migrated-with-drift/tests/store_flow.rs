//! The store and the router, exercised together against a real file.

use std::fs;
use std::path::PathBuf;

use parceltrack::model::{Parcel, Status};
use parceltrack::server::route;
use parceltrack::store::{Store, StoreError};

fn scratch_file(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("parceltrack-test-{}-{name}", std::process::id()));
    fs::create_dir_all(&directory).expect("create scratch directory");
    directory.join("parcels.tsv")
}

#[test]
fn a_parcel_survives_a_reload_and_walks_to_delivered() {
    let path = scratch_file("walk");
    let mut store = Store::open(&path).expect("open an absent file as empty");
    store
        .insert(Parcel::new("PK-1", "12 Quay Road").expect("valid parcel"))
        .expect("insert");

    let reloaded = Store::open(&path).expect("reload");
    assert_eq!(
        reloaded.get("PK-1").map(|p| p.status),
        Some(Status::Registered)
    );

    let mut store = reloaded;
    assert_eq!(store.advance("PK-1").expect("advance"), Status::InTransit);
    assert_eq!(
        store.advance("PK-1").expect("advance"),
        Status::OutForDelivery
    );
    assert_eq!(store.advance("PK-1").expect("advance"), Status::Delivered);
    assert!(matches!(store.advance("PK-1"), Err(StoreError::Final(_))));
    assert!(matches!(
        store.mark_returned("PK-1"),
        Err(StoreError::Final(_))
    ));
}

#[test]
fn a_duplicate_id_is_refused() {
    let path = scratch_file("duplicate");
    let mut store = Store::open(&path).expect("open");
    store
        .insert(Parcel::new("PK-2", "3 Mill Lane").expect("valid parcel"))
        .expect("insert");
    let again = store.insert(Parcel::new("PK-2", "4 Mill Lane").expect("valid parcel"));
    assert!(matches!(again, Err(StoreError::Duplicate(_))));
}

#[test]
fn the_router_answers_lookups_and_refuses_the_rest() {
    let path = scratch_file("route");
    let mut store = Store::open(&path).expect("open");
    store
        .insert(Parcel::new("PK-3", "9 Harbour Street").expect("valid parcel"))
        .expect("insert");

    assert_eq!(
        route(&store, "GET", "/parcels/PK-3"),
        (200, "PK-3\tregistered\n".to_owned())
    );
    assert_eq!(
        route(&store, "GET", "/parcels/PK-9"),
        (404, "not found\n".to_owned())
    );
    assert_eq!(route(&store, "POST", "/parcels/PK-3").0, 405);
}
