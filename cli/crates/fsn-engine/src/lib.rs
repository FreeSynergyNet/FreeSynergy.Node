// fsn-engine – State reconciliation and code generation.
//
// The engine is the heart of FSN: it reads config, computes state,
// generates Quadlet/env files, and enforces constraints.

pub mod constraints;
pub mod deploy;
pub mod diff;
pub mod generate;
pub mod health;
pub mod hooks;
pub mod observe;
pub mod resolve;
pub mod setup;
pub mod template;
