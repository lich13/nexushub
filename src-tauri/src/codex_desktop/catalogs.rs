//! Bounded projections of native catalogs. Only projected items survive a
//! helper request; opaque grants bind references to the selected local thread.
use super::{
    desktop_error,
    selection::{GrantValue, Grants},
};
use nexushub_core::{
    codex::desktop::{
        helper::{CatalogQuery, DesktopHelper},
        references::NativeReference,
    },
    security::hash_token,
    services::codex_desktop::*,
};
use serde_json::Value;
use std::{
    collections::HashSet,
    path::Path,
    time::{Duration, Instant},
};

const MAX_ENTRIES: usize = 8;
const MAX_ITEMS: usize = 20_000;
const MAX_BYTES: usize = 8 * 1024 * 1024;
const PAGE_SIZE: usize = 100;
const CACHE_AGE: Duration = Duration::from_secs(60);

#[derive(Clone)]
struct Item {
    id: String,
    kind: String,
    name: String,
    description: String,
    efforts: Vec<String>,
    default_effort: Option<String>,
    grant: Option<GrantValue>,
}
impl Item {
    fn bytes(&self) -> usize {
        self.id.len()
            + self.kind.len()
            + self.name.len()
            + self.description.len()
            + self.efforts.iter().map(String::len).sum::<usize>()
            + self.default_effort.as_ref().map_or(0, String::len)
            + match &self.grant {
                Some(GrantValue::Reference(reference)) => {
                    serde_json::to_vec(reference).map_or(MAX_BYTES, |bytes| bytes.len())
                }
                _ => 0,
            }
            + 256
    }
}
struct Snapshot {
    key: String,
    revision: u64,
    thread: String,
    kind: String,
    nonce: String,
    created: Instant,
    items: Vec<Item>,
    modes: Vec<String>,
    bytes: usize,
}
impl Snapshot {
    fn page(&self, request: &DesktopCatalogRequest) -> Result<Page, String> {
        let query = request.query.trim().to_lowercase();
        let stamp = hash_token(&format!("{}\n{query}", self.nonce));
        let offset = match request.cursor.as_deref() {
            None => 0,
            Some(cursor) => {
                let (prefix, offset) = cursor.split_once('.').ok_or("引用页已失效")?;
                if prefix != stamp {
                    return Err("引用页已失效".into());
                }
                offset
                    .parse::<usize>()
                    .ok()
                    .filter(|offset| *offset <= self.items.len() && *offset % PAGE_SIZE == 0)
                    .ok_or("引用页已失效")?
            }
        };
        let filtered: Vec<_> = self
            .items
            .iter()
            .filter(|item| {
                query.is_empty()
                    || item.name.to_lowercase().contains(&query)
                    || item.description.to_lowercase().contains(&query)
            })
            .collect();
        if offset > filtered.len() {
            return Err("引用页已失效".into());
        }
        let items = filtered
            .iter()
            .skip(offset)
            .take(PAGE_SIZE)
            .map(|item| (*item).clone())
            .collect();
        let next_cursor = (offset + PAGE_SIZE < filtered.len())
            .then(|| format!("{stamp}.{}", offset + PAGE_SIZE));
        Ok(Page { items, next_cursor })
    }
}
#[derive(Default)]
pub(super) struct CatalogCache {
    entries: Vec<Snapshot>,
}
pub(super) struct Page {
    items: Vec<Item>,
    next_cursor: Option<String>,
}
impl Page {
    pub fn project(self, revision: u64, thread: &str, grants: &mut Grants) -> DesktopCatalog {
        DesktopCatalog {
            next_cursor: self.next_cursor,
            items: self
                .items
                .into_iter()
                .map(|item| DesktopCatalogItem {
                    handle: item.grant.map_or(item.id, |value| {
                        grants.insert(revision, Some(thread.into()), value)
                    }),
                    kind: item.kind,
                    name: item.name,
                    description: item.description,
                    reasoning_efforts: item.efforts,
                    default_reasoning_effort: item.default_effort,
                })
                .collect(),
        }
    }
}
impl CatalogCache {
    pub async fn read(
        &mut self,
        helper: &DesktopHelper,
        cwd: &Path,
        thread: &str,
        request: &DesktopCatalogRequest,
    ) -> Result<Page, String> {
        if request
            .cursor
            .as_ref()
            .is_some_and(|cursor| cursor.len() > 128)
            || request.kind.len() > 128
            || request.project_handle.is_some()
        {
            return Err("引用请求无效".into());
        }
        self.entries.retain(|entry| {
            entry.created.elapsed() < CACHE_AGE && entry.revision == request.revision
        });
        let key = hash_token(&format!("{}\n{thread}\n{}", request.revision, request.kind));
        let position =
            if let Some(position) = self.entries.iter().position(|entry| entry.key == key) {
                position
            } else {
                if request.cursor.is_some() {
                    return Err("引用列表已更新，请重新搜索".into());
                }
                let (items, modes) = fetch(helper, cwd, thread, &request.kind).await?;
                let bytes = items.iter().map(Item::bytes).sum::<usize>();
                if items.len() > MAX_ITEMS || bytes > MAX_BYTES {
                    return Err("引用列表超出容量限制".into());
                }
                while !self.entries.is_empty()
                    && (self.entries.len() >= MAX_ENTRIES
                        || self
                            .entries
                            .iter()
                            .map(|entry| entry.bytes)
                            .sum::<usize>()
                            .saturating_add(bytes)
                            > MAX_BYTES)
                {
                    self.entries.remove(0);
                }
                self.entries.push(Snapshot {
                    key,
                    revision: request.revision,
                    thread: thread.into(),
                    kind: request.kind.clone(),
                    nonce: hash_token(&nexushub_core::crypto::SecretBox::random_key_material()),
                    created: Instant::now(),
                    items,
                    modes,
                    bytes,
                });
                self.entries.len() - 1
            };
        self.entries[position].page(request)
    }

    pub fn validate_settings(
        &self,
        revision: u64,
        thread: &str,
        model: &str,
        effort: Option<&str>,
        mode: DesktopMode,
    ) -> Result<(), String> {
        let entry = self
            .entries
            .iter()
            .find(|entry| {
                entry.revision == revision
                    && entry.thread == thread
                    && entry.kind == "models"
                    && entry.created.elapsed() < CACHE_AGE
            })
            .ok_or("模型列表已失效，请重新连接后选择")?;
        let item = entry
            .items
            .iter()
            .find(|item| item.id == model)
            .ok_or("所选模型当前不可用")?;
        let mode = match mode {
            DesktopMode::Default => "default",
            DesktopMode::Plan => "plan",
        };
        if !entry.modes.iter().any(|value| value == mode)
            || effort.is_some_and(|effort| !item.efforts.iter().any(|value| value == effort))
        {
            return Err("所选模式或思考强度当前不可用".into());
        }
        Ok(())
    }
}

async fn fetch(
    helper: &DesktopHelper,
    cwd: &Path,
    thread: &str,
    kind: &str,
) -> Result<(Vec<Item>, Vec<String>), String> {
    let mut items = Vec::new();
    let mut modes = Vec::new();
    match kind {
        "models" => {
            let mut cursor = None;
            let mut seen = HashSet::new();
            loop {
                let page = helper
                    .catalog(CatalogQuery::Models { cursor })
                    .await
                    .map_err(desktop_error)?;
                for value in array(&page, "data")? {
                    if value["hidden"] == true {
                        continue;
                    }
                    let id = identifier(value, "model")?;
                    let efforts = array(value, "supportedReasoningEfforts")?
                        .iter()
                        .map(|effort| identifier(effort, "reasoningEffort"))
                        .collect::<Result<Vec<_>, _>>()?;
                    let default_effort = Some(identifier(value, "defaultReasoningEffort")?);
                    items.push(Item {
                        id,
                        kind: "model".into(),
                        name: identifier(value, "displayName")?,
                        description: description(value["description"].as_str()),
                        efforts,
                        default_effort,
                        grant: None,
                    });
                }
                limit(&items)?;
                cursor = next(&page, &mut seen)?;
                if cursor.is_none() {
                    break;
                }
            }
            let result = helper
                .catalog(CatalogQuery::Modes)
                .await
                .map_err(desktop_error)?;
            modes = array(&result, "data")?
                .iter()
                .filter_map(|value| value["mode"].as_str())
                .filter(|mode| matches!(*mode, "default" | "plan"))
                .map(str::to_owned)
                .collect();
        }
        "skills" => {
            let result = helper
                .catalog(CatalogQuery::Skills {
                    cwd: cwd.to_owned(),
                })
                .await
                .map_err(desktop_error)?;
            for entry in array(&result, "data")? {
                if entry["cwd"].as_str() != cwd.to_str() {
                    continue;
                }
                append_skills(&mut items, array(entry, "skills")?)?;
                if items.is_empty()
                    && entry["errors"]
                        .as_array()
                        .is_some_and(|errors| !errors.is_empty())
                {
                    return Err("部分技能目录无法读取，请在 Codex 中检查".into());
                }
            }
        }
        "plugins" => {
            let result = helper
                .catalog(CatalogQuery::Plugins {
                    cwd: cwd.to_owned(),
                })
                .await
                .map_err(desktop_error)?;
            for marketplace in array(&result, "marketplaces")? {
                let market_name = identifier(marketplace, "name")?;
                for plugin in array(marketplace, "plugins")? {
                    if !usable_plugin(plugin) {
                        continue;
                    }
                    let name = identifier(plugin, "name")?;
                    let display = plugin["interface"]["displayName"].as_str().unwrap_or(&name);
                    let grant = GrantValue::Reference(
                        NativeReference::plugin(name.clone(), &market_name)
                            .map_err(desktop_error)?,
                    );
                    items.push(Item {
                        id: identifier(plugin, "id")?,
                        kind: "plugin".into(),
                        name: description(Some(display)),
                        description: description(plugin["interface"]["shortDescription"].as_str()),
                        efforts: vec![],
                        default_effort: None,
                        grant: Some(grant),
                    });
                }
            }
        }
        "apps" => {
            items = apps(helper, thread).await?;
        }
        _ => return Err("引用类型不支持".into()),
    }
    limit(&items)?;
    let mut seen = HashSet::new();
    items.retain(|item| seen.insert((item.kind.clone(), item.id.clone())));
    Ok((items, modes))
}

async fn apps(helper: &DesktopHelper, thread: &str) -> Result<Vec<Item>, String> {
    let mut items = Vec::new();
    let mut cursor = None;
    let mut seen = HashSet::new();
    loop {
        let page = helper
            .catalog(CatalogQuery::Apps {
                thread_id: Some(thread.into()),
                cursor,
            })
            .await
            .map_err(desktop_error)?;
        for app in array(&page, "data")? {
            if app["isAccessible"] != true || app["isEnabled"] == false {
                continue;
            }
            let id = identifier(app, "id")?;
            let name = identifier(app, "name")?;
            let reference = app_reference(&id, &name)?;
            items.push(Item {
                id,
                kind: "app".into(),
                name,
                description: description(app["description"].as_str()),
                efforts: vec![],
                default_effort: None,
                grant: Some(GrantValue::Reference(reference)),
            });
        }
        limit(&items)?;
        cursor = next(&page, &mut seen)?;
        if cursor.is_none() {
            break;
        }
    }
    Ok(items)
}

// Verified against the installed native UserInput::Mention protocol; the ID
// remains opaque and comes only from the native accessible application list.
fn app_reference(id: &str, name: &str) -> Result<NativeReference, String> {
    NativeReference::app(name.to_owned(), format!("app://{id}")).map_err(desktop_error)
}
fn append_skills(items: &mut Vec<Item>, skills: &[Value]) -> Result<(), String> {
    for skill in skills {
        if skill["enabled"] != true {
            continue;
        }
        let Some(path) = skill["path"].as_str() else {
            continue;
        };
        let name = identifier(skill, "name")?;
        let reference = NativeReference::skill(name.clone(), path.into()).map_err(desktop_error)?;
        items.push(Item {
            id: path.into(),
            kind: "skill".into(),
            name,
            description: description(
                skill["shortDescription"]
                    .as_str()
                    .or_else(|| skill["description"].as_str()),
            ),
            efforts: vec![],
            default_effort: None,
            grant: Some(GrantValue::Reference(reference)),
        });
    }
    Ok(())
}
fn usable_plugin(plugin: &Value) -> bool {
    plugin["installed"] == true
        && plugin["enabled"] == true
        && plugin["disabledReason"].is_null()
        && matches!(plugin["availability"].as_str(), None | Some("AVAILABLE"))
}
fn array<'a>(value: &'a Value, key: &str) -> Result<&'a [Value], String> {
    value[key]
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| "Codex 返回的目录格式不兼容".into())
}
fn identifier(value: &Value, key: &str) -> Result<String, String> {
    value[key]
        .as_str()
        .filter(|value| {
            !value.is_empty() && value.len() <= 1024 && !value.chars().any(char::is_control)
        })
        .map(str::to_owned)
        .ok_or_else(|| "Codex 返回的目录格式不兼容".into())
}
fn description(value: Option<&str>) -> String {
    value
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .take(1024)
        .collect()
}
fn limit(items: &[Item]) -> Result<(), String> {
    if items.len() > MAX_ITEMS || items.iter().map(Item::bytes).sum::<usize>() > MAX_BYTES {
        Err("引用列表超出容量限制".into())
    } else {
        Ok(())
    }
}
fn next(page: &Value, seen: &mut HashSet<String>) -> Result<Option<String>, String> {
    match &page["nextCursor"] {
        Value::Null => Ok(None),
        Value::String(cursor)
            if !cursor.is_empty()
                && cursor.len() <= 4096
                && seen.len() < 200
                && seen.insert(cursor.clone()) =>
        {
            Ok(Some(cursor.clone()))
        }
        _ => Err("Codex 目录分页无法继续".into()),
    }
}

#[cfg(test)]
#[path = "catalogs_tests.rs"]
mod tests;
