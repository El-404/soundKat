use mpris::{Metadata, Player, PlayerFinder};


pub struct AudioManager {
    player: Player,
    metadata: Metadata,
}

impl AudioManager {
    pub fn new() -> Result<AudioManager, Box<dyn std::error::Error>> {
        let player = PlayerFinder::new()?
            .find_active()?;

        Ok(AudioManager { 
            player: player,
            metadata: metadata,
        })
    }
}