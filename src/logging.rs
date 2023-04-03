use js_sys::JsString;
use screeps::game;
use web_sys::console;

pub use log::LevelFilter::*;

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
        game::notify(&format!("{}", record.args()), None);
    }

    fn flush(&self) {}
}

pub fn init_logging(verbosity: log::LevelFilter) {
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
                out.finish(format_args!(
                    "{}",
                    message
                ))
            }
        })
        .chain(Box::new(JsLog) as Box<dyn log::Log>)
        .chain(
            fern::Dispatch::new()
                .level(Warn)
                .format(|out, message, _record| {
                    let time = game::time();
                    out.finish(format_args!("[{}] {}", time, message))
                })
                .chain(Box::new(JsNotify) as Box<dyn log::Log>),
        )
        .apply()
        .expect("Failed to set up logging. init_logging should only be called once per WASM VM instance.");
}
