use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;

use log::warn;
use miette::Result;

use crate::api::account::Account;
use crate::api::entity::Entity;
use crate::config;

const USAGE_FILE: &str = "usage.json";

/// Which accounts have been opened, and how often.
///
/// The missing-id rule lives here once: an account with no id records nothing and counts as zero
/// usage. Adapters implement the storage for a real id only.
pub trait UsageStore {
    fn count(&self, id: u32) -> u32;

    /// Best-effort: a failed write warns instead of failing the caller.
    fn record_id(&self, id: u32);

    fn usage(&self, id: Option<&u32>) -> u32 {
        id.map_or(0, |id| self.count(*id))
    }

    fn record(&self, id: Option<&u32>) {
        match id {
            Some(id) => self.record_id(*id),
            None => warn!("Account has no id, usage not recorded"),
        }
    }

    /// Accounts ordered by usage, then by id. An account without an id counts as zero usage.
    fn sorted(&self, accounts: &[Account]) -> Vec<Account> {
        let mut list = accounts.to_vec();

        list.sort_by(|a, b| self.usage(b.id()).cmp(&self.usage(a.id())).then_with(|| a.id().cmp(&b.id())));

        list
    }
}

/// File adapter. The config directory is fixed at construction so it stops being an argument at
/// every call site.
pub struct FileUsageStore {
    path: PathBuf,
    counts: RefCell<HashMap<u32, u32>>,
    loaded: Cell<bool>,
    /// Set when the file could not be read or parsed. An empty map must not overwrite history.
    degraded: Cell<bool>,
}

impl FileUsageStore {
    pub fn open(dir: Option<&str>) -> Result<Self> {
        Ok(Self {
            path: PathBuf::from(config::get_config_path(USAGE_FILE, dir)?),
            counts: RefCell::new(HashMap::new()),
            loaded: Cell::new(false),
            degraded: Cell::new(false),
        })
    }

    fn load(&self) {
        if self.loaded.get() {
            return;
        }

        self.loaded.set(true);

        match config::read_or_write_json(&self.path, &HashMap::<u32, u32>::new()) {
            Ok(data) => match serde_json::from_str::<HashMap<u32, u32>>(&data) {
                Ok(usage) => *self.counts.borrow_mut() = usage,
                Err(e) => {
                    warn!("Failed to read usage data: {e}");
                    self.degraded.set(true);
                }
            },
            Err(e) => {
                warn!("Could not read usage data: {e}");
                self.degraded.set(true);
            }
        }
    }
}

impl UsageStore for FileUsageStore {
    fn count(&self, id: u32) -> u32 {
        self.load();

        *self.counts.borrow().get(&id).unwrap_or(&0)
    }

    fn record_id(&self, id: u32) {
        self.load();

        if self.degraded.get() {
            warn!("Usage data is unreadable, so it is not written over");
            return;
        }

        let mut counts = self.counts.borrow_mut();
        *counts.entry(id).or_insert(0) += 1;

        if let Err(e) = config::write_json(&self.path, &*counts) {
            warn!("Could not record usage: {e}");
        }
    }
}

#[cfg(test)]
#[derive(Default)]
pub struct MemoryUsageStore {
    counts: RefCell<HashMap<u32, u32>>,
}

#[cfg(test)]
impl UsageStore for MemoryUsageStore {
    fn count(&self, id: u32) -> u32 {
        *self.counts.borrow().get(&id).unwrap_or(&0)
    }

    fn record_id(&self, id: u32) {
        *self.counts.borrow_mut().entry(id).or_insert(0) += 1;
    }
}

#[cfg(test)]
mod tests {
    use crate::api::account::Account;
    use crate::api::entity::Entity;
    use crate::usage::{FileUsageStore, MemoryUsageStore, UsageStore};

    fn account(id: Option<u32>) -> Account {
        Account::new(id, "name".to_owned(), "login".to_owned(), None, None, 0, 0, None, None)
    }

    fn temp_dir() -> tempfile::TempDir {
        let temp = tempfile::tempdir().expect("Failed to create temp dir");
        std::fs::create_dir_all(temp.path().join(".syspass")).expect("Failed to create dir");

        temp
    }

    #[test]
    fn test_order_changes_after_recording() {
        let store = MemoryUsageStore::default();

        let before = store.sorted(&[account(Some(1)), account(Some(2))]);
        assert_eq!(before[0].id(), Some(&1), "equal usage falls back to id order");

        store.record(Some(&2));

        let after = store.sorted(&[account(Some(1)), account(Some(2))]);
        assert_eq!(after[0].id(), Some(&2), "the recorded account comes first");
    }

    #[test]
    fn test_record_skips_account_without_id() {
        let store = MemoryUsageStore::default();

        store.record(None);

        assert_eq!(store.usage(None), 0, "nothing was recorded");
        assert_eq!(store.sorted(&[account(None)])[0].id(), None);
    }

    #[test]
    fn test_file_adapter_records_through_the_interface() {
        let temp = temp_dir();
        let dir = temp.path().to_str().expect("Failed to get path");
        let store = FileUsageStore::open(Some(dir)).expect("Failed to create store");

        store.record(Some(&31337));
        assert_eq!(store.usage(Some(&31337)), 1);

        store.record(Some(&31337));
        assert_eq!(store.usage(Some(&31337)), 2);
    }

    #[test]
    fn test_file_adapter_degrades_on_corrupt_data() {
        let temp = temp_dir();
        let dir = temp.path().to_str().expect("Failed to get path");
        std::fs::write(temp.path().join(".syspass/usage.json"), "not json").expect("Failed to write file");

        let store = FileUsageStore::open(Some(dir)).expect("Failed to create store");

        assert_eq!(store.usage(Some(&1)), 0, "a corrupt file degrades to zero usage");
    }

    #[test]
    fn test_unparseable_file_is_not_written_over() {
        let temp = temp_dir();
        let dir = temp.path().to_str().expect("Failed to get path");
        let file = temp.path().join(".syspass/usage.json");
        std::fs::write(&file, "{\"1\":3} trailing").expect("Failed to write file");

        let store = FileUsageStore::open(Some(dir)).expect("Failed to create store");

        assert_eq!(store.usage(Some(&1)), 0, "unparseable data degrades to zero usage");

        store.record(Some(&2));

        let raw = std::fs::read_to_string(&file).expect("Failed to read the file back");
        assert!(
            raw.contains("\"1\":3"),
            "the unparseable history survives a later record"
        );
    }

    #[test]
    fn test_unreadable_file_is_not_written_over() {
        let temp = temp_dir();
        let dir = temp.path().to_str().expect("Failed to get path");
        let file = temp.path().join(".syspass/usage.json");

        // A directory where the store expects a file: reading it fails, which is
        // the branch that marks the store degraded. A chmod 000 file would test
        // the same branch but fails when the suite runs as root.
        std::fs::create_dir_all(file.parent().expect("No parent directory")).expect("Failed to create directory");
        std::fs::create_dir(&file).expect("Failed to create directory in place of the file");

        let store = FileUsageStore::open(Some(dir)).expect("Failed to create store");

        assert_eq!(store.usage(Some(&1)), 0, "an unreadable file degrades to zero usage");

        store.record(Some(&2));

        assert!(file.is_dir(), "nothing was written over the unreadable entry");
    }
}
