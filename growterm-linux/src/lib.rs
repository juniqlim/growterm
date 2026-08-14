mod event;
pub mod key_convert;
mod window;

pub use event::{AppEvent, KeyEventType, Modifiers};
pub use key_convert::convert_key;
pub use window::{run, MacWindow};

/// Tabs are switched with Alt+1~9 here, so labels hint Alt.
pub const TAB_SHORTCUT_PREFIX: &str = "Alt";
