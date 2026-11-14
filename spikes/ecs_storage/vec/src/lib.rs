#![forbid(unsafe_code)]
//! The plain-`Vec` reference backends of the ECS storage spike (M3.2).
//!
//! Each workload keeps one `Vec` of plain structs and the loop a programmer would write by hand:
//! the best case that every storage prototype's cost is read against.

pub mod frag_iter;
pub mod heavy_compute;
pub mod simple_iter;

#[cfg(test)]
mod tests;
