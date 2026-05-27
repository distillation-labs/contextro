//! Code tool: AST operations dispatch.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use crate::analysis::is_test_file;
use contextro_core::graph::UniversalNode;
use contextro_core::traits::Parser;
use contextro_engines::graph::CodeGraph;
use contextro_parsing::TreeSitterParser;
use serde_json::{json, Value};

mod codebase_map;
mod common;
mod document_symbols;
mod edit_plan;
mod pattern;
mod symbols;

use codebase_map::*;
use common::*;
use document_symbols::*;
use edit_plan::*;
use pattern::*;
use symbols::*;

const DEFAULT_DOCUMENT_SYMBOL_LIMIT: usize = 3;

pub fn handle_code(args: &Value, graph: &CodeGraph, codebase: Option<&str>) -> Value {
    // Accept both `operation` (current) and `action` (v0.4.0 name) for backward compat
    let operation = args
        .get("operation")
        .or_else(|| args.get("action"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    match operation {
        "get_document_symbols" => get_document_symbols(args, Some(graph), codebase),
        // v0.4.0 name alias
        "list_symbols" => {
            // If `file_path` or `path` point to a file, use get_document_symbols;
            // otherwise fall through to the directory-based list
            let has_file = get_document_path_arg(args)
                .and_then(|path| resolve_existing_path(path, codebase).ok())
                .map(|path| path.is_file())
                .unwrap_or(false);
            if has_file {
                get_document_symbols(args, Some(graph), codebase)
            } else {
                list_symbols(args, graph, codebase)
            }
        }
        "search_symbols" => search_symbols(args, graph, codebase),
        "lookup_symbols" => lookup_symbols(args, graph, codebase),
        "pattern_search" => pattern_search(args, codebase),
        "pattern_rewrite" => pattern_rewrite(args, codebase),
        "edit_plan" => edit_plan(args, graph, codebase),
        "search_codebase_map" => search_codebase_map(args, graph, codebase),
        _ => {
            json!({"error": format!("Unknown code operation: '{}'. Valid operations: get_document_symbols, search_symbols, lookup_symbols, list_symbols, pattern_search, pattern_rewrite, edit_plan, search_codebase_map", operation)})
        }
    }
}

#[cfg(test)]
mod tests;
