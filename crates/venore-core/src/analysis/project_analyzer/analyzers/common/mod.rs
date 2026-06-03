//! Helpers shared across multiple project analyzers.
//!
//! Each submodule groups helpers by ecosystem (node, python, rust, …)
//! so that adding a new analyzer for the same ecosystem reuses the
//! detection bits instead of duplicating them.

pub mod ccpp;
pub mod dotnet;
pub mod kotlin;
pub mod node;
pub mod php;
pub mod python;
pub mod ruby;
pub mod rust;
