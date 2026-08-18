mod event;
pub mod key_convert;
mod window;

pub use event::{AppEvent, KeyEventType, Modifiers};
pub use key_convert::convert_key;
pub use window::{run, MacWindow};

/// Tabs are switched with Alt+1~9 here, so labels carry the Alt key symbol.
/// The bundled Fira Code Nerd Font has U+2387; the Hangul fallback does not,
/// but it never gets asked for this one.
pub const TAB_SHORTCUT_PREFIX: &str = "\u{2387}";

/// The desktop's URL handler.
pub const URL_OPENER: &str = "xdg-open";
