use lalrpop_util::lalrpop_mod;

pub mod evaluation;
pub mod parse_error;
pub mod tree;
lalrpop_mod!(pub grammar, "/language/grammar.rs");
