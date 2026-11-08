//! Non-human creatures: their airways, sources, playable parameters and calls.
pub mod accordion;
pub mod alligator;
pub mod bellows;
pub mod chorister;
pub mod build;
pub mod elephant;
pub mod glasssinger;
pub mod language;
pub mod mammal;
pub mod resonant;
pub mod seal;
pub mod skywhale;
pub mod songbird;
pub mod stalker;
pub mod swarm;
pub mod types;
pub mod ventboiler;
pub mod voice;
pub mod whistler;

pub use types::{CreatureDef, CreatureSim};
pub use voice::{CreatureTelemetry, CreatureVoice};

use std::sync::LazyLock;

/// Creature registry (built once; [`init`] forces it).
pub static CREATURES: LazyLock<Vec<CreatureDef>> = LazyLock::new(|| vec![songbird::def(), seal::def(), alligator::def(), elephant::def(), resonant::def(), bellows::def(), chorister::def(), skywhale::def(), glasssinger::def(), swarm::def(), ventboiler::def(), whistler::def(), accordion::def(), stalker::def()]);

pub fn creature_by_id(id: &str) -> Option<&'static CreatureDef> {
    CREATURES.iter().find(|c| c.id == id)
}

/// Build the registry and the tables the creatures use now, so nothing initialises lazily on the
/// audio thread.
pub fn init() {
    LazyLock::force(&CREATURES);
    crate::sources::syrinx::init();
}
