//! Simple configuration file used to persist state between consecutive launches
use confy::ConfyError;
use serde::{Deserialize, Serialize};

const DEFAULT_CONFIG_NAME: &str = "gh_alfred";

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct GhAlfredConfig {
    /// The last time we spawned a daemon fork to update the cache
    pub last_update_start_time: Option<chrono::DateTime<chrono::Local>>,
}

impl GhAlfredConfig {
    /// load the config from the default disk location
    pub fn load() -> Result<GhAlfredConfig, ConfyError> {
        confy::load::<GhAlfredConfig>(DEFAULT_CONFIG_NAME)
    }

    /// returns weather or not we should update the cache
    /// This will return true, if the cache has not been created yet or is older than 30mn
    pub fn should_update_db(&self) -> bool {
        match self.last_update_start_time {
            None => true,
            Some(time) => time - chrono::Local::now() > chrono::Duration::minutes(30),
        }
    }

    /// update and persist the 'last_update_start_time' timestamp
    pub fn update_last_update_start_time(&mut self) -> Result<(), ConfyError> {
        self.last_update_start_time = Some(chrono::Local::now());
        self.update()
    }

    /// reset the stored 'last_update_start_time' timestamp
    pub fn reset_last_update_start_time(&mut self) -> Result<(), ConfyError> {
        self.last_update_start_time = None;
        self.update()
    }

    /// persist the configuration to disk
    fn update(&self) -> Result<(), ConfyError> {
        confy::store(DEFAULT_CONFIG_NAME, self)
    }
}
