//! All-time statistics, kept in `stats[.<preset>].json` in the data directory.
//!
//! Two things are recorded: the work phases that finished on each day, and the tasks whose spent
//! pomodoros went over their estimate. The tasks are remembered by id, so they stay in the
//! statistics after they are cleared from the list.

use std::{collections::BTreeMap, fs, io, path::Path};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::Task;

const DAY_FORMAT: &str = "%Y-%m-%d";

/// A task that took more pomodoros than estimated, as last seen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Overrun {
    pub text: String,
    pub estimated: u32,
    pub spent: u32,
}

impl Overrun {
    /// How many pomodoros over the estimate.
    pub fn excess(&self) -> u32 {
        self.spent.saturating_sub(self.estimated)
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Stats {
    /// Finished work phases per local day (`YYYY-MM-DD`).
    #[serde(default)]
    days: BTreeMap<String, u32>,
    /// Tasks that went over their estimate, by task id.
    #[serde(default)]
    overruns: BTreeMap<u64, Overrun>,
}

impl Stats {
    /// Reads the file; a missing file is an empty `Stats`. A file that cannot be read is moved
    /// aside (`.corrupt`) instead of being overwritten later, and reported in the `Err`.
    pub fn load(path: &Path) -> (Stats, Option<String>) {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return (Stats::default(), None),
            Err(err) => return (Stats::default(), Some(format!("stats: {err}"))),
        };
        match serde_json::from_str(&text) {
            Ok(stats) => (stats, None),
            Err(err) => {
                let aside = path.with_extension("json.corrupt");
                let moved = fs::rename(path, &aside).is_ok();
                let note = if moved {
                    format!("stats: {err}; the old file was kept as {}", aside.display())
                } else {
                    format!("stats: {err}")
                };
                (Stats::default(), Some(note))
            }
        }
    }

    /// Writes a temporary file and renames it, so a crash can't leave half a file.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        fs::rename(tmp, path)
    }

    /// A work phase finished on `day`.
    pub fn record_pomodoro(&mut self, day: NaiveDate) {
        *self.days.entry(day.format(DAY_FORMAT).to_string()).or_insert(0) += 1;
    }

    /// Updates the overruns from the live task list; returns whether anything changed.
    ///
    /// A live task over its estimate is added or updated. A live task that is no longer over it (its
    /// estimate was raised) is removed. Tasks that left the list keep their last record.
    pub fn observe_tasks(&mut self, tasks: &[Task]) -> bool {
        let mut changed = false;
        for task in tasks.iter().filter(|task| task.id != 0) {
            if task.completed_pomodoros > task.estimated_pomodoros {
                let overrun = Overrun {
                    text: task.text.clone(),
                    estimated: task.estimated_pomodoros,
                    spent: task.completed_pomodoros,
                };
                if self.overruns.get(&task.id) != Some(&overrun) {
                    self.overruns.insert(task.id, overrun);
                    changed = true;
                }
            } else if self.overruns.remove(&task.id).is_some() {
                changed = true;
            }
        }
        changed
    }

    /// The tasks that went furthest over their estimate, worst first.
    pub fn worst_estimates(&self, limit: usize) -> Vec<&Overrun> {
        let mut all: Vec<&Overrun> = self.overruns.values().collect();
        all.sort_by(|a, b| {
            b.excess()
                .cmp(&a.excess())
                .then(b.spent.cmp(&a.spent))
                .then_with(|| a.text.cmp(&b.text))
        });
        all.truncate(limit);
        all
    }

    /// Finished work phases of the last `count` days ending at `today`, oldest first
    /// (days without any are 0).
    pub fn last_days(&self, today: NaiveDate, count: usize) -> Vec<(NaiveDate, u32)> {
        (0..count as i64)
            .rev()
            .filter_map(|ago| today.checked_sub_signed(chrono::Duration::days(ago)))
            .map(|day| {
                let key = day.format(DAY_FORMAT).to_string();
                (day, self.days.get(&key).copied().unwrap_or(0))
            })
            .collect()
    }

    pub fn total_pomodoros(&self) -> u32 {
        self.days.values().sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, DAY_FORMAT).unwrap()
    }

    fn task(id: u64, text: &str, estimated: u32, spent: u32) -> Task {
        Task {
            id,
            text: text.into(),
            estimated_pomodoros: estimated,
            completed_pomodoros: spent,
            done: false,
        }
    }

    #[test]
    fn pomodoros_are_counted_per_day_and_missing_days_are_zero() {
        let mut stats = Stats::default();
        stats.record_pomodoro(day("2026-10-09"));
        stats.record_pomodoro(day("2026-10-09"));
        stats.record_pomodoro(day("2026-10-11"));
        stats.record_pomodoro(day("2026-09-01")); // outside the window, still in the total

        let days = stats.last_days(day("2026-10-11"), 15);
        assert_eq!(days.len(), 15);
        assert_eq!(days[0].0, day("2026-09-27"));
        assert_eq!(days[14], (day("2026-10-11"), 1));
        assert_eq!(days[12], (day("2026-10-09"), 2));
        assert_eq!(days[13], (day("2026-10-10"), 0));
        assert_eq!(stats.total_pomodoros(), 4);
    }

    #[test]
    fn only_tasks_over_their_estimate_are_remembered_and_they_outlive_the_list() {
        let mut stats = Stats::default();
        let mut tasks = vec![task(1, "fine", 3, 3), task(2, "slow", 2, 5), task(3, "late", 1, 2)];
        assert!(stats.observe_tasks(&tasks));
        assert!(!stats.observe_tasks(&tasks), "nothing new the second time");

        let worst = stats.worst_estimates(10);
        assert_eq!(worst.len(), 2);
        assert_eq!((worst[0].text.as_str(), worst[0].excess()), ("slow", 3));
        assert_eq!((worst[1].text.as_str(), worst[1].excess()), ("late", 1));

        // Still growing: updated in place, not duplicated.
        tasks[1].completed_pomodoros = 6;
        assert!(stats.observe_tasks(&tasks));
        assert_eq!(stats.worst_estimates(10)[0].spent, 6);

        // The task leaves the list (cleared): its record stays.
        tasks.remove(1);
        stats.observe_tasks(&tasks);
        assert_eq!(stats.worst_estimates(10).len(), 2);

        // A live task whose estimate was raised is no longer a wrong estimate.
        tasks[1].estimated_pomodoros = 2;
        assert!(stats.observe_tasks(&tasks));
        assert_eq!(stats.worst_estimates(10).len(), 1);
    }

    #[test]
    fn worst_estimates_are_limited_and_ties_are_stable() {
        let mut stats = Stats::default();
        let tasks: Vec<Task> = (1..=12)
            .map(|i| task(i, &format!("t{i:02}"), 1, 1 + i as u32))
            .collect();
        stats.observe_tasks(&tasks);
        let worst = stats.worst_estimates(10);
        assert_eq!(worst.len(), 10);
        assert_eq!(worst[0].text, "t12");
        assert_eq!(worst[9].text, "t03");
    }

    #[test]
    fn tasks_without_an_id_are_ignored() {
        let mut stats = Stats::default();
        assert!(!stats.observe_tasks(&[task(0, "no id yet", 1, 4)]));
    }

    #[test]
    fn stats_survive_a_save_and_a_load_and_a_bad_file_is_kept_aside() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/test-tmp/stats");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("stats.json");

        assert!(Stats::load(&path).1.is_none(), "a missing file is fine");

        let mut stats = Stats::default();
        stats.record_pomodoro(day("2026-10-11"));
        stats.observe_tasks(&[task(7, "slow", 1, 3)]);
        stats.save(&path).unwrap();

        let (loaded, error) = Stats::load(&path);
        assert!(error.is_none());
        assert_eq!(loaded.total_pomodoros(), 1);
        assert_eq!(loaded.worst_estimates(10)[0].spent, 3);

        fs::write(&path, "not json").unwrap();
        let (empty, error) = Stats::load(&path);
        assert_eq!(empty.total_pomodoros(), 0);
        assert!(error.unwrap().contains("kept as"));
        assert!(dir.join("stats.json.corrupt").exists());
    }
}
