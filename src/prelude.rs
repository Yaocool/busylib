use std::{backtrace::Backtrace, fmt::Display};

use log::error;

trait DisplayBackTrace {
    fn to_human_readable(&self) -> String;
}

impl DisplayBackTrace for Backtrace {
    fn to_human_readable(&self) -> String {
        let full = format!("{}", self);
        let split = full.split('\n');
        let mut result = String::new();
        let mut skip_next_line = false;
        let mut is_function_name_line = true;
        for (i, line) in split.enumerate() {
            let first_colon_index = line.find(':').unwrap_or(0);
            let string_after_colon_with_space = if line.is_empty() {
                ""
            } else {
                &line[first_colon_index + 2..]
            };
            if i >= 8 {
                if skip_next_line {
                    skip_next_line = false;
                    continue;
                }
                if string_after_colon_with_space.starts_with("busylib::")
                    || string_after_colon_with_space.starts_with("tokio::")
                    || string_after_colon_with_space.contains("busylib::prelude::EnhancedExpect")
                {
                    skip_next_line = true;
                    continue;
                }
                if string_after_colon_with_space
                    .starts_with("std::sys_common::backtrace::__rust_begin_short_backtrace")
                    || string_after_colon_with_space.starts_with("<F as axum::handler::Handler")
                {
                    break;
                }
                if !is_function_name_line {
                    result.push(' ');
                }
                result.push_str(line.trim_start_matches(' '));
                if !is_function_name_line {
                    result.push('\n');
                }
                is_function_name_line = !is_function_name_line;
            }
        }
        result
    }
}

pub trait EnhancedUnwrap<T> {
    /// Equivalent to [`Option::unwrap`] & [`Result::unwrap`] with additional logging
    fn unwp(self) -> T;
}

pub trait EnhancedExpect<T, E: Display> {
    /// Equivalent to [`Option::expect`] & [`Result::expect`] with additional logging.
    /// [`EnhancedExpect::ex`] stands for Expect, Extra(logging), Exception, Enhanced
    fn ex(self, msg: &str) -> T;
}

impl<T, E: Display> EnhancedUnwrap<T> for Result<T, E> {
    #[inline]
    fn unwp(self) -> T {
        ok(self)
    }
}

impl<T, E: Display> EnhancedExpect<T, E> for Result<T, E> {
    #[inline]
    fn ex(self, msg: &str) -> T {
        ok_ctx(self, msg)
    }
}

impl<T> EnhancedUnwrap<T> for Option<T> {
    #[inline]
    fn unwp(self) -> T {
        some(self)
    }
}

impl<T> EnhancedExpect<T, String> for Option<T> {
    #[inline]
    fn ex(self, msg: &str) -> T {
        some_ctx(self, msg)
    }
}

#[inline]
pub fn ok<T, E: Display>(result: Result<T, E>) -> T {
    ok_ctx(result, "")
}

#[inline]
pub fn some<T>(option: Option<T>) -> T {
    some_ctx(option, "")
}

/// [`Result`] should be ok with custom context
#[inline]
pub fn ok_ctx<T, E: Display>(result: Result<T, E>, msg: &str) -> T {
    match result {
        Ok(value) => value,
        Err(e) => {
            log_and_panic(Some(e), msg);
        }
    }
}

/// [`Option`] should be some with custom context
#[inline]
pub fn some_ctx<T>(option: Option<T>, msg: &str) -> T {
    match option {
        Some(value) => value,
        None => {
            log_and_panic::<String>(None, msg);
        }
    }
}

#[inline]
fn log_and_panic<E: Display>(err: Option<E>, msg: &str) -> ! {
    let err_msg = match err {
        Some(e) => format!("{}", e),
        None => "".to_string(),
    };

    let info = format!(
        "this should never happen: {}, context: {}, back_trace: {}",
        err_msg,
        msg,
        Backtrace::force_capture().to_human_readable()
    );
    error!("{}", info);
    panic!("{}", info);
}

#[cfg(test)]
mod test {
    use crate::prelude::EnhancedExpect;

    #[tokio::test]
    async fn prelude_ex() {
        let counter = std::sync::atomic::AtomicUsize::new(0);
        tokio::spawn(async move {
            loop {
                let a: Option<usize> = None;
                counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if counter.load(std::sync::atomic::Ordering::Relaxed) == 2 {
                    a.ex("on purpose");
                }
            }
        });
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}
