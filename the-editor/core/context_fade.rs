use ropey::RopeSlice;

use crate::core::{
  selection::{Range, Selection},
  syntax::Syntax,
};

/// Maximum depth to trace dependencies to prevent infinite recursion
const DEFAULT_DEPTH_LIMIT: usize = 5;

/// Represents all the ranges of code that are relevant to the current context
#[derive(Debug, Clone, Default)]
pub struct RelevantRanges {
  /// Character ranges that should remain visible (not faded)
  pub ranges: Vec<Range>,
}

impl RelevantRanges {
  pub fn new() -> Self {
    Self { ranges: Vec::new() }
  }

  /// Check if a character position is within any relevant range
  pub fn contains(&self, pos: usize) -> bool {
    self.ranges.iter().any(|range| range.contains(pos))
  }

  /// Add a range, merging with existing overlapping ranges
  pub fn add_range(&mut self, range: Range) {
    self.ranges.push(range);
    self.merge_overlapping();
  }

  /// Merge overlapping ranges to optimize lookup
  fn merge_overlapping(&mut self) {
    if self.ranges.len() < 2 {
      return;
    }

    self.ranges.sort_by_key(|r| r.from());

    let mut merged = Vec::new();
    let mut current = self.ranges[0];

    for range in &self.ranges[1..] {
      if current.overlaps(range) || current.to() == range.from() {
        current = current.extend(range.from(), range.to());
      } else {
        merged.push(current);
        current = *range;
      }
    }
    merged.push(current);

    self.ranges = merged;
  }
}

/// Analyzes code context to determine which ranges are relevant
#[derive(Debug, Clone)]
pub struct ContextAnalyzer {
  pub depth_limit: usize,
}

impl Default for ContextAnalyzer {
  fn default() -> Self {
    Self {
      depth_limit: DEFAULT_DEPTH_LIMIT,
    }
  }
}

impl ContextAnalyzer {
  pub fn new(depth_limit: usize) -> Self {
    Self { depth_limit }
  }

  /// Main entry point: compute all relevant ranges for the current selection
  pub fn compute_relevant_ranges(
    &self,
    text: RopeSlice,
    selection: &Selection,
    syntax: &Syntax,
  ) -> RelevantRanges {
    let mut relevant = RelevantRanges::new();

    // Get the tree for the current document
    let tree = syntax.tree();

    // For each cursor position, find relevant code
    for range in selection.ranges() {
      let cursor_pos = range.cursor(text);
      let byte_pos = text.char_to_byte(cursor_pos) as u32;

      // Find the smallest node containing the cursor (should be an identifier)
      if let Some((start_byte, end_byte)) = Self::find_node_at_position(tree.root_node(), byte_pos)
      {
        // Convert bytes to char positions
        let start_char = text.byte_to_char(start_byte as usize);
        let end_char = text.byte_to_char(end_byte as usize);

        // Extract the identifier text
        let identifier = text.slice(start_char..end_char).to_string();

        // Find all statements/expressions that use this identifier
        self.find_usage_contexts(text, &identifier, tree.root_node(), &mut relevant);
      }
    }

    relevant
  }

  /// Find the smallest node containing the given byte position
  /// Returns the byte range of the node
  fn find_node_at_position(
    node: tree_house::tree_sitter::Node,
    byte_pos: u32,
  ) -> Option<(u32, u32)> {
    // Check if this node contains the position
    if node.start_byte() > byte_pos || node.end_byte() <= byte_pos {
      return None;
    }

    // Try to find a smaller child containing the position
    let mut smallest = Some((node.start_byte(), node.end_byte()));

    for child in node.children() {
      if let Some(child_range) = Self::find_node_at_position(child, byte_pos) {
        smallest = Some(child_range);
        break; // Found a child, use it
      }
    }

    smallest
  }

  /// Find all usage contexts of an identifier (statements/expressions
  /// containing it)
  fn find_usage_contexts(
    &self,
    text: RopeSlice,
    identifier: &str,
    root: tree_house::tree_sitter::Node,
    relevant: &mut RelevantRanges,
  ) {
    eprintln!("[FADE DEBUG] Searching for identifier: '{}'", identifier);
    // Recursively find all usage contexts
    Self::collect_usage_contexts(root, identifier, text, relevant);
    eprintln!(
      "[FADE DEBUG] Found {} relevant ranges",
      relevant.ranges.len()
    );
    for (i, range) in relevant.ranges.iter().enumerate() {
      eprintln!(
        "[FADE DEBUG]   Range {}: {}..{}",
        i,
        range.from(),
        range.to()
      );
    }
  }

  /// Recursively collect usage contexts (statements/expressions) containing the
  /// identifier
  fn collect_usage_contexts(
    node: tree_house::tree_sitter::Node,
    identifier: &str,
    text: RopeSlice,
    relevant: &mut RelevantRanges,
  ) {
    // Collect children first before we potentially move node
    let children: Vec<_> = node.children().collect();

    // Check if this node is an identifier matching our target
    let is_match = if node.kind() == "identifier" {
      let start = text.byte_to_char(node.start_byte() as usize);
      let end = text.byte_to_char(node.end_byte() as usize);
      let node_text = text.slice(start..end).to_string();
      node_text == identifier
    } else {
      false
    };

    if is_match {
      eprintln!(
        "[FADE DEBUG] Found identifier match at {}..{}",
        node.start_byte(),
        node.end_byte()
      );
      // Found a match, find its containing context
      if let Some(context_node) = Self::find_containing_context(node) {
        let start_char = text.byte_to_char(context_node.start_byte() as usize);
        let end_char = text.byte_to_char(context_node.end_byte() as usize);
        eprintln!(
          "[FADE DEBUG]   Context node kind: {}, range: {}..{}",
          context_node.kind(),
          start_char,
          end_char
        );
        relevant.add_range(Range::new(start_char, end_char));
      } else {
        eprintln!("[FADE DEBUG]   No context found!");
      }
    }

    // Recurse into children
    for child in children {
      Self::collect_usage_contexts(child, identifier, text, relevant);
    }
  }

  /// Find the containing statement or expression context for a node
  /// This walks up the tree to find meaningful syntactic boundaries
  fn find_containing_context(
    node: tree_house::tree_sitter::Node,
  ) -> Option<tree_house::tree_sitter::Node> {
    // Statement/expression kinds that represent meaningful contexts
    // These are actual Rust tree-sitter node types
    // We exclude overly broad items like function_item to avoid highlighting entire
    // functions
    const CONTEXT_KINDS: &[&str] = &[
      // Declarations and statements (statement-level, not function-level)
      "let_declaration",
      "expression_statement",
      // Control flow expressions
      "if_expression",
      "for_expression",
      "while_expression",
      "loop_expression",
      "match_expression",
      // Other expressions that form natural boundaries
      "return_expression",
      "assignment_expression",
      "call_expression",
      "macro_invocation",
      // Parameter declarations
      "parameter",
      "parameters",
    ];

    let mut current = node;

    // Walk up the tree looking for a context node
    loop {
      if let Some(parent) = current.parent() {
        let kind = parent.kind();

        // Check if parent is a context kind
        if CONTEXT_KINDS.contains(&kind) {
          return Some(parent);
        }

        current = parent;
      } else {
        // Reached root, return current node
        return Some(current);
      }
    }
  }

  /// Simple identifier extraction for a node
  pub fn extract_identifiers_in_range(
    &self,
    text: RopeSlice,
    start: usize,
    end: usize,
  ) -> Vec<String> {
    let mut identifiers = Vec::new();
    let slice = text.slice(start..end);
    let text_str = slice.to_string();

    // Simple regex-based identifier extraction
    // This is a fallback when we can't use tree-sitter's AST
    let re = regex::Regex::new(r"\b[a-zA-Z_][a-zA-Z0-9_]*\b").unwrap();
    for mat in re.find_iter(&text_str) {
      let ident = mat.as_str().to_string();
      if !identifiers.contains(&ident) {
        identifiers.push(ident);
      }
    }

    identifiers
  }
}

#[cfg(test)]
mod tests {
  use ropey::Rope;

  use super::*;

  #[test]
  fn test_relevant_ranges_contains() {
    let mut ranges = RelevantRanges::new();
    ranges.add_range(Range::new(10, 20));
    ranges.add_range(Range::new(30, 40));

    assert!(ranges.contains(15));
    assert!(ranges.contains(35));
    assert!(!ranges.contains(5));
    assert!(!ranges.contains(25));
  }

  #[test]
  fn test_relevant_ranges_merge() {
    let mut ranges = RelevantRanges::new();
    ranges.add_range(Range::new(10, 20));
    ranges.add_range(Range::new(15, 25)); // Overlapping
    ranges.add_range(Range::new(25, 30)); // Adjacent
    ranges.add_range(Range::new(40, 50)); // Separate

    assert_eq!(ranges.ranges.len(), 2);
    assert_eq!(ranges.ranges[0].from(), 10);
    assert_eq!(ranges.ranges[0].to(), 30);
    assert_eq!(ranges.ranges[1].from(), 40);
    assert_eq!(ranges.ranges[1].to(), 50);
  }

  #[test]
  fn test_extract_identifiers() {
    let analyzer = ContextAnalyzer::default();
    let text = Rope::from("let foo = bar + baz;");
    let identifiers = analyzer.extract_identifiers_in_range(text.slice(..), 0, text.len_chars());

    assert!(identifiers.contains(&"foo".to_string()));
    assert!(identifiers.contains(&"bar".to_string()));
    assert!(identifiers.contains(&"baz".to_string()));
    assert!(identifiers.contains(&"let".to_string())); // This is technically an identifier too
  }
}
