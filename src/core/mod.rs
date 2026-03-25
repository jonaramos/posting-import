//! Core domain models for the Posting importer.
//!
//! This module defines the intermediate representation (IR) that all
//! importer plugins convert their source formats into. This IR is then
//! used by the Posting format writer to generate `.posting.yaml` files.

pub mod models;

