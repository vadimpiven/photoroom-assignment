// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Operations that transform f32 inputs into a single
//! f32 output. Implement [`Operation`] for custom types,
//! or use [`op`] to create one from a closure.

use std::sync::Arc;

type ApplyFn = Box<dyn Fn(&[f32]) -> f32 + Send + Sync>;

/// An operation that takes one or more f32 inputs and
/// produces a single f32 output.
pub trait Operation: Send + Sync {
    /// Human-readable label shown in debug output.
    fn label(&self) -> &str;
    /// Expected number of inputs (>= 1).
    fn num_inputs(&self) -> usize;
    /// Compute the result from the given inputs.
    fn apply(&self, inputs: &[f32]) -> f32;
}

/// Closure-based operation.
pub struct CustomOp {
    label: String,
    num_inputs: usize,
    apply: ApplyFn,
}

impl CustomOp {
    /// Create an operation from a label, arity, and closure.
    pub fn new(
        label: impl Into<String>,
        num_inputs: usize,
        apply: impl Fn(&[f32]) -> f32 + Send + Sync + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            num_inputs,
            apply: Box::new(apply),
        }
    }
}

impl Operation for CustomOp {
    fn label(&self) -> &str {
        &self.label
    }

    fn num_inputs(&self) -> usize {
        self.num_inputs
    }

    fn apply(&self, inputs: &[f32]) -> f32 {
        (self.apply)(inputs)
    }
}

/// Create an operation from a label, arity, and closure.
#[must_use]
pub fn op(
    label: impl Into<String>,
    num_inputs: usize,
    apply: impl Fn(&[f32]) -> f32 + Send + Sync + 'static,
) -> Arc<dyn Operation> {
    Arc::new(CustomOp::new(label, num_inputs, apply))
}
