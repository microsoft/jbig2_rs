// Included in arithmetic_spec; not a child module with broader reveal access.
verus! {

pub(super) proof fn entry_model_equations(
    interval: u32, code: u32, index: u8, mps: u8,
)
    requires index < 47, mps <= 1, interval >= super::MIN_INTERVAL,
    ensures ({
        let state = super::QE_STATES@[index as int];
        let qe = state.qe as u32;
        let reduced = (interval as int - qe as int) as u32;
        let fast = code >> 16 >= qe && reduced >= super::MIN_INTERVAL;
        entry_decision(interval, code, index, mps)
            == if fast { mps } else {
                exchange_decision(code >> 16 < qe, reduced, qe, mps)
            }
        && entry_index(interval, code, index)
            == if fast { index } else {
                exchange_index(code >> 16 < qe, reduced, qe, state.next_mps, state.next_lps)
            }
        && entry_mps(interval, code, index, mps)
            == if fast { mps } else {
                exchange_mps(code >> 16 < qe, reduced, qe, state.switch_mps, mps, mps)
            }
    }),
{
    reveal(entry_decision);
    reveal(entry_index);
    reveal(entry_mps);
}

pub(super) proof fn decision_model_equations(
    decoder: &super::ArithmeticDecoder, context: super::ArithmeticContext,
)
    requires decoder_valid(decoder), context_valid(context),
    ensures decision_state(decoder, context) == decision_registers(
        decoder,
        super::QE_STATES@[context_index(context) as int].qe as u32,
        entry_decision(
            decoder.interval, decoder.code, context_index(context), context_mps(context),
        ),
    ),
{
    reveal(decision_state);
}

pub(crate) proof fn initialization_code_bound(first: u8, next: u8)
    ensures
        (((first as u32) << 16) as nat)
            + (if first == 0xff && next > 0x8f {
                0xff00u32
            } else if first == 0xff {
                (next as u32) << 9
            } else {
                (next as u32) << 8
            }) as nat <= u32::MAX / 128,
{
    assert(((first as u32) << 16) <= 0x00ff_0000u32) by(bit_vector);
    assert(((next as u32) << 9) <= 0x0001_fe00u32) by(bit_vector);
    assert(((next as u32) << 8) <= 0x0000_ff00u32) by(bit_vector);
}

}
