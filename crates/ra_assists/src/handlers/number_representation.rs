use ra_syntax::{
    ast,
    ast::{HasQuotes, LiteralKind},
    AstToken,
    SyntaxKind::{LITERAL},
    TextUnit, AstNode,
    SmolStr
};

use crate::{Assist, AssistCtx, AssistId};

const CONCAT_MACRO: &str = "concat!(";
const SPLIT_SEPARATOR: &str = "\", \"";
const PLUS_OFFSET: usize = 2;

const V: u32 = 0b0010_1010;
const W: u32 = 0o52;
const X: u32 = 42;
const Y: u32 = 0x2A;
const Z: u8 = b'*';

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum NumberLiteralType {
    Decimal,
    PrefixHex,
    PrefixOctal,
    PrefixBinary,
}

#[derive(Clone, Debug)]
struct NumberLiteral {
    number_type: NumberLiteralType,
    suffix: Option<SmolStr>,
    prefix: Option<SmolStr>,
    text: SmolStr,
}

fn identify_number_literal(literal: &ast::Literal) -> Option<NumberLiteral> {
    match literal.kind() {
        LiteralKind::IntNumber { suffix } => {
            let token = literal.token();
            let full_text = token.text().as_str();
            let suffix_clone = suffix.clone();
            let suffix_len = suffix.map(|s| s.len()).unwrap_or_default();
            let non_suffix = &full_text[0..full_text.len() - suffix_len];
            let maybe_prefix = if non_suffix.len() < 2 { None } else { Some(&non_suffix[0..2]) };
            let (prefix, number_type) = match maybe_prefix {
                Some("0x") => (maybe_prefix, NumberLiteralType::PrefixHex),
                Some("0b") => (maybe_prefix, NumberLiteralType::PrefixBinary),
                Some("0o") => (maybe_prefix, NumberLiteralType::PrefixOctal),
                _ => (None, NumberLiteralType::Decimal),
            };
            let prefix_len = prefix.map(|s| s.len()).unwrap_or_default();
            let text = &non_suffix[prefix_len..];

            let result = NumberLiteral {
                number_type,
                suffix: suffix_clone,
                prefix: maybe_prefix.map(SmolStr::new),
                text: SmolStr::new(text),
            };
            Some(result)
        },
        _ => None
    }
}

fn is_int_number(literal: &ast::Literal) -> bool {
    match literal.kind() {
        LiteralKind::IntNumber {..} => true,
        _ => false
    }
}

pub(crate) fn remove_digit_separators(ctx: AssistCtx) -> Option<Assist> {
    let literal = ctx.find_covering_node_at_offset::<ast::Literal>()?;
    if !is_int_number(&literal) {
        return None
    }

    if !literal.syntax().text().contains_char('_') {
        return None
    }

    ctx.add_assist(AssistId("remove_digit_separators"), "Remove digit separators", |edit| {
        edit.target(literal.syntax().text_range());
        let new_text = literal.syntax().text().to_string().replace("_", "");
        edit.replace(literal.syntax().text_range(), new_text);
    })
}

fn separate_number(text: &str, every: usize) -> String {
    let mut result = String::with_capacity(text.len() + text.len() / every);
    let mut i = 0;
    for c in text.chars() {
        if c != '_' {
            result.push(c);
            if i % every == 0 {
                result.push('_');
            }
            i += 1;
        }
    }

    return result;
}

pub(crate) fn separate_number_literal(ctx: AssistCtx) -> Option<Assist> {
    let literal = ctx.find_covering_node_at_offset::<ast::Literal>()?;
    let number_literal = identify_number_literal(&literal)?;
    let separator
    if number_literal.number_type != NumberLiteralType::Decimal {
        return None
    }

    if number_literal.text.len() < 3 {
        return None
    }

    let result = separate_number(number_literal.text.as_str(), 3);
    if result == number_literal.text.as_str() {
        return None
    }

    ctx.add_assist(AssistId("remove_digit_separators"), "Remove digit separators", |edit| {
        edit.target(literal.syntax().text_range());
        let new_text = literal.syntax().text().to_string().replace("_", "");
        edit.replace(literal.syntax().text_range(), new_text);
    })
}

pub(crate) fn number_representation(ctx: AssistCtx) -> Option<Assist> {
    let token = ctx.find_covering_node_at_offset::<ast::Literal>()?;
    println!("LITERAL {:?}", token);
    println!("TEXT {:?}", token.syntax().text());
    println!("KIND {:?}", token.kind());
    match token.kind() {
        LiteralKind::IntNumber {..} => {},
        _ => {
            return None
        }
    }

    ctx.add_assist(AssistId("split_string"), "Split string", |edit| {
        edit.target(token.syntax().text_range());
    })
    /*
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
    */
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::helpers::{check_assist, check_assist_not_applicable, check_assist_target};

    #[test]
    fn remove_digit_separators_target() {
        check_assist_target(
            remove_digit_separators,
            r#"fn f() { let x = <|>42_420; }"#,
            r#"42_420"#,
        );
    }

    #[test]
    fn remove_digit_separators_not_applicable_no_separator() {
        check_assist_not_applicable(
            remove_digit_separators,
            r#"fn f() { let x = <|>42420; }"#,
        );
    }

    #[test]
    fn remove_digit_separators_works_decimal() {
        check_assist(
            remove_digit_separators,
            r#"fn f() { let x = <|>42_420; }"#,
            r#"fn f() { let x = <|>42420; }"#,
        )
    }

/*
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
    fn split_string_add_concat_inside_other_macro() {
        check_assist(
            split_string,
            r#"
            fn f() {
                let s = println!("random<|>\nstring");
            }
            "#,
            r##"
            fn f() {
                let s = println!(concat!("random",<|> "\nstring"));
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
    }*/
}