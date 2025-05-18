use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::prelude::*;
use crate::lcu::LcuClient;

/// Represents the current phase of the game client
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum GamePhase {
    None,
    Lobby,
    Matchmaking,
    ReadyCheck,
    ChampSelect,
    InProgress,
    WaitingForStats,
    PreEndOfGame,
    EndOfGame,
}

impl Default for GamePhase {
    fn default() -> Self {
        Self::None
    }
}

/// Settings for the auto accept client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSettings {
    pub auto_accept_enabled: bool,
    pub cancel_queue_after_dodge: bool,
    pub auto_restart_queue: bool,
    pub queue_max_time: u64,  // in milliseconds
    pub current_champion: Option<i32>,
    pub current_ban: Option<i32>,
    pub summoner_spell1: Option<i32>,
    pub summoner_spell2: Option<i32>,
    pub chat_messages_enabled: bool,
    pub chat_messages: Vec<String>,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            auto_accept_enabled: false,
            cancel_queue_after_dodge: false,
            auto_restart_queue: false,
            queue_max_time: 360000, // 6 minutes
            current_champion: None,
            current_ban: None,
            summoner_spell1: None,
            summoner_spell2: None,
            chat_messages_enabled: false,
            chat_messages: Vec::new(),
        }
    }
}

/// State tracking for champion select
#[derive(Debug, Default)]
struct ChampSelectState {
    picked_champion: bool,
    locked_champion: bool,
    picked_ban: bool,
    locked_ban: bool,
    picked_spell1: bool,
    picked_spell2: bool,
    sent_chat_messages: bool,
    last_chat_room: String,
    champ_select_start: u64,
    queue_start_time: u64,
    last_phase: GamePhase,
}

/// High-level client for League Auto Accept functionality
pub struct GameClient {
    pub lcu: LcuClient,
    settings: GameSettings,
    state: ChampSelectState,
}

impl GameClient {
    /// Creates a new GameClient instance
    pub async fn new() -> Result<Self> {
        Ok(Self {
            lcu: LcuClient::new().await?,
            settings: GameSettings::default(),
            state: ChampSelectState::default(),
        })
    }

    /// Updates the client settings
    pub fn update_settings(&mut self, settings: GameSettings) {
        self.settings = settings;
    }

    /// Gets the current game phase
    pub async fn get_game_phase(&self) -> Result<GamePhase> {
        let (status, response) = self.lcu.request("GET", "/lol-gameflow/v1/gameflow-phase", None).await?;
        
        if status != 200 {
            return Ok(GamePhase::None);
        }

        serde_json::from_str(&response)
            .map_err(|e| Error::Other(format!("Failed to parse game phase: {}", e)))
    }

    /// Main loop for handling auto accept and champion select
    pub async fn run(&mut self) -> Result<()> {
        while self.lcu.is_client_running()? {
            if !self.settings.auto_accept_enabled {
                time::sleep(Duration::from_secs(1)).await;
                continue;
            }

            let phase = self.get_game_phase().await?;
            
            match phase {
                GamePhase::Lobby => {
                    time::sleep(Duration::from_secs(5)).await;
                }
                GamePhase::Matchmaking => {
                    self.handle_matchmaking().await?;
                    time::sleep(Duration::from_secs(2)).await;
                }
                GamePhase::ReadyCheck => {
                    self.handle_ready_check().await?;
                }
                GamePhase::ChampSelect => {
                    self.handle_champ_select().await?;
                }
                GamePhase::InProgress |
                GamePhase::WaitingForStats |
                GamePhase::PreEndOfGame => {
                    time::sleep(Duration::from_secs(9)).await;
                }
                GamePhase::EndOfGame => {
                    time::sleep(Duration::from_secs(5)).await;
                }
                _ => {
                    time::sleep(Duration::from_secs(1)).await;
                }
            }

            if phase != GamePhase::ChampSelect {
                self.state.last_chat_room.clear();
            }

            time::sleep(Duration::from_millis(50)).await;
        }

        Ok(())
    }

    /// Handles the matchmaking phase
    async fn handle_matchmaking(&mut self) -> Result<()> {
        if !self.settings.auto_restart_queue {
            return Ok(());
        }

        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        if self.state.last_phase != GamePhase::Matchmaking {
            self.state.queue_start_time = current_time;
        } else if current_time - self.state.queue_start_time > self.settings.queue_max_time {
            // Cancel and restart queue
            self.lcu.request("DELETE", "/lol-lobby/v2/lobby/matchmaking/search", None).await?;
            self.lcu.request("POST", "/lol-lobby/v2/lobby/matchmaking/search", None).await?;
            self.state.queue_start_time = current_time;
        }

        self.state.last_phase = GamePhase::Matchmaking;
        Ok(())
    }

    /// Handles the ready check phase
    async fn handle_ready_check(&self) -> Result<()> {
        if !self.settings.cancel_queue_after_dodge || self.state.last_chat_room.is_empty() {
            self.lcu.request("POST", "/lol-matchmaking/v1/ready-check/accept", None).await?;
        } else {
            self.lcu.request("POST", "/lol-matchmaking/v1/ready-check/decline", None).await?;
        }
        Ok(())
    }

    /// Handles the champion select phase
    async fn handle_champ_select(&mut self) -> Result<()> {
        let (status, response) = self.lcu.request("GET", "/lol-champ-select/v1/session", None).await?;
        
        if status != 200 {
            return Ok(());
        }

        let session: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| Error::Other(format!("Failed to parse champ select session: {}", e)))?;

        // Extract chat room ID and handle state reset
        let current_chat_room = session["multiUserChatId"].as_str().unwrap_or("").to_string();
        if self.state.last_chat_room != current_chat_room || self.state.last_chat_room.is_empty() {
            self.reset_champ_select_state();
            self.state.champ_select_start = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
        }
        self.state.last_chat_room = current_chat_room;

        // If everything is done, sleep
        if self.is_champ_select_complete() {
            time::sleep(Duration::from_secs(1)).await;
            return Ok(());
        }

        // Handle champion selection, bans, and spells
        self.handle_champion_actions(&session).await?;
        self.handle_summoner_spells().await?;
        self.handle_chat_messages().await?;

        Ok(())
    }

    /// Resets the champion select state
    fn reset_champ_select_state(&mut self) {
        self.state.picked_champion = false;
        self.state.locked_champion = false;
        self.state.picked_ban = false;
        self.state.locked_ban = false;
        self.state.picked_spell1 = false;
        self.state.picked_spell2 = false;
        self.state.sent_chat_messages = false;
    }

    /// Checks if all champion select actions are complete
    fn is_champ_select_complete(&self) -> bool {
        self.state.picked_champion && 
        self.state.locked_champion && 
        self.state.picked_ban && 
        self.state.locked_ban && 
        self.state.picked_spell1 && 
        self.state.picked_spell2 && 
        self.state.sent_chat_messages
    }

    /// Handles champion selection and banning actions
    async fn handle_champion_actions(&mut self, _session: &serde_json::Value) -> Result<()> {
        if let Some(champion_id) = self.settings.current_champion {
            if !self.state.picked_champion {
                self.lcu.request(
                    "PATCH",
                    "/lol-champ-select/v1/session/actions/-1",
                    Some(json!({ "championId": champion_id }).to_string()),
                ).await?;
                self.state.picked_champion = true;
            }
        }

        if let Some(ban_id) = self.settings.current_ban {
            if !self.state.picked_ban {
                self.lcu.request(
                    "PATCH",
                    "/lol-champ-select/v1/session/actions/-1",
                    Some(json!({ "championId": ban_id }).to_string()),
                ).await?;
                self.state.picked_ban = true;
            }
        }

        Ok(())
    }

    /// Handles summoner spell selection
    async fn handle_summoner_spells(&mut self) -> Result<()> {
        if let Some(spell1) = self.settings.summoner_spell1 {
            if !self.state.picked_spell1 {
                self.lcu.request(
                    "PATCH",
                    "/lol-champ-select/v1/session/my-selection",
                    Some(json!({ "spell1Id": spell1 }).to_string()),
                ).await?;
                self.state.picked_spell1 = true;
            }
        }

        if let Some(spell2) = self.settings.summoner_spell2 {
            if !self.state.picked_spell2 {
                self.lcu.request(
                    "PATCH",
                    "/lol-champ-select/v1/session/my-selection",
                    Some(json!({ "spell2Id": spell2 }).to_string()),
                ).await?;
                self.state.picked_spell2 = true;
            }
        }

        Ok(())
    }

    /// Handles sending chat messages in champion select
    async fn handle_chat_messages(&mut self) -> Result<()> {
        if !self.settings.chat_messages_enabled || self.state.sent_chat_messages {
            return Ok(());
        }

        for message in &self.settings.chat_messages {
            self.lcu.request(
                "POST",
                &format!("/lol-chat/v1/conversations/{}/messages", self.state.last_chat_room),
                Some(json!({ "body": message }).to_string()),
            ).await?;
        }

        self.state.sent_chat_messages = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;

    #[test]
    async fn test_game_client_creation() {
        let client = GameClient::new().await;
        assert!(client.is_ok());
    }

    #[test]
    async fn test_settings_update() {
        let mut client = GameClient::new().await.unwrap();
        let mut settings = GameSettings::default();
        settings.auto_accept_enabled = true;
        client.update_settings(settings);
        assert!(client.settings.auto_accept_enabled);
    }
} 