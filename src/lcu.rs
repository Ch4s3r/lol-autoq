use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use nom::{
    bytes::complete::take_until,
    character::complete::{char, u16 as nom_u16},
    sequence::terminated,
    IResult, Parser,
};
use reqwest::{Client, ClientBuilder};
use serde::{Deserialize, Serialize};
use std::fs;
use tracing::{debug, trace};

// --------------------------------------------------------------------------
// Lockfile
// --------------------------------------------------------------------------

/// Candidate lockfile paths for a typical Windows League of Legends install.
const LOCKFILE_CANDIDATES: &[&str] = &[
    r"C:\Riot Games\League of Legends\lockfile",
    r"D:\Riot Games\League of Legends\lockfile",
    r"E:\Riot Games\League of Legends\lockfile",
    r"C:\Program Files\Riot Games\League of Legends\lockfile",
    r"D:\Program Files\Riot Games\League of Legends\lockfile",
];

#[derive(Debug, Clone)]
pub struct LockfileData {
    pub port: u16,
    pub password: String,
}

impl LockfileData {
    /// Parse the lockfile content. Format: `Name:PID:PORT:PASSWORD:PROTOCOL`
    pub fn from_str(content: &str) -> Result<Self> {
        parse_lockfile(content.trim())
            .map(|(_, data)| data)
            .map_err(|e| anyhow!("Malformed lockfile: {e}"))
    }

    /// Try to read the lockfile from a specific path or scan candidate paths.
    pub fn find(override_path: Option<&str>) -> Result<Self> {
        if let Some(path) = override_path {
            let content = fs::read_to_string(path)
                .with_context(|| format!("Cannot read lockfile at {}", path))?;
            return Self::from_str(&content);
        }

        // Scan candidates
        for candidate in LOCKFILE_CANDIDATES {
            if let Ok(content) = fs::read_to_string(candidate) {
                trace!(path = candidate, "Lockfile found");
                return Self::from_str(&content);
            }
        }

        Err(anyhow!(
            "League of Legends lockfile not found. \
             Make sure the client is running. \
             You can also set 'lockfile_path' in config.toml."
        ))
    }
}

/// nom parser for the lockfile. Format: `Name:PID:PORT:PASSWORD:PROTOCOL`
fn parse_lockfile(input: &str) -> IResult<&str, LockfileData> {
    let (input, _name)    = terminated(take_until(":"), char(':')).parse(input)?;
    let (input, _pid)     = terminated(take_until(":"), char(':')).parse(input)?;
    let (input, port)     = terminated(nom_u16,          char(':')).parse(input)?;
    let (input, password) = take_until(":").parse(input)?;
    Ok((input, LockfileData { port, password: password.to_owned() }))
}

// --------------------------------------------------------------------------
// LCU HTTP client
// --------------------------------------------------------------------------

pub struct LcuClient {
    client: Client,
    base_url: String,
    auth_header: String,
}

impl LcuClient {
    pub fn new(lockfile: &LockfileData) -> Result<Self> {
        // LCU uses a self-signed certificate; we must ignore TLS validation.
        let client = ClientBuilder::new()
            .danger_accept_invalid_certs(true)
            .build()
            .context("Failed to build HTTP client")?;

        let credentials = STANDARD.encode(format!("riot:{}", lockfile.password));
        Ok(Self {
            client,
            base_url: format!("https://127.0.0.1:{}", lockfile.port),
            auth_header: format!("Basic {}", credentials),
        })
    }

    // ------------------------------------------------------------------
    // Generic helpers
    // ------------------------------------------------------------------

    async fn get<T: for<'de> serde::Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        trace!(method = "GET", %url, "sending request");
        let resp = self
            .client
            .get(&url)
            .header("Authorization", &self.auth_header)
            .send()
            .await
            .with_context(|| format!("GET {} failed", path))?;

        let status = resp.status();
        trace!(method = "GET", %url, %status, "response received");
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("GET {} returned {}: {}", path, status, body));
        }
        let text = resp
            .text()
            .await
            .with_context(|| format!("Failed to read response body for GET {}", path))?;
        debug!(method = "GET", %url, body = %text, "response body");
        serde_json::from_str::<T>(&text).with_context(|| {
            let preview = if text.len() > 500 {
                format!("{}...", &text[..500])
            } else {
                text.clone()
            };
            format!(
                "Failed to deserialize GET {} — response preview: {}",
                path, preview
            )
        })
    }

    async fn post_no_body(&self, path: &str) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        trace!(method = "POST", %url, "sending request (no body)");
        let resp = self
            .client
            .post(&url)
            .header("Authorization", &self.auth_header)
            .send()
            .await
            .with_context(|| format!("POST {} failed", path))?;

        let status = resp.status();
        trace!(method = "POST", %url, %status, "response received");
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("POST {} returned {}: {}", path, status, body));
        }
        debug!(method = "POST", %url, %status, "response body: (empty)");
        Ok(())
    }

    async fn patch_json<B: Serialize>(&self, path: &str, body: &B) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        trace!(method = "PATCH", %url, "sending request");
        let resp = self
            .client
            .patch(&url)
            .header("Authorization", &self.auth_header)
            .json(body)
            .send()
            .await
            .with_context(|| format!("PATCH {} failed", path))?;

        let status = resp.status();
        trace!(method = "PATCH", %url, %status, "response received");
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("PATCH {} returned {}: {}", path, status, body));
        }
        debug!(method = "PATCH", %url, %status, "response body: (empty)");
        Ok(())
    }

    // ------------------------------------------------------------------
    // Gameflow
    // ------------------------------------------------------------------

    /// Returns the current gameflow phase, e.g. `"ReadyCheck"`, `"ChampSelect"`, `"None"`.
    pub async fn get_gameflow_phase(&self) -> Result<String> {
        self.get("/lol-gameflow/v1/gameflow-phase").await
    }

    // ------------------------------------------------------------------
    // Matchmaking / Ready-Check
    // ------------------------------------------------------------------

    pub async fn accept_ready_check(&self) -> Result<()> {
        self.post_no_body("/lol-matchmaking/v1/ready-check/accept")
            .await
    }

    // ------------------------------------------------------------------
    // Champion Select
    // ------------------------------------------------------------------

    pub async fn get_champ_select_session(&self) -> Result<ChampSelectSession> {
        self.get("/lol-champ-select/v1/session").await
    }

    /// Hover (preview) a champion without locking it in.
    pub async fn hover_champion(&self, action_id: i64, champion_id: i64) -> Result<()> {
        #[derive(Serialize)]
        struct Body {
            #[serde(rename = "championId")]
            champion_id: i64,
            completed: bool,
        }
        self.patch_json(
            &format!("/lol-champ-select/v1/session/actions/{}", action_id),
            &Body { champion_id, completed: false },
        )
        .await
    }

    /// Lock in a champion: PATCH with `completed: true` to set and complete in one call,
    /// then POST to /complete as a belt-and-suspenders fallback.
    pub async fn lock_champion(&self, action_id: i64, champion_id: i64) -> Result<()> {
        #[derive(Serialize)]
        struct Body {
            #[serde(rename = "championId")]
            champion_id: i64,
            completed: bool,
        }
        self.patch_json(
            &format!("/lol-champ-select/v1/session/actions/{}", action_id),
            &Body { champion_id, completed: true },
        )
        .await?;
        // Some LCU versions need the explicit /complete POST as well.
        let _ = self.post_no_body(&format!(
            "/lol-champ-select/v1/session/actions/{}/complete",
            action_id
        ))
        .await;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Champion data
    // ------------------------------------------------------------------

    /// Returns all champions as {id, name, alias} objects.
    pub async fn get_champion_summary(&self) -> Result<Vec<ChampionSummary>> {
        self.get("/lol-game-data/assets/v1/champion-summary.json")
            .await
    }
}

// --------------------------------------------------------------------------
// LCU data models
// --------------------------------------------------------------------------

#[derive(Debug, Deserialize, Clone)]
pub struct ChampSelectSession {
    #[serde(rename = "localPlayerCellId")]
    pub local_player_cell_id: i64,
    /// Phase timer — used to decide when to lock in.
    pub timer: PhaseTimer,
    /// 2-D array: outer = phase, inner = actions within that phase.
    pub actions: Vec<Vec<Action>>,
    #[serde(rename = "myTeam")]
    pub my_team: Vec<TeamMember>,
    /// Champions already picked or banned in this session.
    #[serde(rename = "bans", default)]
    pub bans: Bans,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct Bans {
    #[serde(rename = "myTeamBans", default)]
    pub my_team_bans: Vec<i64>,
    #[serde(rename = "theirTeamBans", default)]
    pub their_team_bans: Vec<i64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Action {
    pub id: i64,
    #[serde(rename = "actorCellId")]
    pub actor_cell_id: i64,
    #[serde(rename = "type")]
    pub action_type: String,
    #[serde(rename = "isInProgress")]
    pub is_in_progress: bool,
    pub completed: bool,
    #[serde(rename = "championId", default)]
    pub champion_id: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TeamMember {
    #[serde(rename = "cellId")]
    pub cell_id: i64,
    #[serde(rename = "assignedPosition")]
    pub assigned_position: String,
    #[serde(rename = "championId", default)]
    pub champion_id: i64,
    #[serde(rename = "summonerId", default)]
    #[allow(dead_code)]
    pub summoner_id: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChampionSummary {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub alias: String,
    /// Path to the square portrait served by the LCU.
    /// Non-playable champions (bots, special variants) use the
    /// placeholder path ending in `/-1.png`; we filter those out.
    #[serde(rename = "squarePortraitPath", default)]
    pub square_portrait_path: String,
}

impl ChampionSummary {
    /// Returns `true` for champions that are selectable in a real game.
    /// Filters out bot/doombot variants whose portrait path is the
    /// default placeholder (`-1.png`) or is missing entirely.
    pub fn is_playable(&self) -> bool {
        self.id > 0
            && !self.name.is_empty()
            && !self.square_portrait_path.is_empty()
            && !self.square_portrait_path.ends_with("/-1.png")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── LockfileData::from_str ────────────────────────────────────────────────

    #[test]
    fn lockfile_parse_extracts_port_and_password() {
        let data = LockfileData::from_str("LeagueClient:12345:54321:s3cr3t-p4ss:https").unwrap();
        assert_eq!(data.port, 54321);
        assert_eq!(data.password, "s3cr3t-p4ss");
    }

    #[test]
    fn lockfile_parse_trims_surrounding_whitespace() {
        let data = LockfileData::from_str("  LeagueClient:12345:54321:mypassword:https  ").unwrap();
        assert_eq!(data.port, 54321);
        assert_eq!(data.password, "mypassword");
    }

    #[test]
    fn lockfile_parse_handles_trailing_newline() {
        let data = LockfileData::from_str("LeagueClient:12345:54321:mypassword:https\n").unwrap();
        assert_eq!(data.port, 54321);
        assert_eq!(data.password, "mypassword");
    }

    #[test]
    fn lockfile_parse_rejects_malformed_input() {
        assert!(LockfileData::from_str("not_a_lockfile").is_err());
        assert!(LockfileData::from_str("").is_err());
    }

    // ── ChampionSummary::is_playable ──────────────────────────────────────────

    #[test]
    fn champion_summary_is_playable_accepts_valid_champion() {
        let c = ChampionSummary {
            id: 103,
            name: "Ahri".into(),
            alias: "Ahri".into(),
            square_portrait_path: "/lol-game-data/assets/v1/champion-icons/103.png".into(),
        };
        assert!(c.is_playable());
    }

    #[test]
    fn champion_summary_is_playable_rejects_negative_id() {
        let c = ChampionSummary {
            id: -1,
            name: "Bot".into(),
            alias: "Bot".into(),
            square_portrait_path: "/champs/Bot.png".into(),
        };
        assert!(!c.is_playable());
    }

    #[test]
    fn champion_summary_is_playable_rejects_placeholder_path() {
        let c = ChampionSummary {
            id: 1,
            name: "Test".into(),
            alias: "Test".into(),
            square_portrait_path: "/lol-game-data/assets/v1/champion-icons/-1.png".into(),
        };
        assert!(!c.is_playable());
    }

    #[test]
    fn champion_summary_is_playable_rejects_empty_name() {
        let c = ChampionSummary {
            id: 1,
            name: "".into(),
            alias: "Test".into(),
            square_portrait_path: "/champs/Test.png".into(),
        };
        assert!(!c.is_playable());
    }

    #[test]
    fn champion_summary_is_playable_rejects_empty_portrait_path() {
        let c = ChampionSummary {
            id: 1,
            name: "Test".into(),
            alias: "Test".into(),
            square_portrait_path: "".into(),
        };
        assert!(!c.is_playable());
    }
}

/// Timer info embedded in a champ-select session.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct PhaseTimer {
    /// Milliseconds remaining in the current pick/ban phase.
    #[serde(rename = "adjustedTimeLeftInPhase", default)]
    pub adjusted_time_left_ms: i64,
    /// Total milliseconds for the current phase.
    #[serde(rename = "totalTimeInPhase", default)]
    #[allow(dead_code)]
    pub total_time_ms: i64,
    /// Current sub-phase name, e.g. "PLANNING", "BAN_PICK", "FINALIZATION".
    #[serde(rename = "phase", default)]
    pub phase: String,
}
