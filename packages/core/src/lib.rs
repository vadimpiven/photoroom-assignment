// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Core library for DAG-based f32 operations.

mod display;
mod eval;
mod node;

pub use display::debug_tree;
pub use eval::EvalContext;
pub use eval::eval;
pub use node::CustomOp;
pub use node::Node;
pub use node::NodeId;
pub use node::NodeKind;
pub use node::Operation;
pub use node::node;
pub use node::op;
pub use node::value;
