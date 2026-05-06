use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Workspace {
    pub id: serde_json::Value,
    pub name: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

impl Workspace {
    pub fn id_str(&self) -> String {
        value_to_id(&self.id)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Board {
    pub id: serde_json::Value,
    pub name: String,
    #[serde(default)]
    pub columns: Vec<Column>,
    #[serde(default)]
    pub groups: Vec<Group>,
    #[serde(default)]
    pub items_count: Option<i64>,
    #[serde(default)]
    pub workspace: Option<BoardWorkspace>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BoardWorkspace {
    #[serde(default)]
    pub id: Option<serde_json::Value>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
}

impl BoardWorkspace {
    pub fn id_str(&self) -> Option<String> {
        self.id.as_ref().map(|v| match v {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            _ => v.to_string(),
        })
    }
}

impl Board {
    pub fn id_str(&self) -> String {
        value_to_id(&self.id)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Group {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Column {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub col_type: String,
    #[serde(default)]
    pub settings_str: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Item {
    pub id: serde_json::Value,
    pub name: String,
    #[serde(default)]
    pub column_values: Vec<ColumnValue>,
    #[serde(default)]
    pub group: Option<ItemGroupRef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ItemGroupRef {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
}

impl Item {
    pub fn id_str(&self) -> String {
        value_to_id(&self.id)
    }

    pub fn value_for(&self, col_id: &str) -> Option<&ColumnValue> {
        self.column_values.iter().find(|c| c.id == col_id)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ColumnValue {
    pub id: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(rename = "type", default)]
    pub col_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ItemsPage {
    #[serde(default)]
    pub cursor: Option<String>,
    pub items: Vec<Item>,
}

fn value_to_id(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => v.to_string(),
    }
}

/// Parses settings_str from a status column to extract index -> label map
pub fn parse_status_labels(settings_str: &str) -> Vec<(i64, String)> {
    let parsed: serde_json::Value = match serde_json::from_str(settings_str) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let labels = match parsed.get("labels") {
        Some(serde_json::Value::Object(map)) => map,
        _ => return Vec::new(),
    };
    let mut out: Vec<(i64, String)> = labels
        .iter()
        .filter_map(|(k, v)| {
            let idx: i64 = k.parse().ok()?;
            let label = v.as_str()?.to_string();
            if label.is_empty() {
                None
            } else {
                Some((idx, label))
            }
        })
        .collect();
    out.sort_by_key(|(i, _)| *i);
    out
}
