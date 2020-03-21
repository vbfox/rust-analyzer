use std::fmt;
use ra_syntax::{
    ast,
    ast::LiteralKind,
    AstNode,
    SmolStr
};

use crate::{Assist, AssistCtx, AssistId};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum NumberLiteralType {
    /// A literal without prefix, '42'
    Decimal,
    /// Hexadecimal literal, '0x2A'
    PrefixHex,
    /// Octal literal, '0o52'
    PrefixOctal,
    /// Binary literal, '0b00101010'
    PrefixBinary,
}

#[derive(Clone, Debug)]
struct NumberLiteral {
    /// The type of literal (no prefix, hex, octal or binary)
    number_type: NumberLiteralType,
    /// The suffix as a string, for example 'i32'
    suffix: Option<SmolStr>,
    /// The prefix as string, for example '0x'
    prefix: Option<SmolStr>,
    /// Text of the literal
    text: SmolStr,
}

impl fmt::Display for NumberLiteral {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(prefix) = &self.prefix {
            f.write_str(prefix)?;
        }

        f.write_str(&self.text)?;

        if let Some(suffix) = &self.suffix {
            f.write_str(suffix)?;
        }

        Ok(())
    }
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
                prefix: prefix.map(SmolStr::new),
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

fn remove_separator_from_string(str: &str) -> String {
    str.replace("_", "")
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
        let new_text = remove_separator_from_string(&literal.syntax().text().to_string());
        edit.replace(literal.syntax().text_range(), new_text);
    })
}

fn len_without_separators(text: &str) -> usize {
    let mut len = 0;
    for c in text.chars() {
        if c != '_' {
            len += 1;
        }
    }
    return len;
}

fn separate_number(text: &str, every: usize, digits_len: usize) -> String {
    let mut result = String::with_capacity(digits_len + digits_len / every);
    let offset = every - (digits_len % every);
    let mut i = 0;
    for c in text.chars() {
        if c != '_' {
            if (i != 0) && ((i + offset) % every == 0) {
                result.push('_');
            }
            result.push(c);
            i += 1;
        }
    }

    return result;
}

#[derive(Clone, Debug)]
struct SeparateNumberDetails {
    id: AssistId,
    label: String,
    every: usize
}

fn get_separate_number_details(literal: &NumberLiteral) -> Option<SeparateNumberDetails> {
    match literal.number_type {
        NumberLiteralType::Decimal => {
            Some(SeparateNumberDetails {
                id: AssistId("separate_decimal_thousands"),
                label: "Separate thousands".to_string(),
                every: 3,
            })
        },
        NumberLiteralType::PrefixHex => {
            Some(SeparateNumberDetails {
                id: AssistId("separate_hexadecimal_word"),
                label: "Separate 16-bits words".to_string(),
                every: 4,
            })
        },
        NumberLiteralType::PrefixBinary => {
            Some(SeparateNumberDetails {
                id: AssistId("separate_binary_bytes"),
                label: "Separate bytes".to_string(),
                every: 8,
            })
        },
        _ => None
    }
}

pub(crate) fn separate_number_literal(ctx: AssistCtx) -> Option<Assist> {
    let literal = ctx.find_covering_node_at_offset::<ast::Literal>()?;
    let number_literal = identify_number_literal(&literal)?;
    let details = get_separate_number_details(&number_literal)?;

    let digits_len = len_without_separators(number_literal.text.as_str());
    if digits_len <= details.every {
        return None
    }

    let result = separate_number(number_literal.text.as_str(), details.every, digits_len);
    if result == number_literal.text.as_str() {
        return None
    }

    ctx.add_assist(details.id, details.label, |edit| {
        edit.target(literal.syntax().text_range());
        let new_literal = NumberLiteral { text: SmolStr::new(result), ..number_literal };
        let new_text = new_literal.to_string();
        edit.replace(literal.syntax().text_range(), new_text);
    })
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
    fn remove_digit_separators_target_range_inside() {
        check_assist_target(
            remove_digit_separators,
            r#"fn f() { let x = 42<|>_<|>420; }"#,
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
    fn remove_digit_separators_not_applicable_range_ends_after() {
        check_assist_not_applicable(
            remove_digit_separators,
            r#"fn f() { let x = <|>42_420; <|>}"#,
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

    #[test]
    fn remove_digit_separators_works_hex() {
        check_assist(
            remove_digit_separators,
            r#"fn f() { let x = <|>0x42_420; }"#,
            r#"fn f() { let x = <|>0x42420; }"#,
        )
    }

    #[test]
    fn remove_digit_separators_works_octal() {
        check_assist(
            remove_digit_separators,
            r#"fn f() { let x = <|>0o42_420; }"#,
            r#"fn f() { let x = <|>0o42420; }"#,
        )
    }

    #[test]
    fn remove_digit_separators_works_binary() {
        check_assist(
            remove_digit_separators,
            r#"fn f() { let x = <|>0b0010_1010; }"#,
            r#"fn f() { let x = <|>0b00101010; }"#,
        )
    }

    #[test]
    fn remove_digit_separators_works_suffix() {
        check_assist(
            remove_digit_separators,
            r#"fn f() { let x = <|>42_420u32; }"#,
            r#"fn f() { let x = <|>42420u32; }"#,
        )
    }

    // ---

    fn separate_number_for_test(text: &str, every: usize) -> String {
        separate_number(text, every, len_without_separators(text))
    }

    #[test]
    fn test_separate_number() {
        assert_eq!(separate_number_for_test("", 2), "");
        assert_eq!(separate_number_for_test("1", 2), "1");
        assert_eq!(separate_number_for_test("12", 2), "12");
        assert_eq!(separate_number_for_test("12345678", 2), "12_34_56_78");
        assert_eq!(separate_number_for_test("123456789", 2), "1_23_45_67_89");
        assert_eq!(separate_number_for_test("1_2_3_4_5_6_7_8_9", 2), "1_23_45_67_89");

        assert_eq!(separate_number_for_test("", 4), "");
        assert_eq!(separate_number_for_test("1", 4), "1");
        assert_eq!(separate_number_for_test("1212", 4), "1212");
        assert_eq!(separate_number_for_test("24204242420", 4), "242_0424_2420");
        assert_eq!(separate_number_for_test("024204242420", 4), "0242_0424_2420");
        assert_eq!(separate_number_for_test("_0_2_4_2_04242_420", 4), "0242_0424_2420");

    }

    // ---

    #[test]
    fn separate_number_literal_decimal_target() {
        check_assist_target(
            separate_number_literal,
            r#"fn f() { let x = <|>42420; }"#,
            r#"42420"#,
        );
    }

    #[test]
    fn separate_number_literal_decimal_already_split_not_applicable() {
        check_assist_not_applicable(
            separate_number_literal,
            r#"fn f() { let x = <|>42_420;}"#,
        );
    }

    #[test]
    fn separate_number_literal_decimal_too_small_not_applicable() {
        check_assist_not_applicable(
            separate_number_literal,
            r#"fn f() { let x = <|>420;}"#,
        );
    }

    #[test]
    fn separate_number_literal_decimal_too_small_separator_not_applicable() {
        check_assist_not_applicable(
            separate_number_literal,
            r#"fn f() { let x = <|>4_2_0;}"#,
        );
    }

    #[test]
    fn separate_number_literal_decimal() {
        check_assist(
            separate_number_literal,
            r#"fn f() { let x = <|>2420420; }"#,
            r#"fn f() { let x = <|>2_420_420; }"#,
        )
    }

    #[test]
    fn separate_number_literal_decimal_badly_split() {
        check_assist(
            separate_number_literal,
            r#"fn f() { let x = <|>4_2_4_2_0420; }"#,
            r#"fn f() { let x = <|>42_420_420; }"#,
        )
    }

    // ---

    #[test]
    fn separate_number_literal_hex_target() {
        check_assist_target(
            separate_number_literal,
            r#"fn f() { let x = <|>0x04242420; }"#,
            r#"0x04242420"#,
        );
    }

    #[test]
    fn separate_number_literal_hex_already_split_not_applicable() {
        check_assist_not_applicable(
            separate_number_literal,
            r#"fn f() { let x = <|>0x0424_2420; <|>}"#,
        );
    }

    #[test]
    fn separate_number_literal_hex_too_small_not_applicable() {
        check_assist_not_applicable(
            separate_number_literal,
            r#"fn f() { let x = <|>0x2420;}"#,
        );
    }

    #[test]
    fn separate_number_literal_hex_too_small_separator_not_applicable() {
        check_assist_not_applicable(
            separate_number_literal,
            r#"fn f() { let x = <|>0x2_4_2_0;}"#,
        );
    }

    #[test]
    fn separate_number_literal_hex() {
        check_assist(
            separate_number_literal,
            r#"fn f() { let x = <|>0x24204242420; }"#,
            r#"fn f() { let x = <|>0x242_0424_2420; }"#,
        )
    }

    #[test]
    fn separate_number_literal_hex_badly_split() {
        check_assist(
            separate_number_literal,
            r#"fn f() { let x = <|>0x2_4204_24_2420; }"#,
            r#"fn f() { let x = <|>0x242_0424_2420; }"#,
        )
    }

    // ---

    #[test]
    fn separate_number_literal_octal_not_applicable() {
        check_assist_not_applicable(
            separate_number_literal,
            r#"fn f() { let x = <|>0o01234567; }"#,
        );
    }

    // ---

    #[test]
    fn separate_number_literal_binary_target() {
        check_assist_target(
            separate_number_literal,
            r#"fn f() { let x = <|>0b0010101000101010; }"#,
            r#"0b0010101000101010"#,
        );
    }

    #[test]
    fn separate_number_literal_binary_already_split_not_applicable() {
        check_assist_not_applicable(
            separate_number_literal,
            r#"fn f() { let x = <|>0b00101010_00101010; <|>}"#,
        );
    }

    #[test]
    fn separate_number_literal_binary_too_small_not_applicable() {
        check_assist_not_applicable(
            separate_number_literal,
            r#"fn f() { let x = <|>0b00101010;}"#,
        );
    }

    #[test]
    fn separate_number_literal_binary_too_small_separator_not_applicable() {
        check_assist_not_applicable(
            separate_number_literal,
            r#"fn f() { let x = <|>0b0_0_101_01_0;}"#,
        );
    }

    #[test]
    fn separate_number_literal_binary() {
        check_assist(
            separate_number_literal,
            r#"fn f() { let x = <|>0b0010101000101010; }"#,
            r#"fn f() { let x = <|>0b00101010_00101010; }"#,
        )
    }

    #[test]
    fn separate_number_literal_binary_badly_split() {
        check_assist(
            separate_number_literal,
            r#"fn f() { let x = <|>0b001_0101_000_101_010; }"#,
            r#"fn f() { let x = <|>0b00101010_00101010; }"#,
        )
    }
}