//! Deterministic per-expression limits shared by both evaluator implementations.
//! These are work limits, not a wall-clock or whole-render-frame watchdog.

use std::cell::Cell;
use std::rc::Rc;

pub const MAX_EXPRESSION_BYTES: usize = 64 * 1024;
pub const MAX_EXPRESSION_TOKENS: usize = 16_384;
pub const MAX_EXPRESSION_WORK: usize = 200_000;
pub const MAX_EXPRESSION_DEPTH: usize = 64;

pub fn validate_expression_source(source: &str) -> Result<(), String> {
    if source.len() > MAX_EXPRESSION_BYTES || source.as_bytes().contains(&0) {
        return Err("Expression is binary or exceeds the 64 KiB limit".to_string());
    }
    Ok(())
}

#[derive(Debug)]
struct BudgetState {
    remaining: Cell<usize>,
    depth: Cell<usize>,
}

#[derive(Clone, Debug)]
pub struct ExpressionBudget(Rc<BudgetState>);

impl Default for ExpressionBudget {
    fn default() -> Self {
        Self(Rc::new(BudgetState {
            remaining: Cell::new(MAX_EXPRESSION_WORK),
            depth: Cell::new(0),
        }))
    }
}

impl ExpressionBudget {
    pub fn spend(&self, work: usize) -> Result<(), String> {
        let Some(remaining) = self.0.remaining.get().checked_sub(work) else {
            self.0.remaining.set(0);
            return Err("Expression work budget exceeded".to_string());
        };
        self.0.remaining.set(remaining);
        Ok(())
    }

    /// The guard owns its counter so child parsers can share the same budget
    /// without borrowing the parent parser. Drop unwinds depth on every error.
    pub fn enter(&self) -> Result<ExpressionDepthGuard, String> {
        self.spend(1)?;
        let depth = self.0.depth.get();
        if depth >= MAX_EXPRESSION_DEPTH {
            return Err("Expression nesting limit exceeded".to_string());
        }
        self.0.depth.set(depth + 1);
        Ok(ExpressionDepthGuard(self.0.clone()))
    }
}

pub struct ExpressionDepthGuard(Rc<BudgetState>);

impl Drop for ExpressionDepthGuard {
    fn drop(&mut self) {
        self.0.depth.set(self.0.depth.get().saturating_sub(1));
    }
}
