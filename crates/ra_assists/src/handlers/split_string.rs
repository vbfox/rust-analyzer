use ra_syntax::{
    ast::{self, HasStringValue},
    AstToken,
    SyntaxKind::{RAW_STRING, STRING},
    TextUnit,
};

use crate::{Assist, AssistCtx, AssistId};

const SPLIT_SEPARATOR: &str = "\" + \"";

pub(crate) fn split_string(ctx: AssistCtx) -> Option<Assist> {
    let token = ctx.find_covering_token_at_offset(STRING).and_then(ast::String::cast)?;
    let token_range = token.syntax().text_range();
    let selection = ctx.frange.range;

    let start_before_quote = token_range.start() == ctx.frange.range.start();
    if start_before_quote {
        return None;
    }
    let end_after_quote = token_range.end() == ctx.frange.range.end();
    if end_after_quote {
        return None;
    }

    ctx.add_assist(AssistId("split_string"), "Split string", |edit| {
        // TODO: Handle split on range
        // TODO: Handle no split on range out of string
        //let selection_length = ctx.frange.range.end() - ctx.frange.range.start();

        edit.target(token_range);
        edit.insert(selection.start(), SPLIT_SEPARATOR);
        if selection.start() != selection.end() {
            edit.insert(selection.end(), SPLIT_SEPARATOR);
            // Cursor is placed just before the second '+'
            let selection_length = selection.end() - selection.start();
            edit.set_cursor(selection.start() + TextUnit::from(7) + selection_length);
        } else {
            // Cursor is placed just before the '+'
            edit.set_cursor(selection.start() + TextUnit::from(2));
        }

    })
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::helpers::{check_assist, check_assist_not_applicable, check_assist_target};

    #[test]
    fn split_string_target() {
        check_assist_target(
            split_string,
            r#"
            fn f() {
                let s = "<|>random\nstring";
            }
            "#,
            r#""random\nstring""#,
        );
    }

    #[test]
    fn split_string_not_applicable_before() {
        check_assist_not_applicable(
            split_string,
            r#"
            fn f() {
                let s = <|>"random\nstring";
            }
            "#,
        );
    }

    #[test]
    fn split_string_not_applicable_after() {
        check_assist_not_applicable(
            split_string,
            r#"
            fn f() {
                let s = "random\nstring"<|>;
            }
            "#,
        );
    }

    #[test]
    fn split_string_works_simple_case() {
        check_assist(
            split_string,
            r#"
            fn f() {
                let s = "random<|>\nstring";
            }
            "#,
            r##"
            fn f() {
                let s = "random" <|>+ "\nstring";
            }
            "##,
        )
    }

    #[test]
    fn split_string_works_range_selected() {
        check_assist(
            split_string,
            r#"
            fn f() {
                let s = "random<|>\n<|>string";
            }
            "#,
            r##"
            fn f() {
                let s = "random" + "\n" <|>+ "string";
            }
            "##,
        )
    }
}