// SPDX-License-Identifier: Apache-2.0 OR MIT

//! DAG-based f32 operation graphs: build, evaluate, display.
//!
//! Build graphs with [`value`] and [`node`], define
//! operations with [`op`] or the [`Operation`] trait,
//! evaluate with [`EvalContext::evaluate`], and inspect
//! with [`debug_tree`].

mod display;
mod eval;
mod node;
mod operation;

pub use display::debug_tree;
pub use eval::EvalContext;
pub use eval::eval;
pub use node::Node;
pub use node::NodeId;
pub use node::NodeKind;
pub use node::node;
pub use node::value;
pub use operation::CustomOp;
pub use operation::Operation;
pub use operation::op;
