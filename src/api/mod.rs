pub mod types;

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

pub use types::*;

const ENDPOINT: &str = "https://api.monday.com/v2";
const API_VERSION: &str = "2024-01";
pub const MAIN_WORKSPACE_ID: &str = "__main__";

#[derive(Clone)]
pub struct MondayClient {
    http: Client,
    token: String,
}

#[derive(Debug, Deserialize)]
struct GqlResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GqlError>>,
}

#[derive(Debug, Deserialize)]
struct GqlError {
    message: String,
}

impl MondayClient {
    pub fn new(token: impl Into<String>) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("build http client")?;
        Ok(Self {
            http,
            token: token.into(),
        })
    }

    async fn execute<T: for<'de> Deserialize<'de>>(
        &self,
        query: &str,
        variables: Value,
    ) -> Result<T> {
        let body = json!({ "query": query, "variables": variables });
        let resp = self
            .http
            .post(ENDPOINT)
            .header("Authorization", &self.token)
            .header("API-Version", API_VERSION)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("send request")?;
        let status = resp.status();
        let text = resp.text().await.context("read response body")?;
        if !status.is_success() {
            return Err(anyhow!("HTTP {}: {}", status, text));
        }
        let parsed: GqlResponse<T> =
            serde_json::from_str(&text).with_context(|| format!("parse response: {}", text))?;
        if let Some(errs) = parsed.errors {
            let msg = errs
                .into_iter()
                .map(|e| e.message)
                .collect::<Vec<_>>()
                .join("; ");
            return Err(anyhow!("API error: {}", msg));
        }
        parsed.data.ok_or_else(|| anyhow!("no data in response"))
    }

    /// Holt alle Boards, auf die der User Zugriff hat — inkl. Workspace-Info.
    /// Daraus leiten wir die Liste relevanter Workspaces ab (nur die, in denen
    /// tatsächlich Boards für den User existieren).
    pub async fn fetch_boards_overview(&self) -> Result<Vec<Board>> {
        #[derive(Deserialize)]
        struct R {
            boards: Option<Vec<Board>>,
        }
        let mut all = Vec::new();
        let mut page = 1;
        loop {
            let q = r#"
            query($page: Int!) {
              boards(limit: 200, page: $page, state: active, order_by: used_at) {
                id name items_count
                workspace { id name kind }
                columns { id title type settings_str }
                groups { id title color }
              }
            }"#;
            let r: R = self.execute(q, json!({ "page": page })).await?;
            let chunk = r.boards.unwrap_or_default();
            let len = chunk.len();
            all.extend(chunk);
            if len < 200 {
                break;
            }
            page += 1;
            if page > 10 {
                break; // Safety-Cap: 2000 Boards
            }
        }
        Ok(all)
    }

    pub fn derive_workspaces(boards: &[Board]) -> Vec<Workspace> {
        use std::collections::BTreeMap;
        let mut seen: BTreeMap<String, Workspace> = BTreeMap::new();
        let mut has_main = false;
        for b in boards {
            match &b.workspace {
                Some(ws) if ws.id_str().is_some() => {
                    let id_str = ws.id_str().unwrap();
                    seen.entry(id_str.clone()).or_insert_with(|| Workspace {
                        id: ws.id.clone().unwrap_or(serde_json::Value::String(id_str.clone())),
                        name: ws.name.clone().unwrap_or_else(|| "Unknown".into()),
                        kind: ws.kind.clone(),
                        description: None,
                    });
                }
                _ => has_main = true,
            }
        }
        let mut list: Vec<Workspace> = seen.into_values().collect();
        list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        if has_main {
            list.insert(
                0,
                Workspace {
                    id: serde_json::Value::String(MAIN_WORKSPACE_ID.to_string()),
                    name: "Main Workspace".to_string(),
                    kind: Some("main".to_string()),
                    description: Some("Personal / default workspace".to_string()),
                },
            );
        }
        list
    }

    pub fn filter_boards_for_workspace(boards: &[Board], workspace_id: &str) -> Vec<Board> {
        boards
            .iter()
            .filter(|b| match &b.workspace {
                Some(ws) => match ws.id_str() {
                    Some(id) => id == workspace_id,
                    None => workspace_id == MAIN_WORKSPACE_ID,
                },
                None => workspace_id == MAIN_WORKSPACE_ID,
            })
            .cloned()
            .collect()
    }

    pub async fn list_items(&self, board_id: &str) -> Result<Vec<Item>> {
        #[derive(Deserialize)]
        struct R {
            boards: Option<Vec<BoardItems>>,
        }
        #[derive(Deserialize)]
        struct BoardItems {
            items_page: Option<ItemsPage>,
        }
        let q = r#"
        query($boardId: [ID!]) {
          boards(ids: $boardId) {
            items_page(limit: 200) {
              cursor
              items {
                id name
                group { id title }
                column_values { id text value type }
              }
            }
          }
        }"#;
        let vars = json!({ "boardId": [board_id] });
        let r: R = self.execute(q, vars).await?;
        let mut items = Vec::new();
        if let Some(boards) = r.boards {
            for b in boards {
                if let Some(page) = b.items_page {
                    items.extend(page.items);
                }
            }
        }
        Ok(items)
    }

    pub async fn change_status(
        &self,
        board_id: &str,
        item_id: &str,
        column_id: &str,
        index: i64,
    ) -> Result<()> {
        let q = r#"
        mutation($boardId: ID!, $itemId: ID!, $colId: String!, $value: JSON!) {
          change_column_value(board_id: $boardId, item_id: $itemId,
                              column_id: $colId, value: $value) { id }
        }"#;
        let value_str = json!({ "index": index }).to_string();
        let vars = json!({
            "boardId": board_id,
            "itemId": item_id,
            "colId": column_id,
            "value": value_str,
        });
        let _r: Value = self.execute(q, vars).await?;
        Ok(())
    }

    pub async fn change_simple(
        &self,
        board_id: &str,
        item_id: &str,
        column_id: &str,
        value: &str,
    ) -> Result<()> {
        let q = r#"
        mutation($boardId: ID!, $itemId: ID!, $colId: String!, $value: String!) {
          change_simple_column_value(board_id: $boardId, item_id: $itemId,
                                     column_id: $colId, value: $value) { id }
        }"#;
        let vars = json!({
            "boardId": board_id,
            "itemId": item_id,
            "colId": column_id,
            "value": value,
        });
        let _r: Value = self.execute(q, vars).await?;
        Ok(())
    }

    pub async fn create_item(&self, board_id: &str, name: &str) -> Result<Item> {
        self.create_item_in_group(board_id, None, name).await
    }

    pub async fn create_item_in_group(
        &self,
        board_id: &str,
        group_id: Option<&str>,
        name: &str,
    ) -> Result<Item> {
        #[derive(Deserialize)]
        struct R {
            create_item: Item,
        }
        let q = r#"
        mutation($boardId: ID!, $groupId: String, $name: String!) {
          create_item(board_id: $boardId, group_id: $groupId, item_name: $name) {
            id name
            group { id title }
            column_values { id text value type }
          }
        }"#;
        let vars = json!({
            "boardId": board_id,
            "groupId": group_id,
            "name": name,
        });
        let r: R = self.execute(q, vars).await?;
        Ok(r.create_item)
    }

    pub async fn delete_item(&self, item_id: &str) -> Result<()> {
        let q = r#"
        mutation($itemId: ID!) {
          delete_item(item_id: $itemId) { id }
        }"#;
        let vars = json!({ "itemId": item_id });
        let _r: Value = self.execute(q, vars).await?;
        Ok(())
    }
}
