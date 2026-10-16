use super::{
    hash_token, limit, next, CatalogCache, Item, Snapshot, CACHE_AGE, MAX_BYTES, MAX_ITEMS,
    PAGE_SIZE,
};
use nexushub_core::services::codex_desktop::{DesktopCatalogRequest, DesktopMode};
use serde_json::json;
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

fn model_item(index: usize, name: &str, description: &str) -> Item {
    Item {
        id: format!("model-{index}"),
        kind: "model".into(),
        name: name.into(),
        description: description.into(),
        efforts: vec!["low".into(), "high".into()],
        default_effort: Some("low".into()),
        grant: None,
    }
}

fn snapshot(items: Vec<Item>, modes: &[&str]) -> Snapshot {
    let bytes = items.iter().map(Item::bytes).sum();
    Snapshot {
        key: "fixture-key".into(),
        revision: 9,
        thread: "fixture-thread".into(),
        kind: "models".into(),
        nonce: "fixture-nonce".into(),
        created: Instant::now(),
        items,
        modes: modes.iter().map(|mode| (*mode).into()).collect(),
        bytes,
    }
}

fn request(query: &str, cursor: Option<String>) -> DesktopCatalogRequest {
    DesktopCatalogRequest {
        revision: 9,
        session_id: None,
        project_handle: None,
        kind: "models".into(),
        query: query.into(),
        cursor,
    }
}

fn cursor(snapshot: &Snapshot, query: &str, offset: usize) -> String {
    let query = query.trim().to_lowercase();
    let stamp = hash_token(&format!("{}\n{query}", snapshot.nonce));
    format!("{stamp}.{offset}")
}

#[test]
fn page_cursor_is_query_bound_and_query_normalization_is_stable() {
    let items = (0..205)
        .map(|index| model_item(index, &format!("fixture item {index}"), ""))
        .collect();
    let snapshot = snapshot(items, &[]);
    let first = snapshot.page(&request("fixture", None)).unwrap();
    assert_eq!(first.items.len(), PAGE_SIZE);
    let next_cursor = first.next_cursor.unwrap();

    let second = snapshot
        .page(&request(" Fixture ", Some(next_cursor.clone())))
        .unwrap();
    assert_eq!(second.items.len(), PAGE_SIZE);
    assert_eq!(second.items[0].name, "fixture item 100");
    assert!(snapshot.page(&request("other", Some(next_cursor))).is_err());
}

#[test]
fn page_cursor_fails_after_snapshot_replacement() {
    let items: Vec<_> = (0..101)
        .map(|index| model_item(index, &format!("fixture item {index}"), ""))
        .collect();
    let original = snapshot(items.clone(), &[]);
    let stale_cursor = original
        .page(&request("fixture", None))
        .unwrap()
        .next_cursor
        .unwrap();
    let mut replacement = snapshot(items, &[]);
    replacement.nonce = "replacement-nonce".into();

    assert!(replacement
        .page(&request("fixture", Some(stale_cursor)))
        .is_err());
}

#[test]
fn page_rejects_malformed_misaligned_and_filtered_out_offsets() {
    let mut items: Vec<_> = (0..150)
        .map(|index| model_item(index, &format!("match item {index}"), ""))
        .collect();
    items.extend((150..205).map(|index| model_item(index, &format!("other item {index}"), "")));
    let snapshot = snapshot(items, &[]);
    let valid_stamp = cursor(&snapshot, "match", 0)
        .strip_suffix(".0")
        .unwrap()
        .to_owned();

    for invalid in [
        "malformed".to_owned(),
        format!("{valid_stamp}.not-a-number"),
        format!("{valid_stamp}.1"),
        format!("{valid_stamp}.300"),
        format!("{valid_stamp}.200"),
    ] {
        assert!(snapshot.page(&request("match", Some(invalid))).is_err());
    }
}

#[test]
fn model_settings_require_current_catalog_identity_and_allowlisted_choices() {
    let item = model_item(1, "Fixture model", "");
    let mut cache = CatalogCache::default();
    cache.entries.push(snapshot(vec![item], &["plan"]));

    assert!(cache
        .validate_settings(
            9,
            "fixture-thread",
            "model-1",
            Some("high"),
            DesktopMode::Plan
        )
        .is_ok());
    assert!(cache
        .validate_settings(
            10,
            "fixture-thread",
            "model-1",
            Some("high"),
            DesktopMode::Plan,
        )
        .is_err());
    assert!(cache
        .validate_settings(9, "other-thread", "model-1", None, DesktopMode::Plan)
        .is_err());
    assert!(cache
        .validate_settings(
            9,
            "fixture-thread",
            "unlisted-model",
            None,
            DesktopMode::Plan
        )
        .is_err());
    assert!(cache
        .validate_settings(
            9,
            "fixture-thread",
            "model-1",
            Some("unlisted-effort"),
            DesktopMode::Plan,
        )
        .is_err());
    assert!(cache
        .validate_settings(9, "fixture-thread", "model-1", None, DesktopMode::Default)
        .is_err());
}

#[test]
fn expired_model_catalog_cannot_authorize_settings() {
    let mut cache = CatalogCache::default();
    let mut entry = snapshot(vec![model_item(1, "Fixture model", "")], &["default"]);
    entry.created = Instant::now() - CACHE_AGE - Duration::from_secs(1);
    cache.entries.push(entry);

    assert!(cache
        .validate_settings(9, "fixture-thread", "model-1", None, DesktopMode::Default)
        .is_err());
}

#[test]
fn catalog_item_and_byte_budgets_are_enforced() {
    let too_many: Vec<_> = (0..=MAX_ITEMS)
        .map(|index| model_item(index, "fixture", ""))
        .collect();
    assert!(limit(&too_many).is_err());

    let too_large = model_item(1, "fixture", &"x".repeat(MAX_BYTES));
    assert!(limit(&[too_large]).is_err());
}

#[test]
fn helper_pagination_rejects_duplicate_invalid_and_over_budget_cursors() {
    let mut seen = HashSet::new();
    assert_eq!(next(&json!({}), &mut seen).unwrap(), None);
    assert_eq!(
        next(&json!({"nextCursor":"fixture-cursor"}), &mut seen).unwrap(),
        Some("fixture-cursor".into())
    );
    assert!(next(&json!({"nextCursor":"fixture-cursor"}), &mut seen).is_err());
    assert!(next(&json!({"nextCursor":""}), &mut seen).is_err());
    assert!(next(&json!({"nextCursor":42}), &mut seen).is_err());
    assert!(next(&json!({"nextCursor": "x".repeat(4097)}), &mut seen).is_err());

    let mut bounded = HashSet::new();
    for index in 0..200 {
        let value = format!("cursor-{index}");
        assert_eq!(
            next(&json!({"nextCursor":value}), &mut bounded).unwrap(),
            Some(value)
        );
    }
    assert!(next(&json!({"nextCursor":"cursor-over-budget"}), &mut bounded).is_err());
}
