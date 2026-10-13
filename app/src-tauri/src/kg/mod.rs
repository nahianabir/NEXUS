//! Knowledge graph. Phase D.
//!
//! The graph stores typed entities and relationships, with evidence
//! links back to the source records that produced them. The
//! traversal helpers are bounded (max depth, max nodes) so a query
//! never blows up the front-end.
//!
//! All writes go through this module; the UI does not embed SQL.

pub mod evidence;
pub mod extract;
pub mod graph;
pub mod normalize;
pub mod queries;
pub mod relationship;
pub mod types;

// Re-export the data types and constants so callers can do
// `kg::Entity`, `kg::Relationship`, etc. without reaching into
// `kg::types::*`. Submodule functions are NOT re-exported because
// their names (`get`, `list`, `delete`) would clash.
pub use evidence::CreateEvidence;
pub use graph::{CreateEntity, ListFilter, MergeRequest};
pub use relationship::CreateRelationship;
pub use types::{
    Entity, Evidence, Neighborhood, Relationship, ENTITY_TYPES, RELATIONSHIP_TYPES,
};
pub use queries::{GraphStats, Bounded};
