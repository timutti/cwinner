use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Streak milestones that trigger special celebrations
pub const STREAK_MILESTONES: &[u32] = &[5, 10, 25, 100];

#[derive(Debug, Clone, PartialEq)]
pub struct CommitResult {
    pub first_today: bool,
    /// If the streak just hit a milestone (5, 10, 25, 100), contains the milestone value
    pub streak_milestone: Option<u32>,
}

pub const LEVELS: &[(u32, &str)] = &[
    // Arc 1: Vibe Origins (1-10)
    (0, "Vibe Initiate"),
    (100, "Prompt Whisperer"),
    (500, "Vibe Architect"),
    (1_500, "Flow State Master"),
    (5_000, "Claude Sensei"),
    (10_000, "Code Whisperer"),
    (20_000, "Vibe Lord"),
    (35_000, "Zen Master"),
    (50_000, "Transcendent"),
    (75_000, "Singularity"),
    // Arc 2: Digital Awakening (11-20)
    (101_000, "Syntax Sage"),
    (128_000, "Debug Dancer"),
    (155_000, "Refactor Ronin"),
    (183_000, "Pattern Prophet"),
    (212_000, "Algorithm Ace"),
    (242_000, "Logic Luminary"),
    (273_000, "Stack Shaman"),
    (305_000, "Binary Bard"),
    (338_000, "Byte Bishop"),
    (372_000, "Kilobyte Knight"),
    // Arc 3: Code Elements (21-30)
    (407_000, "Null Navigator"),
    (443_000, "Pointer Pilgrim"),
    (481_000, "Loop Laureate"),
    (520_000, "Recursion Rider"),
    (560_000, "Hash Hermit"),
    (601_000, "Cache Cleric"),
    (644_000, "Thread Thane"),
    (688_000, "Mutex Monk"),
    (733_000, "Buffer Baron"),
    (780_000, "Pipeline Paladin"),
    // Arc 4: Data Domains (31-40)
    (828_000, "Schema Scribe"),
    (878_000, "Query Quester"),
    (930_000, "Index Oracle"),
    (983_000, "Table Tactician"),
    (1_040_000, "Row Ranger"),
    (1_100_000, "Column Commander"),
    (1_160_000, "Join Juggernaut"),
    (1_220_000, "Shard Sentinel"),
    (1_280_000, "Replica Rogue"),
    (1_340_000, "Data Duke"),
    // Arc 5: Network Realms (41-50)
    (1_410_000, "Packet Pathfinder"),
    (1_480_000, "Socket Sorcerer"),
    (1_550_000, "Port Phantom"),
    (1_620_000, "Protocol Priest"),
    (1_700_000, "Firewall Falcon"),
    (1_780_000, "Gateway Guardian"),
    (1_860_000, "Proxy Prince"),
    (1_940_000, "Latency Lancer"),
    (2_030_000, "Bandwidth Baron"),
    (2_120_000, "Network Nomad"),
    // Arc 6: System Spirits (51-60)
    (2_210_000, "Kernel Knight"),
    (2_300_000, "Process Paladin"),
    (2_400_000, "Memory Mage"),
    (2_500_000, "Heap Herald"),
    (2_600_000, "Stack Sovereign"),
    (2_710_000, "Signal Sage"),
    (2_820_000, "Daemon Druid"),
    (2_930_000, "Cron Crusader"),
    (3_050_000, "Shell Shaman"),
    (3_170_000, "Root Regent"),
    // Arc 7: Architect (61-70)
    (3_290_000, "Module Maven"),
    (3_420_000, "Package Phantom"),
    (3_550_000, "Crate Captain"),
    (3_690_000, "Monolith Monk"),
    (3_830_000, "Microservice Mystic"),
    (3_980_000, "API Apostle"),
    (4_130_000, "REST Ranger"),
    (4_290_000, "GraphQL Guru"),
    (4_450_000, "Webhook Wizard"),
    (4_620_000, "Endpoint Emperor"),
    // Arc 8: Quality (71-80)
    (4_790_000, "Test Templar"),
    (4_970_000, "Assert Assassin"),
    (5_150_000, "Coverage Centurion"),
    (5_340_000, "Lint Lord"),
    (5_530_000, "Format Friar"),
    (5_730_000, "Review Raven"),
    (5_940_000, "Merge Monarch"),
    (6_150_000, "Deploy Deity"),
    (6_370_000, "CI Champion"),
    (6_600_000, "Pipeline Pharaoh"),
    // Arc 9: Security (81-90)
    (6_830_000, "Cipher Centurion"),
    (7_070_000, "Token Templar"),
    (7_320_000, "Auth Archon"),
    (7_580_000, "Vault Vanguard"),
    (7_850_000, "Entropy Envoy"),
    (8_120_000, "Hash Guardian"),
    (8_400_000, "Payload Paladin"),
    (8_690_000, "Sandbox Sage"),
    (8_990_000, "Keymaster"),
    (9_300_000, "Guardian Prime"),
    // Arc 10: Type System (91-100)
    (9_620_000, "Type Titan"),
    (9_950_000, "Generic Gladiator"),
    (10_300_000, "Trait Tempest"),
    (10_700_000, "Lifetime Lorekeeper"),
    (11_100_000, "Borrow Baron"),
    (11_500_000, "Ownership Oracle"),
    (11_900_000, "Closure Crusader"),
    (12_300_000, "Macro Magus"),
    (12_700_000, "Unsafe Usurper"),
    (13_100_000, "Rustacean"),
    // Arc 11: Cloud (101-110)
    (13_500_000, "Cloud Caller"),
    (14_000_000, "Container Captain"),
    (14_500_000, "Cluster Keeper"),
    (15_000_000, "Pod Prophet"),
    (15_500_000, "Volume Vagrant"),
    (16_000_000, "Ingress Inquisitor"),
    (16_500_000, "Service Scout"),
    (17_000_000, "Helm Harbinger"),
    (17_600_000, "Terraform Titan"),
    (18_200_000, "Infrastructure Imperator"),
    // Arc 12: AI & ML (111-120)
    (18_800_000, "Neural Navigator"),
    (19_400_000, "Tensor Templar"),
    (20_000_000, "Gradient Guide"),
    (20_700_000, "Model Maven"),
    (21_400_000, "Epoch Elder"),
    (22_100_000, "Attention Architect"),
    (22_800_000, "Transformer Thane"),
    (23_600_000, "Prompt Paladin"),
    (24_400_000, "Context Commander"),
    (25_200_000, "Token Titan"),
    // Arc 13: Mythical Debugging (121-130)
    (26_000_000, "Segfault Slayer"),
    (26_900_000, "Deadlock Destroyer"),
    (27_800_000, "Race Resolver"),
    (28_700_000, "Leak Liberator"),
    (29_600_000, "Panic Purifier"),
    (30_600_000, "Overflow Obliterator"),
    (31_600_000, "Null Nemesis"),
    (32_600_000, "Exception Exorcist"),
    (33_700_000, "Bug Banisher"),
    (34_800_000, "Error Eradicator"),
    // Arc 14: Cosmic Code (131-140)
    (35_900_000, "Stellar Scripter"),
    (37_100_000, "Nebula Namer"),
    (38_300_000, "Quasar Querier"),
    (39_500_000, "Pulsar Programmer"),
    (40_800_000, "Comet Coder"),
    (42_100_000, "Orbit Optimizer"),
    (43_500_000, "Eclipse Engineer"),
    (44_900_000, "Supernova Sage"),
    (46_400_000, "Galaxy Gardener"),
    (47_900_000, "Cosmos Crafter"),
    // Arc 15: Time (141-150)
    (49_400_000, "Async Ancestor"),
    (51_000_000, "Future Forger"),
    (52_600_000, "Promise Prophet"),
    (54_300_000, "Await Arbiter"),
    (56_100_000, "Concurrent Consul"),
    (57_900_000, "Parallel Paragon"),
    (59_800_000, "Temporal Titan"),
    (61_700_000, "Chrono Champion"),
    (63_700_000, "Epoch Emperor"),
    (65_800_000, "Time Lord"),
    // Arc 16: Elements (151-160)
    (67_900_000, "Iron Invoker"),
    (70_100_000, "Silicon Sage"),
    (72_400_000, "Carbon Caster"),
    (74_700_000, "Photon Phantom"),
    (77_100_000, "Plasma Priest"),
    (79_600_000, "Quantum Quester"),
    (82_200_000, "Neutron Noble"),
    (84_800_000, "Proton Prince"),
    (87_500_000, "Electron Emperor"),
    (90_300_000, "Atom Ascendant"),
    // Arc 17: Dimensional (161-170)
    (93_200_000, "Void Voyager"),
    (96_200_000, "Matrix Master"),
    (99_300_000, "Vector Virtuoso"),
    (102_000_000, "Scalar Sovereign"),
    (105_000_000, "Tensor Tyrant"),
    (108_000_000, "Dimension Drifter"),
    (112_000_000, "Plane Pathfinder"),
    (116_000_000, "Realm Ruler"),
    (120_000_000, "Sphere Sage"),
    (124_000_000, "Tesseract Titan"),
    // Arc 18: Ancient Power (171-180)
    (128_000_000, "Code Colossus"),
    (132_000_000, "Digital Demigod"),
    (136_000_000, "Cyber Centurion"),
    (140_000_000, "Silicon Samurai"),
    (145_000_000, "Chrome Chimera"),
    (150_000_000, "Titanium Templar"),
    (155_000_000, "Platinum Prophet"),
    (160_000_000, "Diamond Druid"),
    (165_000_000, "Obsidian Oracle"),
    (170_000_000, "Mythril Monarch"),
    // Arc 19: Transcendence (181-190)
    (175_000_000, "Infinite Iterator"),
    (181_000_000, "Eternal Evaluator"),
    (187_000_000, "Boundless Builder"),
    (193_000_000, "Limitless Linker"),
    (199_000_000, "Perpetual Parser"),
    (205_000_000, "Timeless Typer"),
    (212_000_000, "Ageless Allocator"),
    (219_000_000, "Undying Unwrapper"),
    (226_000_000, "Immortal Indexer"),
    (233_000_000, "Deathless Debugger"),
    // Arc 20: The Pantheon (191-200)
    (240_000_000, "Omega Overseer"),
    (248_000_000, "Alpha Architect"),
    (256_000_000, "Prime Programmer"),
    (264_000_000, "Supreme Scripter"),
    (272_000_000, "Absolute Admin"),
    (281_000_000, "Ultimate Unifier"),
    (290_000_000, "Sovereign Source"),
    (299_000_000, "Eternal Engine"),
    (309_000_000, "Apex Automaton"),
    (319_000_000, "Code God"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub xp: u32,
    pub level: u32,
    pub level_name: String,
    pub commits_total: u32,
    pub commit_streak_days: u32,
    pub last_commit_date: Option<NaiveDate>,
    pub sessions_total: u32,
    pub achievements_unlocked: Vec<String>,
    pub tools_used: HashSet<String>,
    pub last_event_at: Option<DateTime<Utc>>,
}

impl Default for State {
    fn default() -> Self {
        let (_, name) = LEVELS[0];
        Self {
            xp: 0,
            level: 1,
            level_name: name.to_string(),
            commits_total: 0,
            commit_streak_days: 0,
            last_commit_date: None,
            sessions_total: 0,
            achievements_unlocked: vec![],
            tools_used: HashSet::new(),
            last_event_at: None,
        }
    }
}

impl State {
    pub fn add_xp(&mut self, amount: u32) {
        self.xp = self.xp.saturating_add(amount);
        self.update_level();
    }

    fn update_level(&mut self) {
        for (i, &(threshold, name)) in LEVELS.iter().enumerate().rev() {
            if self.xp >= threshold {
                self.level = (i + 1) as u32;
                self.level_name = name.to_string();
                break;
            }
        }
    }

    /// Returns CommitResult with first_today flag and optional streak milestone
    pub fn record_commit(&mut self) -> CommitResult {
        self.commits_total += 1;
        let today = Utc::now().date_naive();
        let first_today = self.last_commit_date.map(|d| d != today).unwrap_or(true);
        let old_streak = self.commit_streak_days;
        if first_today {
            let yesterday = today.pred_opt().unwrap();
            if self.last_commit_date == Some(yesterday) {
                self.commit_streak_days += 1;
            } else if self.last_commit_date != Some(today) {
                self.commit_streak_days = 1;
            }
            self.last_commit_date = Some(today);
        }
        let streak_milestone = if self.commit_streak_days != old_streak {
            STREAK_MILESTONES
                .iter()
                .find(|&&m| self.commit_streak_days == m)
                .copied()
        } else {
            None
        };
        CommitResult {
            first_today,
            streak_milestone,
        }
    }

    /// Returns true if this is the first use of this tool
    pub fn record_tool_use(&mut self, tool: &str) -> bool {
        self.tools_used.insert(tool.to_string())
    }

    pub fn unlock_achievement(&mut self, id: &str) -> bool {
        if !self.achievements_unlocked.contains(&id.to_string()) {
            self.achievements_unlocked.push(id.to_string());
            true
        } else {
            false
        }
    }

    pub fn load_from(path: &Path) -> anyhow::Result<Self> {
        let s = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&s)?)
    }

    pub fn save_to(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)?;
        let tmp_path = path.with_extension("tmp");
        std::fs::write(&tmp_path, &data)?;
        std::fs::rename(&tmp_path, path)?;
        Ok(())
    }

    pub fn state_path() -> Option<PathBuf> {
        dirs::data_local_dir().map(|d| d.join("cwinner").join("state.json"))
    }

    pub fn load() -> Self {
        Self::state_path()
            .and_then(|p| Self::load_from(&p).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(path) = Self::state_path() {
            let _ = self.save_to(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_new_state_defaults() {
        let s = State::default();
        assert_eq!(s.xp, 0);
        assert_eq!(s.level, 1);
        assert_eq!(s.level_name, "Vibe Initiate");
    }

    #[test]
    fn test_add_xp_no_level_up() {
        let mut s = State::default();
        s.add_xp(50);
        assert_eq!(s.xp, 50);
        assert_eq!(s.level, 1);
    }

    #[test]
    fn test_add_xp_level_up() {
        let mut s = State::default();
        s.add_xp(100);
        assert_eq!(s.level, 2);
        assert_eq!(s.level_name, "Prompt Whisperer");
    }

    #[test]
    fn test_persist_and_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("state.json");
        let mut s = State::default();
        s.add_xp(250);
        s.save_to(&path).unwrap();
        let loaded = State::load_from(&path).unwrap();
        assert_eq!(loaded.xp, 250);
        assert_eq!(loaded.level, 2);
    }

    #[test]
    fn test_commit_streak() {
        let mut s = State::default();
        let result = s.record_commit();
        assert_eq!(s.commits_total, 1);
        assert_eq!(s.commit_streak_days, 1);
        assert!(result.first_today);
        assert_eq!(result.streak_milestone, None);
    }

    #[test]
    fn test_streak_milestone_at_5() {
        let mut s = State::default();
        // Simulate streak at 4, then commit extends to 5
        let yesterday = chrono::Utc::now().date_naive().pred_opt().unwrap();
        s.last_commit_date = Some(yesterday);
        s.commit_streak_days = 4;
        let result = s.record_commit();
        assert_eq!(s.commit_streak_days, 5);
        assert_eq!(result.streak_milestone, Some(5));
    }

    #[test]
    fn test_streak_milestone_at_10() {
        let mut s = State::default();
        let yesterday = chrono::Utc::now().date_naive().pred_opt().unwrap();
        s.last_commit_date = Some(yesterday);
        s.commit_streak_days = 9;
        let result = s.record_commit();
        assert_eq!(s.commit_streak_days, 10);
        assert_eq!(result.streak_milestone, Some(10));
    }

    #[test]
    fn test_streak_milestone_at_25() {
        let mut s = State::default();
        let yesterday = chrono::Utc::now().date_naive().pred_opt().unwrap();
        s.last_commit_date = Some(yesterday);
        s.commit_streak_days = 24;
        let result = s.record_commit();
        assert_eq!(s.commit_streak_days, 25);
        assert_eq!(result.streak_milestone, Some(25));
    }

    #[test]
    fn test_streak_milestone_at_100() {
        let mut s = State::default();
        let yesterday = chrono::Utc::now().date_naive().pred_opt().unwrap();
        s.last_commit_date = Some(yesterday);
        s.commit_streak_days = 99;
        let result = s.record_commit();
        assert_eq!(s.commit_streak_days, 100);
        assert_eq!(result.streak_milestone, Some(100));
    }

    #[test]
    fn test_no_streak_milestone_at_6() {
        let mut s = State::default();
        let yesterday = chrono::Utc::now().date_naive().pred_opt().unwrap();
        s.last_commit_date = Some(yesterday);
        s.commit_streak_days = 5;
        let result = s.record_commit();
        assert_eq!(s.commit_streak_days, 6);
        assert_eq!(result.streak_milestone, None);
    }

    #[test]
    fn test_tool_first_use() {
        let mut s = State::default();
        assert!(s.record_tool_use("Task"));
        assert!(!s.record_tool_use("Task"));
    }
}
