// #![allow(unused)]

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use time::UtcOffset;
use tokio_cron_scheduler::Job;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    filter::Targets,
    fmt::{time::OffsetTime, MakeWriter},
    layer::SubscriberExt,
    reload,
    reload::Handle,
    util::SubscriberInitExt,
    Layer, Registry,
};

use crate::errors::RemoveFilesError;
use crate::prelude::EnhancedExpect;

pub type LogHandle = Handle<Targets, Registry>;

pub struct LogConfig {
    level: tracing_subscriber::filter::LevelFilter,
    /// start with bin name, following with other crates
    crates_to_log: Vec<String>,
    directory: Option<PathBuf>,
    json_format: bool,
}

impl LogConfig {
    /// crates_to_log: start with bin name, following with other crates
    pub fn new(crates_to_log: &[&str]) -> Self {
        if crates_to_log.is_empty() {
            panic!(
                "must specify at least one name of crate/bin to log with `crates_to_log` argument"
            )
        }
        Self {
            level: tracing_subscriber::filter::LevelFilter::INFO,
            crates_to_log: crates_to_log.iter().map(|s| s.to_string()).collect(),
            directory: None,
            json_format: false,
        }
    }

    pub fn level(mut self, level: log::Level) -> Self {
        self.level = match level {
            log::Level::Error => tracing_subscriber::filter::LevelFilter::ERROR,
            log::Level::Warn => tracing_subscriber::filter::LevelFilter::WARN,
            log::Level::Info => tracing_subscriber::filter::LevelFilter::INFO,
            log::Level::Debug => tracing_subscriber::filter::LevelFilter::DEBUG,
            log::Level::Trace => tracing_subscriber::filter::LevelFilter::TRACE,
        };
        self
    }

    pub fn directory(mut self, directory: impl Into<PathBuf>) -> Self {
        self.directory = Some(directory.into());
        self
    }

    pub fn with_json_format(mut self) -> Self {
        self.json_format = true;
        self
    }

    pub fn init_logger(&self) -> (Option<WorkerGuard>, Option<LogHandle>) {
        let timer = OffsetTime::new(
            UtcOffset::from_hms(8, 0, 0).ex("UtcOffset::from_hms should work"),
            time::format_description::well_known::Rfc3339,
        );
        let stdout_log = tracing_subscriber::fmt::layer().with_timer(timer.clone());
        let reg = tracing_subscriber::registry();

        let mut base_filter = Targets::new();
        for crate_name in &self.crates_to_log {
            base_filter = base_filter.with_target(crate_name, self.level);
        }

        let (filter, reload_handle) = reload::Layer::new(base_filter.clone());
        let filtered = stdout_log.with_filter(filter);

        if let Some(dir) = &self.directory {
            let file_name_prefix = format!("{}.log", self.crates_to_log[0]);
            let file_appender = tracing_appender::rolling::daily(dir, file_name_prefix);
            let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
            let layer = tracing_subscriber::fmt::layer()
                .with_timer(timer)
                .with_writer(non_blocking.make_writer());
            if self.json_format {
                let file_filter = layer.json().with_filter(base_filter);
                reg.with(filtered.and_then(file_filter)).init();
            } else {
                let file_filter = layer.with_filter(base_filter);
                reg.with(filtered.and_then(file_filter)).init();
            }
            return (Some(guard), Some(reload_handle));
        }

        reg.with(filtered).init();
        (None, Some(reload_handle))
    }
}

pub trait LogCleanerErrorHandler {
    fn handle_error(&self, error: RemoveFilesError);
}

#[derive(Clone, Debug)]
pub struct LogCleaner<P, H>
where
    P: AsRef<Path>,
    H: LogCleanerErrorHandler,
{
    pub dir: P,
    pub days: i64,
    pub cron_expression: Option<String>,
    pub error_handler: H,
}

impl<P, H> LogCleaner<P, H>
where
    P: AsRef<Path> + Sync + Send + Clone + 'static,
    H: LogCleanerErrorHandler + Sync + Send + Clone + 'static,
{
    pub fn new(dir: P, days: i64, cron_expression: Option<String>, error_handler: H) -> Self {
        Self {
            dir,
            days,
            cron_expression,
            error_handler,
        }
    }

    /// Immediately clean up files in the specified `self.dir` that have been modified more than
    /// a specified number of `self.days` ago.
    /// Typically used to clean up log files with.
    ///
    /// ```rust,ignore
    ///
    /// cleanup_files_immediately("/opt/logs/apps/", 30);
    /// ```
    pub fn cleanup_files_immediately(&self) -> Result<(), RemoveFilesError> {
        let paths = fs::read_dir(&self.dir).map_err(|e| RemoveFilesError {
            details: format!(
                "An error occurred in reading the directory and the cleanup file failed: {}",
                e
            ),
        })?;

        for path in paths.flatten().map(|e| e.path()) {
            let modified = fs::metadata(&path)
                .and_then(|metadata| metadata.modified())
                .map_err(|e| RemoveFilesError {
                    details: format!("An error occurred in getting file modified time and the cleanup file failed: {}", e),
                })?;
            if (Utc::now() - DateTime::from(modified)).num_days() > self.days {
                fs::remove_file(&path).map_err(|e| RemoveFilesError {
                    details: format!("delete file failed, path: {:?}, error: {}", path, e),
                })?;
            }
        }
        Ok(())
    }

    /// Clean up files in the specified `self.dir` that have been modified more than
    /// a specified number of `self.days` ago.
    ///
    /// ```rust,ignore
    /// // The parameter `cron_expression` default is `0 0 0 * * * *`.
    /// // The parameter `cron_expression` sample: 0 15 6,8,10 * Mar,Jun Fri 2017
    /// // means Run at second 0 of the 15th minute of the 6th, 8th, and 10th hour of any day in March
    /// // and June that is a Friday of the year 2017.
    /// // More information about `cron_expression` parameter see
    /// // https://docs.rs/job_scheduler/latest/job_scheduler/
    ///
    /// schedule_cleanup_log_files("/opt/logs/apps/", 30, None);
    /// ```
    pub async fn schedule_cleanup_log_files(self) -> Result<(), RemoveFilesError> {
        let sched = tokio_cron_scheduler::JobScheduler::new().await?;
        let cron = self
            .clone()
            .cron_expression
            .unwrap_or("0 0 0 * * * *".to_string());
        sched
            .add(Job::new_async(cron.as_str(), move |uuid, mut l| {
                let cleaner = self.clone();
                Box::pin(async move {
                    if let Err(e) = cleaner.cleanup_files_immediately() {
                        cleaner.error_handler.handle_error(e);
                    };
                    let next_tick = l.next_tick_for_job(uuid).await;
                    if let Ok(Some(ts)) = next_tick {
                        tokio::time::sleep(tokio::time::Duration::from_secs(
                            (ts - Utc::now()).num_seconds() as u64,
                        ))
                        .await
                    }
                })
            })?)
            .await?;
        sched.start().await?;
        Ok(())
    }
}

#[allow(unused, unreachable_code)]
pub fn change_debug(handle: &LogHandle, debug: &str) -> bool {
    // TODO: change_debug
    panic!("TODO: ");
    let base_filter =
        Targets::new().with_target("foo", tracing_subscriber::filter::LevelFilter::DEBUG);
    handle.modify(|filter| *filter = base_filter);
    true
}

#[cfg(test)]
mod logger_test {
    use std::fs;
    use std::time::Duration;

    use crate::errors::RemoveFilesError;
    use chrono::{DateTime, Utc};

    use crate::logger::{LogCleaner, LogCleanerErrorHandler};
    use crate::prelude::EnhancedUnwrap;

    #[derive(Clone)]
    struct MyLoggerErrorHandler;

    // define custom error handler and implement LogCleanerErrorHandler trait in application code
    impl LogCleanerErrorHandler for MyLoggerErrorHandler {
        fn handle_error(&self, error: RemoveFilesError) {
            // put custom error handling logic here
            dbg!("handling error: {:?}", error);
        }
    }

    #[test]
    fn test_delete_log_files() {
        let cleaner = LogCleaner {
            dir: "/opt/logs/apps/",
            days: 30,
            cron_expression: None,
            error_handler: MyLoggerErrorHandler,
        };
        if let Err(e) = cleaner.cleanup_files_immediately() {
            panic!("test_delete_log_files failed, error: {}", e);
        }
    }

    #[tokio::test]
    async fn test_schedule_cleanup_log_files() {
        let dir = "/opt/logs/apps/";
        let days = 30;
        let cleaner = LogCleaner {
            dir,
            days,
            // execute once every 5 seconds for testing
            cron_expression: Some("1/5 * * * * * *".to_string()),
            error_handler: MyLoggerErrorHandler,
        };

        println!("test_schedule_cleanup_log_files start");
        if let Err(e) = cleaner.schedule_cleanup_log_files().await {
            panic!("schedule_cleanup_log_files failed, error: {}", e)
        }
        println!("test_schedule_cleanup_log_files end");

        let mut has_files = true;
        let mut count = 0;
        while count < 3 {
            if let Ok(entries) = fs::read_dir(dir) {
                has_files = entries.filter_map(|entry| entry.ok()).any(|entry| {
                    entry
                        .metadata()
                        .ok()
                        .map(|md| {
                            (Utc::now() - DateTime::from(md.modified().unwp())).num_days() > days
                        })
                        .unwrap_or(false)
                });
                if !has_files {
                    return;
                }
                tokio::time::sleep(Duration::from_secs(6)).await;
                count += 1;
            }
        }
        assert!(!has_files);
    }
}
