use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result, anyhow};
use serde_yaml::{Mapping, Value};

use crate::collection::Request;
use crate::model::{BodyMode, KeyValueRow, RequestDraft};

/// Reads a collection directory, returning its name and requests.
pub fn load(root: impl AsRef<Path>) -> Result<(String, Vec<Request>)> {
    let root = root.as_ref().to_path_buf();
    let collection_file = root.join("opencollection.yml");
    let collection_value = read_yaml_value(&collection_file)
        .with_context(|| format!("failed to read {}", collection_file.display()))?;
    let name = string_at(&collection_value, &["info", "name"])
        .or_else(|| string_at(&collection_value, &["name"]))
        .unwrap_or_else(|| {
            root.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Collection")
                .to_string()
        });

    let mut requests = Vec::new();
    collect_requests(&root, &root, &mut requests)?;
    requests.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    Ok((name, requests))
}

pub fn save_request(request: &mut Request) -> Result<()> {
    write_draft_to_value(&mut request.raw, &request.draft);
    let serialized = serde_yaml::to_string(&request.raw)?;
    fs::write(&request.path, serialized)
        .with_context(|| format!("failed to write {}", request.path.display()))?;
    Ok(())
}

pub fn create_request(root: &Path, name: &str) -> Result<Request> {
    let file_name = slugify(name);
    let path = unique_request_path(root, &file_name);
    let draft = RequestDraft {
        name: name.to_string(),
        ..RequestDraft::default()
    };
    let mut raw = Value::Mapping(Mapping::new());
    write_draft_to_value(&mut raw, &draft);
    let mut request = Request {
        path: path.clone(),
        relative_path: path.strip_prefix(root).unwrap_or(&path).to_path_buf(),
        draft,
        raw,
        parse_error: None,
    };
    save_request(&mut request)?;
    Ok(request)
}

fn collect_requests(root: &Path, dir: &Path, requests: &mut Vec<Request>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_requests(root, &path, requests)?;
            continue;
        }

        if path.extension().and_then(|ext| ext.to_str()) != Some("yml") {
            continue;
        }

        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if matches!(file_name, "opencollection.yml" | "folder.yml") {
            continue;
        }

        let relative_path = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        let loaded = match read_yaml_value(&path) {
            Ok(raw) => {
                let draft = draft_from_value(&raw, &path);
                Request {
                    path,
                    relative_path,
                    draft,
                    raw,
                    parse_error: None,
                }
            }
            Err(error) => Request {
                path,
                relative_path,
                draft: RequestDraft::default(),
                raw: Value::Mapping(Mapping::new()),
                parse_error: Some(error.to_string()),
            },
        };
        requests.push(loaded);
    }
    Ok(())
}

fn read_yaml_value(path: &Path) -> Result<Value> {
    let text = fs::read_to_string(path)?;
    Ok(serde_yaml::from_str(&text)?)
}

fn draft_from_value(raw: &Value, path: &Path) -> RequestDraft {
    let name = string_at(raw, &["info", "name"])
        .or_else(|| string_at(raw, &["name"]))
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("Request")
                .to_string()
        });

    let method = string_at(raw, &["http", "method"])
        .or_else(|| string_at(raw, &["method"]))
        .and_then(|method| method.parse().ok())
        .unwrap_or_default();

    let url = string_at(raw, &["http", "url"])
        .or_else(|| string_at(raw, &["url"]))
        .unwrap_or_default();

    let headers = rows_at(raw, &["http", "headers"])
        .or_else(|| rows_at(raw, &["headers"]))
        .unwrap_or_default();
    let params = rows_at(raw, &["http", "params"])
        .or_else(|| rows_at(raw, &["http", "query"]))
        .or_else(|| rows_at(raw, &["params"]))
        .unwrap_or_default();
    let body = body_at(raw);

    RequestDraft {
        name,
        method,
        url,
        headers,
        params,
        body,
    }
}

fn body_at(raw: &Value) -> BodyMode {
    string_at(raw, &["http", "body", "json"])
        .map(BodyMode::Json)
        .or_else(|| string_at(raw, &["http", "body", "raw"]).map(BodyMode::Raw))
        .or_else(|| string_at(raw, &["http", "body", "text"]).map(BodyMode::Raw))
        .or_else(|| string_at(raw, &["body", "json"]).map(BodyMode::Json))
        .or_else(|| string_at(raw, &["body", "raw"]).map(BodyMode::Raw))
        .unwrap_or_default()
}

fn write_draft_to_value(raw: &mut Value, draft: &RequestDraft) {
    set_string(raw, &["info", "name"], &draft.name);
    set_string(raw, &["http", "method"], draft.method.as_str());
    set_string(raw, &["http", "url"], &draft.url);
    set_rows(raw, &["http", "headers"], &draft.headers);
    set_rows(raw, &["http", "params"], &draft.params);

    remove_path(raw, &["http", "body", "json"]);
    remove_path(raw, &["http", "body", "raw"]);
    remove_path(raw, &["http", "body", "text"]);
    match &draft.body {
        BodyMode::None => {}
        BodyMode::Json(body) => set_string(raw, &["http", "body", "json"], body),
        BodyMode::Raw(body) => set_string(raw, &["http", "body", "raw"], body),
    }
}

fn string_at(value: &Value, path: &[&str]) -> Option<String> {
    value_at(value, path).and_then(|value| match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    })
}

fn rows_at(value: &Value, path: &[&str]) -> Option<Vec<KeyValueRow>> {
    match value_at(value, path)? {
        Value::Sequence(items) => Some(
            items
                .iter()
                .filter_map(|item| match item {
                    Value::Mapping(map) => {
                        let key = map_string(map, "name").or_else(|| map_string(map, "key"))?;
                        Some(KeyValueRow {
                            enabled: map_bool(map, "enabled").unwrap_or(true),
                            value: map_string(map, "value").unwrap_or_default(),
                            key,
                        })
                    }
                    _ => None,
                })
                .collect(),
        ),
        Value::Mapping(map) => Some(
            map.iter()
                .filter_map(|(key, value)| {
                    let key = key.as_str()?.to_string();
                    let value = match value {
                        Value::String(value) => value.clone(),
                        Value::Number(value) => value.to_string(),
                        Value::Bool(value) => value.to_string(),
                        _ => String::new(),
                    };
                    Some(KeyValueRow {
                        enabled: true,
                        key,
                        value,
                    })
                })
                .collect(),
        ),
        _ => None,
    }
}

fn value_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for segment in path {
        current = current
            .as_mapping()?
            .get(Value::String((*segment).to_string()))?;
    }
    Some(current)
}

fn set_string(root: &mut Value, path: &[&str], value: &str) {
    set_value(root, path, Value::String(value.to_string()));
}

fn set_rows(root: &mut Value, path: &[&str], rows: &[KeyValueRow]) {
    let rows = rows
        .iter()
        .filter(|row| !row.key.trim().is_empty())
        .map(|row| {
            let mut map = Mapping::new();
            map.insert(Value::String("name".into()), Value::String(row.key.clone()));
            map.insert(
                Value::String("value".into()),
                Value::String(row.value.clone()),
            );
            map.insert(Value::String("enabled".into()), Value::Bool(row.enabled));
            Value::Mapping(map)
        })
        .collect();
    set_value(root, path, Value::Sequence(rows));
}

fn set_value(root: &mut Value, path: &[&str], value: Value) {
    if !matches!(root, Value::Mapping(_)) {
        *root = Value::Mapping(Mapping::new());
    }

    let mut current = root;
    for segment in &path[..path.len() - 1] {
        let map = current.as_mapping_mut().expect("mapping initialized");
        current = map
            .entry(Value::String((*segment).to_string()))
            .or_insert_with(|| Value::Mapping(Mapping::new()));
        if !matches!(current, Value::Mapping(_)) {
            *current = Value::Mapping(Mapping::new());
        }
    }

    current
        .as_mapping_mut()
        .expect("mapping initialized")
        .insert(Value::String(path[path.len() - 1].to_string()), value);
}

fn remove_path(root: &mut Value, path: &[&str]) {
    if path.is_empty() {
        return;
    }
    let Some(parent) = value_at_mut(root, &path[..path.len() - 1]) else {
        return;
    };
    if let Some(map) = parent.as_mapping_mut() {
        map.remove(Value::String(path[path.len() - 1].to_string()));
    }
}

fn value_at_mut<'a>(value: &'a mut Value, path: &[&str]) -> Option<&'a mut Value> {
    let mut current = value;
    for segment in path {
        current = current
            .as_mapping_mut()?
            .get_mut(Value::String((*segment).to_string()))?;
    }
    Some(current)
}

fn map_string(map: &Mapping, key: &str) -> Option<String> {
    map.get(Value::String(key.to_string()))
        .and_then(|value| match value {
            Value::String(value) => Some(value.clone()),
            Value::Number(value) => Some(value.to_string()),
            _ => None,
        })
}

fn map_bool(map: &Mapping, key: &str) -> Option<bool> {
    map.get(Value::String(key.to_string()))
        .and_then(Value::as_bool)
}

fn unique_request_path(root: &Path, slug: &str) -> PathBuf {
    let mut path = root.join(format!("{slug}.yml"));
    let mut suffix = 2;
    while path.exists() {
        path = root.join(format!("{slug}-{suffix}.yml"));
        suffix += 1;
    }
    path
}

fn slugify(name: &str) -> String {
    let slug: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "new-request".to_string()
    } else {
        slug
    }
}

pub fn ensure_collection_root(root: &Path) -> Result<()> {
    if !root.join("opencollection.yml").exists() {
        return Err(anyhow!(
            "{} does not contain opencollection.yml",
            root.display()
        ));
    }
    Ok(())
}

/// Creates `root` as an empty collection if it is not one already.
pub fn init_collection_root(root: &Path, name: &str) -> Result<()> {
    if ensure_collection_root(root).is_ok() {
        return Ok(());
    }

    fs::create_dir_all(root)
        .with_context(|| format!("failed to create {}", root.display()))?;

    let mut value = Value::Mapping(Mapping::new());
    set_string(&mut value, &["info", "name"], name);
    let serialized = serde_yaml::to_string(&value)?;
    let manifest = root.join("opencollection.yml");
    fs::write(&manifest, serialized)
        .with_context(|| format!("failed to write {}", manifest.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::HttpMethod;

    #[test]
    fn parses_request_yaml() {
        let value: Value = serde_yaml::from_str(
            r#"
info:
  name: List users
http:
  method: GET
  url: https://example.com/users
  headers:
    - name: Accept
      value: application/json
      enabled: true
"#,
        )
        .unwrap();

        let draft = draft_from_value(&value, Path::new("users.yml"));
        assert_eq!(draft.name, "List users");
        assert_eq!(draft.method, HttpMethod::Get);
        assert_eq!(draft.headers[0].key, "Accept");
    }

    #[test]
    fn writes_request_without_dropping_unknown_fields() {
        let mut value: Value = serde_yaml::from_str(
            r#"
info:
  name: Old
scripts:
  preRequest: echo keep
"#,
        )
        .unwrap();

        let draft = RequestDraft {
            name: "New".into(),
            method: HttpMethod::Post,
            url: "https://example.com".into(),
            ..RequestDraft::default()
        };

        write_draft_to_value(&mut value, &draft);
        assert_eq!(string_at(&value, &["info", "name"]).unwrap(), "New");
        assert!(value_at(&value, &["scripts", "preRequest"]).is_some());
    }
}
