//! Deterministic expression and frame work limits shared by both evaluators.
//! These counters are not wall-clock, allocator or GPU-driver watchdogs.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub const MAX_EXPRESSION_BYTES: usize = 64 * 1024;
pub const MAX_EXPRESSION_TOKENS: usize = 16_384;
pub const MAX_EXPRESSION_WORK: usize = 200_000;
pub const MAX_EXPRESSION_DEPTH: usize = 64;
pub const MAX_FRAME_EXPRESSION_WORK: usize = 2_000_000;

#[derive(Debug)]
struct FrameState {
    remaining: Cell<usize>,
    error: RefCell<Option<String>>,
}

thread_local! {
    static ACTIVE_FRAME: RefCell<Option<Rc<FrameState>>> = RefCell::new(None);
}

/// Synchronous, thread-local frame scope. Nested scopes reuse the outer budget;
/// they cannot replenish it. This guard is intentionally not Send or Sync.
pub struct FrameEvaluation {
    state: Rc<FrameState>,
    previous: Option<Rc<FrameState>>,
}

impl FrameEvaluation {
    pub fn begin() -> Self {
        ACTIVE_FRAME.with(|slot| {
            let previous = slot.borrow_mut().take();
            let state = previous.clone().unwrap_or_else(|| Rc::new(FrameState {
                remaining: Cell::new(MAX_FRAME_EXPRESSION_WORK),
                error: RefCell::new(None),
            }));
            *slot.borrow_mut() = Some(state.clone());
            Self { state, previous }
        })
    }

    pub fn error(&self) -> Option<String> {
        self.state.error.borrow().clone()
    }
}

impl Drop for FrameEvaluation {
    fn drop(&mut self) {
        ACTIVE_FRAME.with(|slot| *slot.borrow_mut() = self.previous.take());
    }
}

pub fn record_frame_error(error: &str) {
    ACTIVE_FRAME.with(|slot| {
        if let Some(frame) = slot.borrow().as_ref() {
            let mut recorded = frame.error.borrow_mut();
            if recorded.is_none() { *recorded = Some(error.chars().take(512).collect()); }
        }
    });
}

pub fn check_frame_budget() -> Result<(), String> {
    ACTIVE_FRAME.with(|slot| match slot.borrow().as_ref() {
        Some(frame) => match frame.error.borrow().as_ref() {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        },
        None => Ok(()),
    })
}

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
        check_frame_budget()?;
        let Some(remaining) = self.0.remaining.get().checked_sub(work) else {
            self.0.remaining.set(0);
            record_frame_error("Expression work budget exceeded");
            return Err("Expression work budget exceeded".to_string());
        };
        ACTIVE_FRAME.with(|slot| -> Result<(), String> {
            if let Some(frame) = slot.borrow().as_ref() {
                let Some(remaining) = frame.remaining.get().checked_sub(work) else {
                    *frame.error.borrow_mut() = Some("Frame expression work budget exceeded".to_string());
                    return Err("Frame expression work budget exceeded".to_string());
                };
                frame.remaining.set(remaining);
            }
            Ok(())
        })?;
        self.0.remaining.set(remaining);
        Ok(())
    }

    /// The guard owns its counter so child parsers can share the same budget
    /// without borrowing the parent parser. Drop unwinds depth on every error.
    pub fn enter(&self) -> Result<ExpressionDepthGuard, String> {
        self.spend(1)?;
        let depth = self.0.depth.get();
        if depth >= MAX_EXPRESSION_DEPTH {
            record_frame_error("Expression nesting limit exceeded");
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
