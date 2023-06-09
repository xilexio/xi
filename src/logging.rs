use log::LevelFilter::*;
use crate::game_time::game_tick;

struct JsLog;
struct JsNotify;

impl log::Log for JsLog {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        #[cfg(not(test))]
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
            if record.level() >= Trace {
                out.finish(format_args!(
                    "<span style=\"color: #666\">{}: {}</span>",
                    record.target(),
                    message
                ))
            } else if record.level() >= Debug {
                out.finish(format_args!(
                    "<span style=\"color: #66b\">{}: {}</span>",
                    record.target(),
                    message
                ))
            } else if record.level() <= Warn {
                out.finish(format_args!(
                    "<span style=\"color: #f99\">[{}] {}: {}</span>",
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
                    let time = game_tick();
                    out.finish(format_args!("[{}] {}", time, message))
                })
                .chain(Box::new(JsNotify) as Box<dyn log::Log>),
        )
        .apply()
        .expect("Failed to set up logging. init_logging should only be called once per WASM VM instance.");
}
