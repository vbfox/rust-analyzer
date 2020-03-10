use ra_syntax::{
    ast,
    ast::{HasQuotes},
    AstToken,
    SyntaxKind::{STRING, MACRO_CALL},
    TextUnit, AstNode,
};

use crate::{Assist, AssistCtx, AssistId};
use ast::{NameOwner, make::name};

const CONCAT_MACRO: &str = "concat!(";
const SPLIT_SEPARATOR: &str = "\", \"";
const PLUS_OFFSET: usize = 2;

fn _f() {
    let _s: String = concat!("random", "\n", "string").into();
}

pub(crate) fn split_string(ctx: AssistCtx) -> Option<Assist> {
    let token = ctx.find_covering_token_at_offset(STRING).and_then(ast::String::cast)?;
    let between_quotes = token.text_range_between_quotes()?;
    let selection = ctx.frange.range;

    if !selection.is_subrange(&between_quotes) {
        return None
    }

    ctx.add_assist(AssistId("split_string"), "Split string", |edit| {
        let token_range = token.syntax().text_range();
        edit.target(token_range);

        let need_macro = {
            let ancestor = token.syntax().ancestors().nth(1);

            println!("{:?}", ancestor);
            match ancestor {
                None => true,
                Some(ancestor) => {
                    let as_macro = ast::MacroCall::cast(ancestor);
                    //println!("{:?}", as_macro);
                    if let Some(as_macro) = as_macro {
                        let macro_name = as_macro.path().map(|n| n.syntax().text().to_string()).unwrap_or_default();
                        println!("Found macro with name {:?}", macro_name);
                        macro_name != "concat"
                        /*
                        println!("{:?}", as_macro.path());
                        println!("{:?}", as_macro.token_tree());
                        let name = as_macro.name().map(|n| n.syntax().text().to_string()).unwrap_or_default();
                        println!("{:?}", as_macro.name());
                        println!("{:?}", name);
                        println!("{:?}", as_macro.path().map(|n| n.syntax().text().to_string()).unwrap_or_default());
                        println!("{:?}", as_macro.syntax().text());
                        */
                    } else {
                        true
                    }
                    //ancestor.kind() == MACRO_CALL
                }
            }
        };

        if need_macro {
            edit.insert(token_range.start(), CONCAT_MACRO);
        }

        edit.insert(selection.start(), SPLIT_SEPARATOR);

        if selection.start() != selection.end() {
            edit.insert(selection.end(), SPLIT_SEPARATOR);
        }

        // Cursor is placed before the last '+'
        let selection_end = edit.text_edit_builder().clone().finish().apply_to_offset(selection.end()).unwrap();
        edit.set_cursor(selection_end + TextUnit::from(PLUS_OFFSET as u32));

        if need_macro {
            edit.insert(token_range.end(), ")");
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
    fn split_string_not_applicable_starting_before() {
        check_assist_not_applicable(
            split_string,
            r#"
            fn f() {
                let s = <|>"random<|>\nstring";
            }
            "#,
        );
    }

    #[test]
    fn split_string_not_applicable_ending_after() {
        check_assist_not_applicable(
            split_string,
            r#"
            fn f() {
                let s = "random\n<|>string"<|>;
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
                let s = concat!("random",<|> "\nstring");
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
                let s = concat!("random", "\n",<|> "string");
            }
            "##,
        )
    }

    #[test]
    fn split_string_works_keep_existing_concat() {
        check_assist(
            split_string,
            r#"
            fn f() {
                let s: String = concat!("random<|>\n", "string").into();
            }
            "#,
            r##"
            fn f() {
                let s: String = concat!("random",<|> "\n", "string").into();
            }
            "##,
        )
    }
}