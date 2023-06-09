use std::sync::RwLock;
#[cfg(not(test))]
use js_sys::JsString;
#[cfg(not(test))]
use web_sys::console;

use log::LevelFilter::*;
use crate::game_time::game_time;

struct JsLog;
struct JsNotify;

impl log::Log for JsLog {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        #[cfg(not(test))]
        console::log_1(&JsString::from(format!("{}", record.args())));
        #[cfg(test)]
        println!("{}", record.args());
    }

    fn flush(&self) {}
}

impl log::Log for JsNotify {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        #[cfg(not(test))]
        screeps::game::notify(&format!("{}", record.args()), None);
    }

    fn flush(&self) {}
}

#[cfg(test)]
static mut LOGGING_INITIALIZED: std::sync::Mutex<bool> = std::sync::Mutex::new(false);

pub fn init_logging(verbosity: log::LevelFilter) {
    #[cfg(test)]
    unsafe {
        let mut lock = LOGGING_INITIALIZED.lock().unwrap();

        if *lock {
            return;
        }

        *lock = true;
    }

    fern::Dispatch::new()
        .level(verbosity)
        .format(|out, message, record| {
            if record.level() >= Debug {
                out.finish(format_args!(
                    "<span style=\"color: #6666bb\">{}: {}</span>",
                    record.target(),
                    message
                ))
            } else if record.level() <= Warn {
                out.finish(format_args!(
                    "<span style=\"color: #ff9999\">[{}] {}: {}</span>",
                    record.level(),
                    record.target(),
                    message
                ))
            } else {
                out.finish(format_args!("{}", message))
            }
        })
        .chain(Box::new(JsLog) as Box<dyn log::Log>)
        .chain(
            fern::Dispatch::new()
                .level(Warn)
                .format(|out, message, _record| {
                    let time = game_time();
                    out.finish(format_args!("[{}] {}", time, message))
                })
                .chain(Box::new(JsNotify) as Box<dyn log::Log>),
        )
        .apply()
        .expect("Failed to set up logging. init_logging should only be called once per WASM VM instance.");
}
