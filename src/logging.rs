use std::cell::RefCell;
use log::LevelFilter::*;
use crate::utils::game_tick::game_tick;

thread_local! {
    static LOG: RefCell<Vec<String>> = RefCell::new(Vec::new());
}

pub fn take_log() -> Vec<String> {
    LOG.with(|log| {
        log.replace(Vec::new())
    })
}

struct JsLog;
struct JsNotify;

impl log::Log for JsLog {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        #[cfg(not(test))]
        #[cfg(not(feature = "separate_messages"))]
        LOG.with(|log| {
            log.borrow_mut().push(format!("{}", record.args()));
        });
        #[cfg(not(test))]
        #[cfg(feature = "separate_messages")]
        web_sys::console::log_1(&js_sys::JsString::from(format!("{}", record.args())));
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
static LOGGING_INITIALIZED: std::sync::Mutex<bool> = std::sync::Mutex::new(false);

pub fn init_logging(verbosity: log::LevelFilter) {
    #[cfg(test)]
    {
        let mut lock = LOGGING_INITIALIZED.lock().unwrap();

        if *lock {
            return;
        }

        *lock = true;
    }

    fern::Dispatch::new()
        .level(verbosity)
        .format(|out, message, record| {
            #[cfg(not(test))]
            let postfix = "</span>";
            #[cfg(test)]
            let postfix = "";
            if record.level() >= Trace {
                #[cfg(not(test))]
                let prefix = "<span style=\"color: #666\">";
                #[cfg(test)]
                let prefix = "[TRACE] ";
                out.finish(format_args!(
                    "{}{}: {}{}",
                    prefix,
                    record.target(),
                    message,
                    postfix
                ))
            } else if record.level() >= Debug {
                #[cfg(not(test))]
                let prefix = "<span style=\"color: #66b\">";
                #[cfg(test)]
                let prefix = "[DEBUG] ";
                out.finish(format_args!(
                    "{}{}: {}{}",
                    prefix,
                    record.target(),
                    message,
                    postfix
                ))
            } else if record.level() <= Warn {
                #[cfg(not(test))]
                let prefix = "<span style=\"color: #f99\">";
                #[cfg(test)]
                let prefix = "";
                out.finish(format_args!(
                    "{}[{}] {}: {}{}",
                    prefix,
                    record.level(),
                    record.target(),
                    message,
                    postfix
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
                    let time = game_tick();
                    out.finish(format_args!("[{}] {}", time, message))
                })
                .chain(Box::new(JsNotify) as Box<dyn log::Log>),
        )
        .apply()
        .expect("Failed to set up logging. init_logging should only be called once per WASM VM instance.");
}
