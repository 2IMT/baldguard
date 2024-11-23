use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Operator {
    Not,
    And,
    Nand,
    Or,
    Nor,
    Xor,
    Equal,
    NotEqual,
    Plus,
    Minus,
    Multiply,
    Divide,
    Matches,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Literal {
    Int(i64),
    Str(String),
    Bool(bool),
    Empty,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Expression {
    Identifier(String),
    Literal(Literal),
    BinaryOp {
        left: Box<Expression>,
        operator: Operator,
        right: Box<Expression>,
    },
    UnaryOp {
        expression: Box<Expression>,
        operator: Operator,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Assignment {
    pub identifier: String,
    pub expression: Expression,
}
