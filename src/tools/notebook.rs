use crate::core::json::JsonValue;
use crate::error::Error;
use crate::tools::{PermissionLevel, Tool};
use std::future::Future;
use std::pin::Pin;

pub struct NotebookEditTool;

impl Tool for NotebookEditTool {
    fn name(&self) -> &str {
        "NotebookEdit"
    }

    fn description(&self) -> &str {
        "Edit Jupyter notebook (.ipynb) cells. Supports replacing, inserting, and deleting cells by cell ID."
    }

    fn input_schema(&self) -> JsonValue {
        crate::tools::parse_schema(
            r#"{
            "type":"object",
            "properties":{
                "notebook_path":{"type":"string","description":"Absolute path to the .ipynb file"},
                "cell_id":{"type":"string","description":"The cell's id field for locating the target cell"},
                "cell_type":{"type":"string","enum":["code","markdown"],"description":"Cell type, required for insert mode"},
                "edit_mode":{"type":"string","enum":["replace","insert","delete"],"description":"Edit mode: replace (default), insert, or delete"},
                "new_source":{"type":"string","description":"New cell content (ignored for delete)"}
            },
            "required":["notebook_path","new_source"]
        }"#,
        )
    }

    fn execute(
        &self,
        input: &JsonValue,
    ) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        let input = input.clone();
        Box::pin(async move {
            let notebook_path = input
                .get("notebook_path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Tool("missing 'notebook_path'".into()))?;
            let new_source = input
                .get("new_source")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let cell_id = input.get("cell_id").and_then(|v| v.as_str());
            let cell_type = input.get("cell_type").and_then(|v| v.as_str());
            let edit_mode = input
                .get("edit_mode")
                .and_then(|v| v.as_str())
                .unwrap_or("replace");

            // Read and parse the notebook
            let content = std::fs::read_to_string(notebook_path)
                .map_err(|e| Error::Tool(format!("failed to read '{}': {}", notebook_path, e)))?;
            let notebook = JsonValue::parse(&content)
                .map_err(|e| Error::Tool(format!("failed to parse notebook JSON: {}", e)))?;

            // Extract cells array
            let cells = match notebook.get("cells") {
                Some(JsonValue::Array(arr)) => arr.clone(),
                _ => return Err(Error::Tool("notebook has no 'cells' array".into())),
            };

            // Build the source array: split by \n, each line ends with \n except possibly the last
            let source_array = source_to_json_array(new_source);

            let new_cells = match edit_mode {
                "replace" => {
                    let cell_id = cell_id
                        .ok_or_else(|| Error::Tool("'cell_id' is required for replace".into()))?;
                    let idx = find_cell_index(&cells, cell_id)?;
                    let mut new_cells = cells;
                    new_cells[idx] = replace_cell_source(&new_cells[idx], &source_array)?;
                    new_cells
                }
                "insert" => {
                    let ct = cell_type.ok_or_else(|| {
                        Error::Tool("'cell_type' is required for insert mode".into())
                    })?;
                    if ct != "code" && ct != "markdown" {
                        return Err(Error::Tool(format!("invalid cell_type: '{}'", ct)));
                    }
                    let new_cell = make_new_cell(ct, &source_array);
                    let mut new_cells = cells;
                    match cell_id {
                        Some(id) => {
                            let idx = find_cell_index(&new_cells, id)?;
                            new_cells.insert(idx + 1, new_cell);
                        }
                        None => {
                            // Insert at the beginning if no cell_id
                            new_cells.insert(0, new_cell);
                        }
                    }
                    new_cells
                }
                "delete" => {
                    let cell_id = cell_id
                        .ok_or_else(|| Error::Tool("'cell_id' is required for delete".into()))?;
                    let idx = find_cell_index(&cells, cell_id)?;
                    let mut new_cells = cells;
                    new_cells.remove(idx);
                    new_cells
                }
                _ => return Err(Error::Tool(format!("invalid edit_mode: '{}'", edit_mode))),
            };

            // Reconstruct the notebook with updated cells
            let new_notebook = rebuild_notebook(&notebook, new_cells);

            // Write back
            let output = format!("{}", new_notebook);
            std::fs::write(notebook_path, &output)
                .map_err(|e| Error::Tool(format!("failed to write '{}': {}", notebook_path, e)))?;

            let msg = match edit_mode {
                "replace" => format!(
                    "Replaced cell '{}' in {}",
                    cell_id.unwrap_or(""),
                    notebook_path
                ),
                "insert" => format!("Inserted new cell in {}", notebook_path),
                "delete" => format!(
                    "Deleted cell '{}' from {}",
                    cell_id.unwrap_or(""),
                    notebook_path
                ),
                _ => "Done".to_string(),
            };
            Ok(msg)
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Write
    }
}

/// Convert a source string into a JSON array of lines (ipynb format).
/// Each line ends with `\n` except possibly the last.
fn source_to_json_array(source: &str) -> JsonValue {
    if source.is_empty() {
        return JsonValue::Array(vec![]);
    }
    let lines: Vec<&str> = source.split('\n').collect();
    let mut arr = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        if i < lines.len() - 1 {
            arr.push(JsonValue::Str(format!("{}\n", line)));
        } else {
            // Last line: only add \n if the original string ended with \n
            if source.ends_with('\n') {
                arr.push(JsonValue::Str(format!("{}\n", line)));
            } else {
                arr.push(JsonValue::Str(line.to_string()));
            }
        }
    }
    // If the source ended with \n, the split produces an empty trailing element
    // like ["line\n", "\n"]. Remove the extra empty-string trailing element.
    if source.ends_with('\n') {
        if let Some(last) = arr.last() {
            if *last == JsonValue::Str("\n".to_string()) {
                // keep it
            } else if *last == JsonValue::Str("".to_string()) {
                arr.pop();
            }
        }
    }
    JsonValue::Array(arr)
}

/// Find the index of a cell with the given id.
fn find_cell_index(cells: &[JsonValue], cell_id: &str) -> crate::Result<usize> {
    for (i, cell) in cells.iter().enumerate() {
        if let Some(id_val) = cell.get("id") {
            if id_val.as_str() == Some(cell_id) {
                return Ok(i);
            }
        }
    }
    Err(Error::Tool(format!("cell with id '{}' not found", cell_id)))
}

/// Replace the source of a cell, returning a new cell JsonValue.
fn replace_cell_source(cell: &JsonValue, new_source: &JsonValue) -> crate::Result<JsonValue> {
    if let JsonValue::Object(pairs) = cell {
        let new_pairs: Vec<(String, JsonValue)> = pairs
            .iter()
            .map(|(k, v)| {
                if k == "source" {
                    (k.clone(), new_source.clone())
                } else {
                    (k.clone(), v.clone())
                }
            })
            .collect();
        Ok(JsonValue::Object(new_pairs))
    } else {
        Err(Error::Tool("cell is not a JSON object".into()))
    }
}

/// Create a new cell object.
fn make_new_cell(cell_type: &str, source: &JsonValue) -> JsonValue {
    let mut pairs = vec![
        (
            "cell_type".to_string(),
            JsonValue::Str(cell_type.to_string()),
        ),
        ("source".to_string(), source.clone()),
        ("metadata".to_string(), JsonValue::Object(vec![])),
    ];
    if cell_type == "code" {
        pairs.push(("outputs".to_string(), JsonValue::Array(vec![])));
    }
    JsonValue::Object(pairs)
}

/// Rebuild the notebook JSON with new cells, preserving all other fields.
fn rebuild_notebook(notebook: &JsonValue, new_cells: Vec<JsonValue>) -> JsonValue {
    if let JsonValue::Object(pairs) = notebook {
        let new_pairs: Vec<(String, JsonValue)> = pairs
            .iter()
            .map(|(k, v)| {
                if k == "cells" {
                    (k.clone(), JsonValue::Array(new_cells.clone()))
                } else {
                    (k.clone(), v.clone())
                }
            })
            .collect();
        JsonValue::Object(new_pairs)
    } else {
        notebook.clone()
    }
}
