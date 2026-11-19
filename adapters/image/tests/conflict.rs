//! Pre-existing hook conflicts must not replace owners or install partial hooks.

use std::ffi::OsStr;

use image::error::{ImageFormatHint, UnsupportedError};
use jbig2_image::{register_decoding_hooks, DecoderOptions};

#[test]
fn existing_hook_conflict_does_not_install_partial_registration_or_replace_owner() {
    assert!(image::hooks::register_decoding_hook(
        "jbig2".into(),
        Box::new(|_| {
            Err(image::ImageError::Unsupported(
                UnsupportedError::from_format_and_kind(
                    ImageFormatHint::Unknown,
                    image::error::UnsupportedErrorKind::GenericFeature("existing owner".into()),
                ),
            ))
        })
    ));
    assert_eq!(
        register_decoding_hooks(DecoderOptions::default())
            .unwrap_err()
            .extension,
        "jbig2"
    );
    assert!(!image::hooks::decoding_hook_registered(OsStr::new("jb2")));
    assert!(image::hooks::decoding_hook_registered(OsStr::new("jbig2")));
}
