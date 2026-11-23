#![cfg(test)]

use super::*;

#[test]
fn certificate_initialization_accounts_for_prefix_and_fill_separately() {
    for (bytes, spread, marker) in [
        (&[0xff, 0xac][..], None, true),
        (&[0x00, 0xff, 0xac][..], Some(0xff00 * 128), false),
        (&[0xff, 0x05, 0xff, 0xac][..], Some(0), false),
        (&[0xff, 0xac, 0xff, 0xac][..], Some(0xff00 * 128), true),
    ] {
        let decoder = ArithmeticDecoder::new(bytes).unwrap();
        let observer = PrefixCertainty::new(&decoder);
        assert_eq!(observer.spread, spread.unwrap_or(decoder.code));
        assert_eq!(decoder.marker.is_some(), marker);
    }
    let bytes = [0xff, 0xac];
    let mut decoder = ArithmeticDecoder::new(&bytes).unwrap();
    let mut observer = PrefixCertainty::new(&decoder);
    observer
        .decode(&mut decoder, &mut ArithmeticContext::new())
        .unwrap();
    assert!(!observer.active);
    assert!(!observer.capture(&decoder, 1));
}

#[test]
fn certified_drivers_preserve_genuine_entropy_results_and_all_state_after_disable() {
    for bytes in [
        &[0xff, 0xac][..],
        &[0x00, 0xff, 0xac][..],
        &[0x82, 0x47, 0x1a, 0x12, 0xff, 0xac][..],
        &ANNEX_H_ENCODED[..],
    ] {
        let mut actual = ArithmeticDecoder::new(bytes).unwrap();
        let mut ordinary = ArithmeticDecoder::new(bytes).unwrap();
        let mut observer = PrefixCertainty::new(&actual);
        let mut integers = [ArithmeticContext::new(); INTEGER_CONTEXT_COUNT];
        let mut ordinary_integers = integers;
        let mut iaid = [ArithmeticContext::new(); 8];
        let mut ordinary_iaid = iaid;
        for _ in 0..128 {
            let was_active = observer.active;
            let left = actual.decode_integer_certified(&mut integers, &mut observer);
            let right = ordinary.decode_integer(&mut ordinary_integers);
            assert_eq!(left, right);
            assert_decoder_state_eq(&actual, &ordinary);
            assert_eq!(integers, ordinary_integers);
            assert!(was_active || !observer.active);
            if left.is_err() {
                break;
            }
            let left = actual.decode_iaid_certified(3, &mut iaid, &mut observer);
            let right = ordinary.decode_iaid(3, &mut ordinary_iaid);
            assert_eq!(left, right);
            assert_decoder_state_eq(&actual, &ordinary);
            assert_eq!(iaid, ordinary_iaid);
            if left.is_err() {
                break;
            }
        }
    }
}
