use vstd::prelude::*;

verus! {

pub(super) proof fn qe_transition_valid(index: int)
    requires
        0 <= index < 47,
    ensures
        super::QE_STATES@[index].next_mps < 47,
        super::QE_STATES@[index].next_lps < 47,
        super::QE_STATES@[index].qe > 0,
        super::QE_STATES@[index].qe <= 0x5601,
{
    assert(super::QE_STATES@.len() == 47);
    if index == 0 {} else if index == 1 {} else if index == 2 {}
    else if index == 3 {} else if index == 4 {} else if index == 5 {}
    else if index == 6 {} else if index == 7 {} else if index == 8 {}
    else if index == 9 {} else if index == 10 {} else if index == 11 {}
    else if index == 12 {} else if index == 13 {} else if index == 14 {}
    else if index == 15 {} else if index == 16 {} else if index == 17 {}
    else if index == 18 {} else if index == 19 {} else if index == 20 {}
    else if index == 21 {} else if index == 22 {} else if index == 23 {}
    else if index == 24 {} else if index == 25 {} else if index == 26 {}
    else if index == 27 {} else if index == 28 {} else if index == 29 {}
    else if index == 30 {} else if index == 31 {} else if index == 32 {}
    else if index == 33 {} else if index == 34 {} else if index == 35 {}
    else if index == 36 {} else if index == 37 {} else if index == 38 {}
    else if index == 39 {} else if index == 40 {} else if index == 41 {}
    else if index == 42 {} else if index == 43 {} else if index == 44 {}
    else if index == 45 {} else {}
}

pub(crate) proof fn packed_context_bounds(context: super::ArithmeticContext)
    requires
        super::arithmetic_spec::context_valid(context),
    ensures
        super::arithmetic_spec::context_index(context) < 47,
        super::arithmetic_spec::context_mps(context) <= 1,
        context.packed == ((super::arithmetic_spec::context_index(context) * 2)
            + super::arithmetic_spec::context_mps(context)),
{
}

pub(super) proof fn packed_tables_match(packed: u8)
    requires
        packed < 94,
    ensures
        super::QE_PACKED_VALS@[packed as int] == super::QE_STATES@[(packed / 2) as int].qe,
        super::NEXT_MPS_PACKED@[packed as int] < 94,
        (super::NEXT_MPS_PACKED@[packed as int] / 2)
            == super::QE_STATES@[(packed / 2) as int].next_mps,
        (super::NEXT_MPS_PACKED@[packed as int] % 2) == (packed % 2),
        super::QE_STATES@[(packed / 2) as int].next_mps < 47,
        super::QE_STATES@[(packed / 2) as int].next_lps < 47,
        super::QE_STATES@[(packed / 2) as int].qe > 0,
        super::QE_STATES@[(packed / 2) as int].qe <= 0x5601,
{
    let idx: int = (packed / 2) as int;
    qe_transition_valid(idx);
    assert(super::QE_PACKED_VALS@.len() == 128);
    assert(super::NEXT_MPS_PACKED@.len() == 128);
    if packed == 0 {} else if packed == 1 {} else if packed == 2 {} else if packed == 3 {}
    else if packed == 4 {} else if packed == 5 {} else if packed == 6 {} else if packed == 7 {}
    else if packed == 8 {} else if packed == 9 {} else if packed == 10 {} else if packed == 11 {}
    else if packed == 12 {} else if packed == 13 {} else if packed == 14 {} else if packed == 15 {}
    else if packed == 16 {} else if packed == 17 {} else if packed == 18 {} else if packed == 19 {}
    else if packed == 20 {} else if packed == 21 {} else if packed == 22 {} else if packed == 23 {}
    else if packed == 24 {} else if packed == 25 {} else if packed == 26 {} else if packed == 27 {}
    else if packed == 28 {} else if packed == 29 {} else if packed == 30 {} else if packed == 31 {}
    else if packed == 32 {} else if packed == 33 {} else if packed == 34 {} else if packed == 35 {}
    else if packed == 36 {} else if packed == 37 {} else if packed == 38 {} else if packed == 39 {}
    else if packed == 40 {} else if packed == 41 {} else if packed == 42 {} else if packed == 43 {}
    else if packed == 44 {} else if packed == 45 {} else if packed == 46 {} else if packed == 47 {}
    else if packed == 48 {} else if packed == 49 {} else if packed == 50 {} else if packed == 51 {}
    else if packed == 52 {} else if packed == 53 {} else if packed == 54 {} else if packed == 55 {}
    else if packed == 56 {} else if packed == 57 {} else if packed == 58 {} else if packed == 59 {}
    else if packed == 60 {} else if packed == 61 {} else if packed == 62 {} else if packed == 63 {}
    else if packed == 64 {} else if packed == 65 {} else if packed == 66 {} else if packed == 67 {}
    else if packed == 68 {} else if packed == 69 {} else if packed == 70 {} else if packed == 71 {}
    else if packed == 72 {} else if packed == 73 {} else if packed == 74 {} else if packed == 75 {}
    else if packed == 76 {} else if packed == 77 {} else if packed == 78 {} else if packed == 79 {}
    else if packed == 80 {} else if packed == 81 {} else if packed == 82 {} else if packed == 83 {}
    else if packed == 84 {} else if packed == 85 {} else if packed == 86 {} else if packed == 87 {}
    else if packed == 88 {} else if packed == 89 {} else if packed == 90 {} else if packed == 91 {}
    else if packed == 92 {} else {}
}

}
