//! Datum Plan core: everything except the window. No UI or OS code lives here.
pub mod cal;
pub mod doc;
pub mod formula;
pub mod io;
pub mod level;
pub mod load;
pub mod model;
pub mod parse;
pub mod sched;
pub mod templates;
pub mod view;

pub use model::*;

#[cfg(test)]
mod tests;
