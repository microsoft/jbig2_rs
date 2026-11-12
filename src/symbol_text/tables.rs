//! Huffman table association, selection and cached materialization.

#[cfg(verus_keep_ghost)]
use super::symbol_text_spec;
use super::{
    BoundHuffmanTable, CustomHuffmanTable, DecodeError, HuffmanError, HuffmanFieldChoices,
    HuffmanFieldTables, HuffmanTable, IntegerField, Limits, MalformedError,
    MaterializedHuffmanTable, StandardHuffmanTable, StandardHuffmanTables, SymbolDictionaryConfig,
    SymbolTextError, TableBinding, TableBindings, TableChoice, TableParameter, TextRegionConfig,
    MAX_TABLE_BINDINGS,
};
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures out == (left == right),
))]
pub(super) fn same_table_parameter(left: TableParameter, right: TableParameter) -> bool {
    matches!(
        (left, right),
        (
            TableParameter::DictionaryHeight,
            TableParameter::DictionaryHeight
        ) | (
            TableParameter::DictionaryWidth,
            TableParameter::DictionaryWidth
        ) | (
            TableParameter::DictionaryBitmapSize,
            TableParameter::DictionaryBitmapSize
        ) | (
            TableParameter::DictionaryAggregationInstances,
            TableParameter::DictionaryAggregationInstances
        ) | (TableParameter::TextFirstS, TableParameter::TextFirstS)
            | (
                TableParameter::TextSubsequentS,
                TableParameter::TextSubsequentS
            )
            | (
                TableParameter::TextStripDelta,
                TableParameter::TextStripDelta
            )
            | (
                TableParameter::TextRefinementWidth,
                TableParameter::TextRefinementWidth
            )
            | (
                TableParameter::TextRefinementHeight,
                TableParameter::TextRefinementHeight
            )
            | (
                TableParameter::TextRefinementX,
                TableParameter::TextRefinementX
            )
            | (
                TableParameter::TextRefinementY,
                TableParameter::TextRefinementY
            )
            | (
                TableParameter::TextRefinementSize,
                TableParameter::TextRefinementSize
            )
    )
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(bindings) => symbol_text_spec::table_bindings_valid(&bindings),
        Err(_) => true,
    },
))]
pub(super) fn associate_dictionary_tables<'a>(
    config: &SymbolDictionaryConfig,
    custom: &'a [CustomHuffmanTable<'a>],
) -> Result<TableBindings<'a>, SymbolTextError> {
    if !config.huffman {
        if custom.is_empty() {
            return Ok(TableBindings {
                entries: [None; MAX_TABLE_BINDINGS],
                len: 0,
            });
        }
        return Err(SymbolTextError::InvalidTableAssociation(
            "custom tables on arithmetic dictionary",
        ));
    }
    let specifications = [
        (TableParameter::DictionaryHeight, config.height_delta, false),
        (TableParameter::DictionaryWidth, config.width_delta, true),
        (
            TableParameter::DictionaryBitmapSize,
            config.bitmap_size,
            false,
        ),
        (
            TableParameter::DictionaryAggregationInstances,
            config.aggregation_instances,
            false,
        ),
    ];
    bind_tables(&specifications, custom)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(bindings) => symbol_text_spec::table_bindings_valid(&bindings),
        Err(_) => true,
    },
))]
pub(super) fn associate_text_tables<'a>(
    config: &TextRegionConfig,
    custom: &'a [CustomHuffmanTable<'a>],
) -> Result<TableBindings<'a>, SymbolTextError> {
    let Some(choices) = config.huffman_choices else {
        if custom.is_empty() {
            return Ok(TableBindings {
                entries: [None; MAX_TABLE_BINDINGS],
                len: 0,
            });
        }
        return Err(SymbolTextError::InvalidTableAssociation(
            "custom tables on arithmetic text region",
        ));
    };
    let specifications = [
        (TableParameter::TextFirstS, choices.first_s, false),
        (TableParameter::TextSubsequentS, choices.subsequent_s, true),
        (TableParameter::TextStripDelta, choices.strip_delta, false),
        (
            TableParameter::TextRefinementWidth,
            choices.refinement_width,
            false,
        ),
        (
            TableParameter::TextRefinementHeight,
            choices.refinement_height,
            false,
        ),
        (TableParameter::TextRefinementX, choices.refinement_x, false),
        (TableParameter::TextRefinementY, choices.refinement_y, false),
        (
            TableParameter::TextRefinementSize,
            choices.refinement_size,
            false,
        ),
    ];
    bind_tables(&specifications, custom)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(bindings) =>
            symbol_text_spec::table_bindings_valid(&bindings)
                && bindings.len == specifications@.len(),
        Err(_) => true,
    },
))]
pub(super) fn bind_tables<'a>(
    specifications: &[(TableParameter, TableChoice, bool)],
    custom: &'a [CustomHuffmanTable<'a>],
) -> Result<TableBindings<'a>, SymbolTextError> {
    if specifications.len() > MAX_TABLE_BINDINGS {
        return Err(SymbolTextError::InvalidTableAssociation(
            "too many table selectors",
        ));
    }
    let mut custom_count = 0_usize;
    let mut specification_index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            specification_index <= specifications@.len(),
            custom_count <= specification_index,
        decreases specifications@.len() - specification_index,
    ))]
    while specification_index < specifications.len() {
        if matches!(specifications[specification_index].1, TableChoice::Custom) {
            custom_count += 1;
        }
        specification_index += 1;
    }
    if custom_count != custom.len() {
        return Err(SymbolTextError::InvalidTableAssociation(
            "custom table count does not match selectors",
        ));
    }
    let mut bindings = TableBindings {
        entries: [None; MAX_TABLE_BINDINGS],
        len: 0,
    };
    let mut custom_index = 0_usize;
    let mut specification_index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            specification_index <= specifications@.len(),
            specifications@.len() <= MAX_TABLE_BINDINGS,
            bindings.len == specification_index,
            symbol_text_spec::table_bindings_valid(&bindings),
            custom_index <= custom@.len(),
        decreases specifications@.len() - specification_index,
    ))]
    while specification_index < specifications.len() {
        let parameter = specifications[specification_index].0;
        let choice = specifications[specification_index].1;
        let requires_oob = specifications[specification_index].2;
        let table =
            match choice {
                TableChoice::Standard(table) => BoundHuffmanTable::Standard(table),
                TableChoice::Custom => {
                    let supplied = custom.get(custom_index).ok_or(
                        SymbolTextError::InvalidTableAssociation("missing custom table"),
                    )?;
                    if supplied.has_oob != requires_oob {
                        return Err(SymbolTextError::InvalidTableAssociation(
                            "custom table OOB capability is invalid",
                        ));
                    }
                    custom_index = custom_index
                        .checked_add(1)
                        .ok_or(MalformedError::IntegerOverflow)?;
                    BoundHuffmanTable::Custom(supplied.table)
                }
            };
        bindings.entries[specification_index] = Some(TableBinding { parameter, table });
        bindings.len = specification_index + 1;
        specification_index += 1;
    }
    Ok(bindings)
}

impl StandardHuffmanTables {
    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub(super) const fn empty() -> Self {
        Self {
            a: None,
            b: None,
            c: None,
            d: None,
            e: None,
            f: None,
            g: None,
            h: None,
            i: None,
            j: None,
            k: None,
            l: None,
            m: None,
            n: None,
            o: None,
        }
    }

    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub(super) fn materialize(
        &mut self,
        table: StandardHuffmanTable,
        limits: &Limits,
    ) -> Result<(), HuffmanError> {
        if self.table(table).is_none() {
            let materialized = Some(Box::new(HuffmanTable::standard(table, limits)?));
            match table {
                StandardHuffmanTable::A => self.a = materialized,
                StandardHuffmanTable::B => self.b = materialized,
                StandardHuffmanTable::C => self.c = materialized,
                StandardHuffmanTable::D => self.d = materialized,
                StandardHuffmanTable::E => self.e = materialized,
                StandardHuffmanTable::F => self.f = materialized,
                StandardHuffmanTable::G => self.g = materialized,
                StandardHuffmanTable::H => self.h = materialized,
                StandardHuffmanTable::I => self.i = materialized,
                StandardHuffmanTable::J => self.j = materialized,
                StandardHuffmanTable::K => self.k = materialized,
                StandardHuffmanTable::L => self.l = materialized,
                StandardHuffmanTable::M => self.m = materialized,
                StandardHuffmanTable::N => self.n = materialized,
                StandardHuffmanTable::O => self.o = materialized,
            }
        }
        Ok(())
    }

    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub(super) fn table(&self, table: StandardHuffmanTable) -> Option<&HuffmanTable> {
        macro_rules! get {
            ($field:expr) => {
                match $field {
                    Some(table) => Some(table),
                    None => None,
                }
            };
        }
        match table {
            StandardHuffmanTable::A => get!(&self.a),
            StandardHuffmanTable::B => get!(&self.b),
            StandardHuffmanTable::C => get!(&self.c),
            StandardHuffmanTable::D => get!(&self.d),
            StandardHuffmanTable::E => get!(&self.e),
            StandardHuffmanTable::F => get!(&self.f),
            StandardHuffmanTable::G => get!(&self.g),
            StandardHuffmanTable::H => get!(&self.h),
            StandardHuffmanTable::I => get!(&self.i),
            StandardHuffmanTable::J => get!(&self.j),
            StandardHuffmanTable::K => get!(&self.k),
            StandardHuffmanTable::L => get!(&self.l),
            StandardHuffmanTable::M => get!(&self.m),
            StandardHuffmanTable::N => get!(&self.n),
            StandardHuffmanTable::O => get!(&self.o),
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_verify)]
pub(super) fn materialize_huffman_table<'a>(
    choice: Option<BoundHuffmanTable<'a>>,
    standard: &mut StandardHuffmanTables,
    limits: &Limits,
) -> Result<Option<MaterializedHuffmanTable<'a>>, SymbolTextError> {
    match choice {
        None => Ok(None),
        Some(BoundHuffmanTable::Standard(table)) => {
            standard.materialize(table, limits)?;
            Ok(Some(MaterializedHuffmanTable::Standard(table)))
        }
        Some(BoundHuffmanTable::Custom(table)) => Ok(Some(MaterializedHuffmanTable::Custom(table))),
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires symbol_text_spec::table_bindings_valid(bindings),
    ensures
        out.is_some() == exists|i: int|
            0 <= i < bindings.len
            && match #[trigger] bindings.entries[i] {
                Some(binding) => binding.parameter == parameter,
                None => false,
            },
))]
pub(super) fn find_table_binding<'a>(
    bindings: &TableBindings<'a>,
    parameter: TableParameter,
) -> Option<BoundHuffmanTable<'a>> {
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= bindings.len,
            bindings.len <= MAX_TABLE_BINDINGS,
            symbol_text_spec::table_bindings_valid(bindings),
            forall|i: int| 0 <= i < index
                ==> match #[trigger] bindings.entries[i] {
                    Some(binding) => binding.parameter != parameter,
                    None => true,
                },
        decreases bindings.len - index,
    ))]
    while index < bindings.len {
        #[cfg(verus_keep_ghost)]
        proof! {
            assert(symbol_text_spec::table_bindings_valid(bindings));
            reveal(symbol_text_spec::table_bindings_valid);
            assert(index < MAX_TABLE_BINDINGS);
            assert(bindings.entries[index as int].is_some() == (index < bindings.len));
            assert(bindings.entries[index as int].is_some());
        }
        let binding = bindings.entries[index]?;
        if same_table_parameter(binding.parameter, parameter) {
            return Some(binding.table);
        }
        index += 1;
    }
    None
}

impl HuffmanFieldChoices<'_> {
    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub(super) const fn empty() -> Self {
        Self {
            aggregation_instances: None,
            height_delta: None,
            width_delta: None,
            export_run: None,
            strip_delta: None,
            first_s: None,
            subsequent_s: None,
            refinement_width: None,
            refinement_height: None,
            refinement_x: None,
            refinement_y: None,
            refinement_size: None,
            dictionary_bitmap_size: None,
        }
    }
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires symbol_text_spec::table_bindings_valid(bindings),
    ensures match out {
        Ok(choices) =>
            config.huffman
                && choices.height_delta.is_some()
                && choices.width_delta.is_some()
                && choices.export_run.is_some()
                && choices.dictionary_bitmap_size.is_some()
                && (config.refinement_aggregation ==> (
                    choices.aggregation_instances.is_some()
                        && choices.first_s
                            == Some(BoundHuffmanTable::Standard(StandardHuffmanTable::F))
                        && choices.subsequent_s
                            == Some(BoundHuffmanTable::Standard(StandardHuffmanTable::H))
                        && choices.strip_delta
                            == Some(BoundHuffmanTable::Standard(StandardHuffmanTable::K))
                        && choices.refinement_width
                            == Some(BoundHuffmanTable::Standard(StandardHuffmanTable::O))
                        && choices.refinement_height
                            == Some(BoundHuffmanTable::Standard(StandardHuffmanTable::O))
                        && choices.refinement_x
                            == Some(BoundHuffmanTable::Standard(StandardHuffmanTable::O))
                        && choices.refinement_y
                            == Some(BoundHuffmanTable::Standard(StandardHuffmanTable::O))
                        && choices.refinement_size
                            == Some(BoundHuffmanTable::Standard(StandardHuffmanTable::A))
                )),
        Err(_) => true,
    },
))]
pub(super) fn select_dictionary_tables<'a>(
    config: &SymbolDictionaryConfig,
    bindings: &TableBindings<'a>,
) -> Result<HuffmanFieldChoices<'a>, SymbolTextError> {
    if !config.huffman {
        return Err(SymbolTextError::InvalidTableAssociation(
            "dictionary choices require Huffman mode",
        ));
    }
    let mut choices = HuffmanFieldChoices::empty();
    choices.height_delta = Some(
        find_table_binding(bindings, TableParameter::DictionaryHeight).ok_or(
            SymbolTextError::InvalidTableAssociation("missing height table"),
        )?,
    );
    choices.width_delta = Some(
        find_table_binding(bindings, TableParameter::DictionaryWidth).ok_or(
            SymbolTextError::InvalidTableAssociation("missing width table"),
        )?,
    );
    choices.dictionary_bitmap_size = Some(
        find_table_binding(bindings, TableParameter::DictionaryBitmapSize).ok_or(
            SymbolTextError::InvalidTableAssociation("missing bitmap-size table"),
        )?,
    );
    choices.export_run = Some(BoundHuffmanTable::Standard(StandardHuffmanTable::A));
    if config.refinement_aggregation {
        choices.aggregation_instances = Some(
            find_table_binding(bindings, TableParameter::DictionaryAggregationInstances).ok_or(
                SymbolTextError::InvalidTableAssociation("missing aggregation-instances table"),
            )?,
        );
        // T.88 Table 17 fixes the nested text decoder's tables.
        choices.first_s = Some(BoundHuffmanTable::Standard(StandardHuffmanTable::F));
        choices.subsequent_s = Some(BoundHuffmanTable::Standard(StandardHuffmanTable::H));
        choices.strip_delta = Some(BoundHuffmanTable::Standard(StandardHuffmanTable::K));
        choices.refinement_width = Some(BoundHuffmanTable::Standard(StandardHuffmanTable::O));
        choices.refinement_height = Some(BoundHuffmanTable::Standard(StandardHuffmanTable::O));
        choices.refinement_x = Some(BoundHuffmanTable::Standard(StandardHuffmanTable::O));
        choices.refinement_y = Some(BoundHuffmanTable::Standard(StandardHuffmanTable::O));
        choices.refinement_size = Some(BoundHuffmanTable::Standard(StandardHuffmanTable::A));
    }
    Ok(choices)
}

#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    requires symbol_text_spec::table_bindings_valid(bindings),
    ensures match out {
        Ok(choices) =>
            choices.first_s.is_some()
                && choices.subsequent_s.is_some()
                && choices.strip_delta.is_some()
                && (refinement ==> (
                    choices.refinement_width.is_some()
                        && choices.refinement_height.is_some()
                        && choices.refinement_x.is_some()
                        && choices.refinement_y.is_some()
                        && choices.refinement_size.is_some()
                )),
        Err(_) => true,
    },
))]
pub(super) fn select_text_tables<'a>(
    bindings: &TableBindings<'a>,
    refinement: bool,
) -> Result<HuffmanFieldChoices<'a>, SymbolTextError> {
    let mut choices = HuffmanFieldChoices::empty();
    choices.first_s = Some(
        find_table_binding(bindings, TableParameter::TextFirstS)
            .ok_or(SymbolTextError::InvalidTableAssociation("missing FS table"))?,
    );
    choices.subsequent_s = Some(
        find_table_binding(bindings, TableParameter::TextSubsequentS)
            .ok_or(SymbolTextError::InvalidTableAssociation("missing DS table"))?,
    );
    choices.strip_delta = Some(
        find_table_binding(bindings, TableParameter::TextStripDelta)
            .ok_or(SymbolTextError::InvalidTableAssociation("missing DT table"))?,
    );
    if refinement {
        choices.refinement_width = Some(
            find_table_binding(bindings, TableParameter::TextRefinementWidth).ok_or(
                SymbolTextError::InvalidTableAssociation("missing RDW table"),
            )?,
        );
        choices.refinement_height = Some(
            find_table_binding(bindings, TableParameter::TextRefinementHeight).ok_or(
                SymbolTextError::InvalidTableAssociation("missing RDH table"),
            )?,
        );
        choices.refinement_x = Some(
            find_table_binding(bindings, TableParameter::TextRefinementX).ok_or(
                SymbolTextError::InvalidTableAssociation("missing RDX table"),
            )?,
        );
        choices.refinement_y = Some(
            find_table_binding(bindings, TableParameter::TextRefinementY).ok_or(
                SymbolTextError::InvalidTableAssociation("missing RDY table"),
            )?,
        );
        choices.refinement_size = Some(
            find_table_binding(bindings, TableParameter::TextRefinementSize).ok_or(
                SymbolTextError::InvalidTableAssociation("missing RSIZE table"),
            )?,
        );
    }
    Ok(choices)
}

impl HuffmanFieldTables<'_> {
    #[cfg_attr(verus_keep_ghost, verus_verify)]
    pub(super) fn table_for(&self, field: IntegerField) -> Option<&HuffmanTable> {
        let resolved = match field {
            IntegerField::AggregationInstances => self.aggregation_instances.as_ref(),
            IntegerField::HeightDelta => self.height_delta.as_ref(),
            IntegerField::WidthDelta => self.width_delta.as_ref(),
            IntegerField::ExportRun => self.export_run.as_ref(),
            IntegerField::StripDelta => self.strip_delta.as_ref(),
            IntegerField::FirstS => self.first_s.as_ref(),
            IntegerField::SubsequentS => self.subsequent_s.as_ref(),
            IntegerField::RefinementWidth => self.refinement_width.as_ref(),
            IntegerField::RefinementHeight => self.refinement_height.as_ref(),
            IntegerField::RefinementX => self.refinement_x.as_ref(),
            IntegerField::RefinementY => self.refinement_y.as_ref(),
            IntegerField::RefinementSize => self.refinement_size.as_ref(),
            IntegerField::DictionaryBitmapSize => self.dictionary_bitmap_size.as_ref(),
            IntegerField::WithinStripT | IntegerField::RefinementIndicator => None,
        };
        match resolved {
            Some(MaterializedHuffmanTable::Standard(table)) => self.standard_tables.table(*table),
            Some(MaterializedHuffmanTable::Custom(table)) => Some(table),
            None => None,
        }
    }
}

/// Wraps caller-supplied custom Huffman tables, deriving each table's
/// out-of-band capability with [`HuffmanTable::has_oob`] rather than trusting
/// a caller-supplied flag.
#[cfg_attr(verus_keep_ghost, verus_spec(out =>
    ensures match out {
        Ok(wrapped) => wrapped@.len() == custom_tables@.len(),
        Err(_) => true,
    },
))]
pub(super) fn wrap_custom_tables<'a>(
    custom_tables: &'a [&'a HuffmanTable],
) -> Result<Vec<CustomHuffmanTable<'a>>, SymbolTextError> {
    let mut wrapped = Vec::new();
    let Some(requested_bytes) = custom_tables
        .len()
        .checked_mul(core::mem::size_of::<CustomHuffmanTable<'_>>())
    else {
        return Err(MalformedError::IntegerOverflow.into());
    };
    let Ok(requested_bytes) = u64::try_from(requested_bytes) else {
        return Err(MalformedError::IntegerOverflow.into());
    };
    if wrapped.try_reserve(custom_tables.len()).is_err() {
        return Err(DecodeError::Allocation { requested_bytes }.into());
    }
    let mut index = 0_usize;
    #[cfg_attr(verus_keep_ghost, verus_spec(
        invariant
            index <= custom_tables@.len(),
            wrapped@.len() == index,
        decreases custom_tables@.len() - index,
    ))]
    while index < custom_tables.len() {
        let table = custom_tables[index];
        wrapped.push(CustomHuffmanTable {
            table,
            has_oob: table.has_oob(),
        });
        index += 1;
    }
    Ok(wrapped)
}
