use std::error::Error;
use std::ffi::OsStr;
use std::fmt;

use image::hooks::{
    decoding_hook_registered, register_decoding_hook, register_format_detection_hook,
};

use crate::{DecoderOptions, Jbig2Decoder};

const EXTENSIONS: &[&str] = &["jb2", "jbig2"];

/// A conflicting process-global decoder hook was not replaced.
#[derive(Debug)]
pub struct HookRegistrationError {
    /// Extension already owned by another registration.
    pub extension: &'static str,
}

impl fmt::Display for HookRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "an image decoder is already registered for .{}",
            self.extension
        )
    }
}

impl Error for HookRegistrationError {}

/// Register `.jb2`/`.jbig2` and the complete standalone JBIG2 magic signature.
///
/// Call once at startup. Hooks are process-global and cannot be unregistered.
/// A pre-existing conflict installs nothing; competing concurrent registrations
/// can leave partial extension registration. Direct construction avoids hooks.
///
/// # Errors
///
/// Returns the conflicting extension; existing hooks are never replaced.
pub fn register_decoding_hooks(options: DecoderOptions) -> Result<(), HookRegistrationError> {
    for &extension in EXTENSIONS {
        if decoding_hook_registered(OsStr::new(extension)) {
            return Err(HookRegistrationError { extension });
        }
    }
    for &extension in EXTENSIONS {
        if !register_decoding_hook(
            extension.into(),
            Box::new(move |reader| Ok(Box::new(Jbig2Decoder::with_options(reader, options)?))),
        ) {
            return Err(HookRegistrationError { extension });
        }
    }
    register_format_detection_hook("jb2".into(), b"\x97JB2\r\n\x1a\n", None);
    Ok(())
}
