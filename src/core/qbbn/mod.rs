pub mod bp;
pub mod exact;
pub mod factor_graph;
pub mod inference;
pub mod kb;

pub use bp::{belief_propagation, BPTrace};
pub use exact::{
    exact_inference, ExactAssignment, ExactConfig, ExactError, ExactResult,
};
pub use factor_graph::{Factor, FactorType, NodeType, QBBNGraph, Rule, Variable};
pub use inference::{
    run_inference_fixture, InferenceFixture, InferenceResult, QueryResult,
};
pub use kb::{HornClause, KnowledgeBase};