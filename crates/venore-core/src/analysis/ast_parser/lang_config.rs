//! Data-driven language configuration
//!
//! Each language defines a `NodeMapping` that maps tree-sitter node kinds
//! to `SymbolKind` values. The generic extractor uses these tables.

use super::SymbolKind;

/// Maps tree-sitter node kinds to semantic categories
pub struct NodeMapping {
    /// node-kind → SymbolKind (functions, classes, etc.)
    pub symbols: &'static [(&'static str, SymbolKind)],
    /// Node kinds that represent imports
    pub import_nodes: &'static [&'static str],
    /// Node kinds that represent exports
    pub export_nodes: &'static [&'static str],
    /// Nodes that contain nested symbols (e.g., impl blocks)
    pub container_nodes: &'static [&'static str],
    /// Nodes that may contain arrow functions / variable patterns
    pub variable_nodes: &'static [&'static str],
}

impl NodeMapping {
    /// Look up the SymbolKind for a given node kind
    pub fn symbol_kind_for(&self, node_kind: &str) -> Option<SymbolKind> {
        self.symbols
            .iter()
            .find(|(kind, _)| *kind == node_kind)
            .map(|(_, sk)| sk.clone())
    }
}

// ── TypeScript / JavaScript / TSX ──────────────────────────────────────

pub static TS_JS_MAPPING: NodeMapping = NodeMapping {
    symbols: &[
        ("function_declaration", SymbolKind::Function),
        ("function", SymbolKind::Function),
        ("class_declaration", SymbolKind::Class),
        ("interface_declaration", SymbolKind::Interface),
        ("type_alias_declaration", SymbolKind::Type),
        ("enum_declaration", SymbolKind::Enum),
    ],
    import_nodes: &["import_statement"],
    export_nodes: &["export_statement"],
    container_nodes: &[],
    variable_nodes: &["lexical_declaration", "variable_declaration"],
};

// ── Python ─────────────────────────────────────────────────────────────

pub static PYTHON_MAPPING: NodeMapping = NodeMapping {
    symbols: &[
        ("function_definition", SymbolKind::Function),
        ("class_definition", SymbolKind::Class),
    ],
    import_nodes: &["import_statement", "import_from_statement"],
    export_nodes: &[],
    container_nodes: &[],
    variable_nodes: &[],
};

// ── Rust ───────────────────────────────────────────────────────────────

pub static RUST_MAPPING: NodeMapping = NodeMapping {
    symbols: &[
        ("function_item", SymbolKind::Function),
        ("struct_item", SymbolKind::Struct),
        ("trait_item", SymbolKind::Trait),
        ("enum_item", SymbolKind::Enum),
        ("type_item", SymbolKind::Type),
    ],
    import_nodes: &["use_declaration"],
    export_nodes: &[], // handled via pub visibility detection
    container_nodes: &["impl_item"],
    variable_nodes: &[],
};

// ── Java ───────────────────────────────────────────────────────────────

pub static JAVA_MAPPING: NodeMapping = NodeMapping {
    symbols: &[
        ("class_declaration", SymbolKind::Class),
        ("interface_declaration", SymbolKind::Interface),
        ("enum_declaration", SymbolKind::Enum),
        ("method_declaration", SymbolKind::Method),
    ],
    import_nodes: &["import_declaration"],
    export_nodes: &[],
    container_nodes: &[],
    variable_nodes: &[],
};

// ── Go ─────────────────────────────────────────────────────────────────

pub static GO_MAPPING: NodeMapping = NodeMapping {
    symbols: &[
        ("function_declaration", SymbolKind::Function),
        ("method_declaration", SymbolKind::Method),
        // type_spec handled separately (needs struct vs interface disambiguation)
    ],
    import_nodes: &["import_declaration"],
    export_nodes: &[], // handled via uppercase detection
    container_nodes: &[],
    variable_nodes: &[],
};

// ── C# ─────────────────────────────────────────────────────────────────

pub static CSHARP_MAPPING: NodeMapping = NodeMapping {
    symbols: &[
        ("class_declaration", SymbolKind::Class),
        ("interface_declaration", SymbolKind::Interface),
        ("enum_declaration", SymbolKind::Enum),
        ("struct_declaration", SymbolKind::Struct),
        ("method_declaration", SymbolKind::Method),
    ],
    import_nodes: &["using_directive"],
    export_nodes: &[],
    container_nodes: &[],
    variable_nodes: &[],
};

// ── PHP ────────────────────────────────────────────────────────────────

pub static PHP_MAPPING: NodeMapping = NodeMapping {
    symbols: &[
        ("function_definition", SymbolKind::Function),
        ("class_declaration", SymbolKind::Class),
        ("interface_declaration", SymbolKind::Interface),
        ("trait_declaration", SymbolKind::Trait),
        ("enum_declaration", SymbolKind::Enum),
        ("method_declaration", SymbolKind::Method),
    ],
    import_nodes: &["namespace_use_declaration"],
    export_nodes: &[],
    container_nodes: &[],
    variable_nodes: &[],
};

// ── GDScript ───────────────────────────────────────────────────────────
//
// GDScript imports live inside `call` expressions (`preload(...)` /
// `load(...)`). The extractor in `gdscript.rs` filters non-import
// calls.
//
// Node kind names are tentative — refine after dumping the AST with
// the `debug_dump_gdscript_ast` test if needed.

pub static GDSCRIPT_MAPPING: NodeMapping = NodeMapping {
    symbols: &[
        ("class_definition", SymbolKind::Class),
        ("class_name_statement", SymbolKind::Class),
        ("function_definition", SymbolKind::Function),
    ],
    import_nodes: &["call"],
    export_nodes: &[],
    container_nodes: &[],
    variable_nodes: &[],
};

// ── C ──────────────────────────────────────────────────────────────────
//
// C is structurally simple: types live in `struct_specifier`,
// `union_specifier`, `enum_specifier`, and functions in
// `function_definition`. Imports are `#include` preprocessor
// directives, which the grammar surfaces as `preproc_include`.

pub static C_MAPPING: NodeMapping = NodeMapping {
    symbols: &[
        ("function_definition", SymbolKind::Function),
        ("struct_specifier", SymbolKind::Struct),
        ("union_specifier", SymbolKind::Struct), // surfaced as Struct on the canvas
        ("enum_specifier", SymbolKind::Enum),
        ("type_definition", SymbolKind::Type),
    ],
    import_nodes: &["preproc_include"],
    export_nodes: &[],
    container_nodes: &[],
    variable_nodes: &[],
};

// ── C++ ────────────────────────────────────────────────────────────────
//
// Inherits the C symbol set and adds classes, namespaces, and
// templates. Tree-sitter-cpp reuses preproc_include from the C grammar.

pub static CPP_MAPPING: NodeMapping = NodeMapping {
    symbols: &[
        ("function_definition", SymbolKind::Function),
        ("class_specifier", SymbolKind::Class),
        ("struct_specifier", SymbolKind::Struct),
        ("union_specifier", SymbolKind::Struct),
        ("enum_specifier", SymbolKind::Enum),
        ("type_definition", SymbolKind::Type),
        // Namespaces register as Class on the canvas — they're the
        // closest analogue to a Java/PHP-style module container.
        ("namespace_definition", SymbolKind::Class),
    ],
    import_nodes: &["preproc_include"],
    export_nodes: &[],
    container_nodes: &[],
    variable_nodes: &[],
};

// ── Kotlin ─────────────────────────────────────────────────────────────
//
// `tree-sitter-kotlin-ng` uses a single `class_declaration` node for
// both classes, data classes, sealed classes, enums and interfaces —
// the keyword (`class` vs `interface`) is the only differentiator.
// We therefore classify everything as `Class` here and rely on the
// extractor to refine if needed. `object_declaration` covers Kotlin
// singletons (`object Foo { ... }`), also surfaced as Class for the
// canvas.

pub static KOTLIN_MAPPING: NodeMapping = NodeMapping {
    symbols: &[
        ("class_declaration", SymbolKind::Class),
        ("object_declaration", SymbolKind::Class),
        ("function_declaration", SymbolKind::Function),
    ],
    // `tree-sitter-kotlin-ng` names the top-level import statement
    // `import` (not `import_header` as some older grammars do).
    import_nodes: &["import"],
    export_nodes: &[],
    container_nodes: &[],
    variable_nodes: &[],
};

// ── Ruby ───────────────────────────────────────────────────────────────
//
// Ruby uses `call` nodes for `require` / `require_relative` (they are
// regular method calls in the grammar). The extractor in `ruby.rs`
// filters those calls by the called identifier; non-require calls are
// ignored, so listing `call` in `import_nodes` is the cheapest way to
// route them through the dispatch loop.

pub static RUBY_MAPPING: NodeMapping = NodeMapping {
    symbols: &[
        ("class", SymbolKind::Class),
        ("module", SymbolKind::Class), // Ruby modules look like classes in the canvas; no separate variant today
        ("method", SymbolKind::Method),
        ("singleton_method", SymbolKind::Method),
    ],
    import_nodes: &["call"],
    export_nodes: &[],
    container_nodes: &[],
    variable_nodes: &[],
};
