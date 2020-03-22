//! `ra_assists` crate provides a bunch of code assists, also known as code
//! actions (in LSP) or intentions (in IntelliJ).
//!
//! An assist is a micro-refactoring, which is automatically activated in
//! certain context. For example, if the cursor is over `,`, a "swap `,`" assist
//! becomes available.

mod assist_ctx;
mod marks;
#[cfg(test)]
mod doc_tests;
pub mod utils;
pub mod ast_transform;

use ra_db::FileRange;
use ra_ide_db::RootDatabase;
use ra_syntax::{TextRange, TextUnit};
use ra_text_edit::TextEdit;

pub(crate) use crate::assist_ctx::{Assist, AssistCtx, AssistHandler};
use hir::Semantics;

/// Unique identifier of the assist, should not be shown to the user
/// directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssistId(pub &'static str);

#[derive(Debug, Clone)]
pub struct AssistLabel {
    /// Short description of the assist, as shown in the UI.
    pub label: String,
    pub id: AssistId,
}

#[derive(Clone, Debug)]
pub struct GroupLabel(pub String);

impl AssistLabel {
    pub(crate) fn new(label: String, id: AssistId) -> AssistLabel {
        // FIXME: make fields private, so that this invariant can't be broken
        assert!(label.starts_with(|c: char| c.is_uppercase()));
        AssistLabel { label, id }
    }
}

#[derive(Debug, Clone)]
pub struct AssistAction {
    pub edit: TextEdit,
    pub cursor_position: Option<TextUnit>,
    // FIXME: This belongs to `AssistLabel`
    pub target: Option<TextRange>,
}

#[derive(Debug, Clone)]
pub struct ResolvedAssist {
    pub label: AssistLabel,
    pub group_label: Option<GroupLabel>,
    pub action: AssistAction,
}

/// Return all the assists applicable at the given position.
///
/// Assists are returned in the "unresolved" state, that is only labels are
/// returned, without actual edits.
pub fn unresolved_assists(db: &RootDatabase, range: FileRange) -> Vec<AssistLabel> {
    let sema = Semantics::new(db);
    let ctx = AssistCtx::new(&sema, range, false);
    handlers::all()
        .iter()
        .filter_map(|f| f(ctx.clone()))
        .flat_map(|it| it.0)
        .map(|a| a.label)
        .collect()
}

/// Return all the assists applicable at the given position.
///
/// Assists are returned in the "resolved" state, that is with edit fully
/// computed.
pub fn resolved_assists(db: &RootDatabase, range: FileRange) -> Vec<ResolvedAssist> {
    let sema = Semantics::new(db);
    let ctx = AssistCtx::new(&sema, range, true);
    let mut a = handlers::all()
        .iter()
        .filter_map(|f| f(ctx.clone()))
        .flat_map(|it| it.0)
        .map(|it| it.into_resolved().unwrap())
        .collect::<Vec<_>>();
    a.sort_by_key(|it| it.action.target.map_or(TextUnit::from(!0u32), |it| it.len()));
    a
}

mod handlers {
    use crate::AssistHandler;

    mod add_derive;
    mod add_explicit_type;
    mod add_impl;
    mod add_custom_impl;
    mod add_new;
    mod apply_demorgan;
    mod auto_import;
    mod invert_if;
    mod flip_comma;
    mod flip_binexpr;
    mod flip_trait_bound;
    mod change_visibility;
    mod fill_match_arms;
    mod merge_match_arms;
    mod introduce_variable;
    mod inline_local_variable;
    mod raw_string;
    mod split_string;
    mod remove_mut;
    mod replace_if_let_with_match;
    mod split_import;
    mod remove_dbg;
    pub(crate) mod replace_qualified_name_with_use;
    mod add_missing_impl_members;
    mod move_guard;
    mod move_bounds;
    mod early_return;
    mod number_representation;

    pub(crate) fn all() -> &'static [AssistHandler] {
        &[
            add_derive::add_derive,
            add_explicit_type::add_explicit_type,
            add_impl::add_impl,
            add_custom_impl::add_custom_impl,
            add_new::add_new,
            apply_demorgan::apply_demorgan,
            invert_if::invert_if,
            change_visibility::change_visibility,
            fill_match_arms::fill_match_arms,
            merge_match_arms::merge_match_arms,
            flip_comma::flip_comma,
            flip_binexpr::flip_binexpr,
            flip_trait_bound::flip_trait_bound,
            introduce_variable::introduce_variable,
            replace_if_let_with_match::replace_if_let_with_match,
            split_import::split_import,
            remove_dbg::remove_dbg,
            replace_qualified_name_with_use::replace_qualified_name_with_use,
            add_missing_impl_members::add_missing_impl_members,
            add_missing_impl_members::add_missing_default_members,
            inline_local_variable::inline_local_variable,
            move_guard::move_guard_to_arm_body,
            move_guard::move_arm_cond_to_match_guard,
            move_bounds::move_bounds_to_where_clause,
            raw_string::add_hash,
            raw_string::make_raw_string,
            split_string::split_string,
            raw_string::make_usual_string,
            raw_string::remove_hash,
            remove_mut::remove_mut,
            early_return::convert_to_guarded_return,
            auto_import::auto_import,
            number_representation::remove_digit_separators,
            number_representation::separate_number_literal,
        ]
    }
}

#[cfg(test)]
mod helpers {
    use crate::AssistId;
use std::sync::Arc;

    use ra_db::{fixture::WithFixture, FileId, FileRange, SourceDatabaseExt};
    use ra_ide_db::{symbol_index::SymbolsDatabase, RootDatabase};
    use ra_syntax::TextRange;
    use test_utils::{add_cursor, assert_eq_text, extract_range_or_offset, RangeOrOffset};

    use crate::{AssistCtx, AssistHandler, assist_ctx::AssistInfo};
    use hir::Semantics;

    pub(crate) fn with_single_file(text: &str) -> (RootDatabase, FileId) {
        let (mut db, file_id) = RootDatabase::with_single_file(text);
        // FIXME: ideally, this should be done by the above `RootDatabase::with_single_file`,
        // but it looks like this might need specialization? :(
        let local_roots = vec![db.file_source_root(file_id)];
        db.set_local_roots(Arc::new(local_roots));
        (db, file_id)
    }

    pub(crate) fn check_assist(
        assist: AssistHandler,
        ra_fixture_before: &str,
        ra_fixture_after: &str,
    ) {
        check(assist, None, ra_fixture_before, ExpectedResult::After(ra_fixture_after));
    }

    pub(crate) fn check_assist_with_id(
        assist: AssistHandler,
        assist_id: AssistId,
        ra_fixture_before: &str,
        ra_fixture_after: &str,
    ) {
        check(assist, Some(assist_id), ra_fixture_before, ExpectedResult::After(ra_fixture_after));
    }

    // FIXME: instead of having a separate function here, maybe use
    // `extract_ranges` and mark the target as `<target> </target>` in the
    // fixuture?
    pub(crate) fn check_assist_target(assist: AssistHandler, ra_fixture: &str, target: &str) {
        check(assist, None, ra_fixture, ExpectedResult::Target(target));
    }

    pub(crate) fn check_assist_target_with_id(assist: AssistHandler, assist_id: AssistId, ra_fixture: &str, target: &str) {
        check(assist, Some(assist_id), ra_fixture, ExpectedResult::Target(target));
    }

    pub(crate) fn check_assist_not_applicable(assist: AssistHandler, ra_fixture: &str) {
        check(assist, None, ra_fixture, ExpectedResult::NotApplicable);
    }

    pub(crate) fn check_assist_not_applicable_with_id(assist: AssistHandler, assist_id: AssistId, ra_fixture: &str) {
        check(assist, Some(assist_id), ra_fixture, ExpectedResult::NotApplicable);
    }

    enum ExpectedResult<'a> {
        NotApplicable,
        After(&'a str),
        Target(&'a str),
    }

    fn check(assist_handler: AssistHandler, assist_id: Option<AssistId>, before: &str, expected: ExpectedResult) {
        let (range_or_offset, before) = extract_range_or_offset(before);
        let range: TextRange = range_or_offset.into();

        let (db, file_id) = with_single_file(&before);
        let frange = FileRange { file_id, range };
        let sema = Semantics::new(&db);
        let assist_ctx = AssistCtx::new(&sema, frange, true);

        let assist_result = assist_handler(assist_ctx);
        let assist: Option<AssistInfo> = assist_result.clone().and_then(|assist| {
            match assist_id {
                None => Some(assist.0[0].clone()),
                Some(assist_id) => {
                    assist.0.iter().find(|info| info.label.id == assist_id).map(AssistInfo::to_owned)
                }
            }
        });

        match (assist, expected) {
            (Some(assist), ExpectedResult::After(after)) => {
                let action = assist.action.clone().unwrap();

                let mut actual = action.edit.apply(&before);
                match action.cursor_position {
                    None => {
                        if let RangeOrOffset::Offset(before_cursor_pos) = range_or_offset {
                            let off = action
                                .edit
                                .apply_to_offset(before_cursor_pos)
                                .expect("cursor position is affected by the edit");
                            actual = add_cursor(&actual, off)
                        }
                    }
                    Some(off) => actual = add_cursor(&actual, off),
                };

                assert_eq_text!(after, &actual);
            }
            (Some(assist), ExpectedResult::Target(target)) => {
                let action = assist.action.clone().unwrap();
                let range = action.target.expect("expected target on action");
                assert_eq_text!(&before[range.start().to_usize()..range.end().to_usize()], target);
            }
            (Some(_), ExpectedResult::NotApplicable) => panic!("assist should not be applicable!"),
            (None, ExpectedResult::After(_)) | (None, ExpectedResult::Target(_)) => {
                match assist_id {
                    None => panic!("No code action is applicable"),
                    Some(assist_id) => {
                        let applicable_actions: Vec<String> = assist_result.map(|r| r.0.iter().map(|i| i.label.id.0.to_string()).collect()).unwrap_or_default();
                        panic!("Code action '{}' is not applicable. Applicables actions: [{}]", assist_id.0, applicable_actions.join(", "));
                    }
                }
            }
            (None, ExpectedResult::NotApplicable) => (),
        };
    }
}

#[cfg(test)]
mod tests {
    use ra_db::FileRange;
    use ra_syntax::TextRange;
    use test_utils::{extract_offset, extract_range};

    use crate::{helpers, resolved_assists};

    #[test]
    fn assist_order_field_struct() {
        let before = "struct Foo { <|>bar: u32 }";
        let (before_cursor_pos, before) = extract_offset(before);
        let (db, file_id) = helpers::with_single_file(&before);
        let frange =
            FileRange { file_id, range: TextRange::offset_len(before_cursor_pos, 0.into()) };
        let assists = resolved_assists(&db, frange);
        let mut assists = assists.iter();

        assert_eq!(
            assists.next().expect("expected assist").label.label,
            "Change visibility to pub(crate)"
        );
        assert_eq!(assists.next().expect("expected assist").label.label, "Add `#[derive]`");
    }

    #[test]
    fn assist_order_if_expr() {
        let before = "
        pub fn test_some_range(a: int) -> bool {
            if let 2..6 = <|>5<|> {
                true
            } else {
                false
            }
        }";
        let (range, before) = extract_range(before);
        let (db, file_id) = helpers::with_single_file(&before);
        let frange = FileRange { file_id, range };
        let assists = resolved_assists(&db, frange);
        let mut assists = assists.iter();

        assert_eq!(assists.next().expect("expected assist").label.label, "Extract into variable");
        assert_eq!(assists.next().expect("expected assist").label.label, "Replace with match");
    }
}
