//! `git-dragon`: an evolving pixel-art dragon for your GitHub profile README.
//!
//! The pipeline is `collect` (GitHub data) -> [`history::AccountSnapshot`] -> [`stage`] and
//! [`coat`] (what the dragon looks like) -> [`render`] (frames) -> [`output`] (GIF/PNG/HTML).

pub mod coat;
pub mod collect;
pub mod effects;
pub mod history;
pub mod languages;
pub mod output;
pub mod pipeline;
pub mod render;
pub mod sprite;
pub mod stage;
