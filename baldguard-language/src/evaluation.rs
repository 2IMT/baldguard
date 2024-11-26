use super::tree::{Assignment, Expression, Literal, Operator};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, convert::From, fmt::Display, result::Result};

pub type SetFromAssignmentResult = Result<(), EvaluationError>;

pub trait SetFromAssignment {
    fn set_from_assignment(
        &mut self,
        assignment: &Assignment,
        variables: &Variables,
    ) -> SetFromAssignmentResult;
}

pub trait ContainsVariable {
    fn contains_variable(&self, identifier: &str) -> bool;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Value {
    Int(i64),
    Str(String),
    Bool(bool),
    Empty,
}

impl Value {
    pub fn type_str(&self) -> &'static str {
        match self {
            Value::Int(_) => "int",
            Value::Str(_) => "str",
            Value::Bool(_) => "bool",
            Value::Empty => "empty",
        }
    }
}

impl From<Literal> for Value {
    fn from(value: Literal) -> Self {
        match value {
            Literal::Int(value) => Value::Int(value),
            Literal::Str(value) => Value::Str(value),
            Literal::Bool(value) => Value::Bool(value),
            Literal::Empty => Value::Empty,
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Int(value) => write!(f, "{value}"),
            Value::Str(value) => write!(f, "{value}"),
            Value::Bool(value) => write!(f, "{}", if *value { "true" } else { "false" }),
            Value::Empty => write!(f, "empty"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ValueError {
    BinaryOp {
        left: Value,
        operator: &'static str,
        right: Value,
    },
    UnaryOp {
        value: Value,
        operator: &'static str,
    },
    DivisionByZero {
        value: Value,
    },
    InvalidRegex {
        regex: String,
        message: String,
    },
    Other {
        message: String,
    },
}

impl ValueError {
    pub fn new_binary(left: Value, operator: &'static str, right: Value) -> Self {
        ValueError::BinaryOp {
            left,
            operator,
            right,
        }
    }

    pub fn new_unary(value: Value, operator: &'static str) -> Self {
        ValueError::UnaryOp { value, operator }
    }

    pub fn new_division_by_zero(value: Value) -> Self {
        ValueError::DivisionByZero { value }
    }

    pub fn new_invalid_regex(regex: String, message: String) -> Self {
        ValueError::InvalidRegex { regex, message }
    }

    pub fn new_other(message: String) -> Self {
        ValueError::Other { message }
    }
}

impl Display for ValueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueError::BinaryOp {
                left,
                operator,
                right,
            } => write!(
                f,
                "unsupported operation {} {operator} {}",
                left.type_str(),
                right.type_str()
            ),
            ValueError::UnaryOp { value, operator } => {
                write!(f, "unsupported operation {operator} {}", value.type_str())
            }
            ValueError::DivisionByZero { value } => {
                write!(f, "division by zero ({value} / 0)")
            }
            ValueError::InvalidRegex { regex, message } => {
                write!(f, "invalid regex \"{regex}\": {message}")
            }
            ValueError::Other { message } => write!(f, "{message}"),
        }
    }
}

pub type ValueResult = Result<Value, ValueError>;

impl Value {
    pub fn not(&self) -> ValueResult {
        match self {
            Value::Bool(value) => Ok(Value::Bool(!value)),
            _ => Err(ValueError::new_unary(self.clone(), "not")),
        }
    }

    pub fn and(&self, other: &Self) -> ValueResult {
        match self {
            Value::Bool(l) => match other {
                Value::Bool(r) => Ok(Value::Bool(*l && *r)),
                _ => Err(ValueError::new_binary(self.clone(), "and", other.clone())),
            },
            _ => Err(ValueError::new_binary(self.clone(), "and", other.clone())),
        }
    }

    pub fn and_short_circuit(&self) -> Option<Value> {
        if let Value::Bool(l) = self {
            if !(*l) {
                return Some(Value::Bool(false));
            }
        }

        None
    }

    pub fn nand(&self, other: &Self) -> ValueResult {
        match self {
            Value::Bool(l) => match other {
                Value::Bool(r) => Ok(Value::Bool(!(*l && *r))),
                _ => Err(ValueError::new_binary(self.clone(), "nand", other.clone())),
            },
            _ => Err(ValueError::new_binary(self.clone(), "nand", other.clone())),
        }
    }

    pub fn nand_short_circuit(&self) -> Option<Value> {
        if let Value::Bool(l) = self {
            if !(*l) {
                return Some(Value::Bool(true));
            }
        }

        None
    }

    pub fn or(&self, other: &Self) -> ValueResult {
        match self {
            Value::Bool(l) => match other {
                Value::Bool(r) => Ok(Value::Bool(*l || *r)),
                _ => Err(ValueError::new_binary(self.clone(), "or", other.clone())),
            },
            _ => Err(ValueError::new_binary(self.clone(), "or", other.clone())),
        }
    }

    pub fn or_short_circuit(&self) -> Option<Value> {
        if let Value::Bool(l) = self {
            if *l {
                return Some(Value::Bool(true));
            }
        }

        None
    }

    pub fn nor(&self, other: &Self) -> ValueResult {
        match self {
            Value::Bool(l) => match other {
                Value::Bool(r) => Ok(Value::Bool(!(*l || *r))),
                _ => Err(ValueError::new_binary(self.clone(), "nor", other.clone())),
            },
            _ => Err(ValueError::new_binary(self.clone(), "nor", other.clone())),
        }
    }

    pub fn nor_short_circuit(&self) -> Option<Value> {
        if let Value::Bool(l) = self {
            if *l {
                return Some(Value::Bool(false));
            }
        }

        None
    }

    pub fn xor(&self, other: &Self) -> ValueResult {
        match self {
            Value::Bool(l) => match other {
                Value::Bool(r) => Ok(Value::Bool(*l ^ *r)),
                _ => Err(ValueError::new_binary(self.clone(), "xor", other.clone())),
            },
            _ => Err(ValueError::new_binary(self.clone(), "xor", other.clone())),
        }
    }

    pub fn equal(&self, other: &Self) -> ValueResult {
        match self {
            Value::Int(l) => match other {
                Value::Int(r) => Ok(Value::Bool(*l == *r)),
                Value::Empty => Ok(Value::Bool(false)),
                _ => Err(ValueError::new_binary(self.clone(), "=", other.clone())),
            },
            Value::Str(l) => match other {
                Value::Str(r) => Ok(Value::Bool(*l == *r)),
                Value::Empty => Ok(Value::Bool(false)),
                _ => Err(ValueError::new_binary(self.clone(), "=", other.clone())),
            },
            Value::Bool(l) => match other {
                Value::Bool(r) => Ok(Value::Bool(*l == *r)),
                Value::Empty => Ok(Value::Bool(false)),
                _ => Err(ValueError::new_binary(self.clone(), "=", other.clone())),
            },
            Value::Empty => match other {
                Value::Empty => Ok(Value::Bool(true)),
                _ => Ok(Value::Bool(false)),
            },
        }
    }

    pub fn not_equal(&self, other: &Self) -> ValueResult {
        match self {
            Value::Int(l) => match other {
                Value::Int(r) => Ok(Value::Bool(*l != *r)),
                Value::Empty => Ok(Value::Bool(true)),
                _ => Err(ValueError::new_binary(self.clone(), "!=", other.clone())),
            },
            Value::Str(l) => match other {
                Value::Str(r) => Ok(Value::Bool(*l != *r)),
                Value::Empty => Ok(Value::Bool(true)),
                _ => Err(ValueError::new_binary(self.clone(), "!=", other.clone())),
            },
            Value::Bool(l) => match other {
                Value::Bool(r) => Ok(Value::Bool(*l != *r)),
                Value::Empty => Ok(Value::Bool(true)),
                _ => Err(ValueError::new_binary(self.clone(), "!=", other.clone())),
            },
            Value::Empty => match other {
                Value::Empty => Ok(Value::Bool(false)),
                _ => Ok(Value::Bool(true)),
            },
        }
    }

    pub fn plus(&self, other: &Self) -> ValueResult {
        match self {
            Value::Int(l) => match other {
                Value::Int(r) => Ok(Value::Int(*l + *r)),
                _ => Err(ValueError::new_binary(self.clone(), "+", other.clone())),
            },
            Value::Str(l) => match other {
                Value::Str(r) => {
                    let mut result = l.to_owned();
                    result.push_str(r);
                    Ok(Value::Str(result))
                }
                _ => Err(ValueError::new_binary(self.clone(), "+", other.clone())),
            },
            _ => Err(ValueError::new_binary(self.clone(), "+", other.clone())),
        }
    }

    pub fn unary_plus(&self) -> ValueResult {
        match self {
            Value::Int(value) => Ok(Value::Int(*value)),
            _ => Err(ValueError::new_unary(self.clone(), "+")),
        }
    }

    pub fn minus(&self, other: &Self) -> ValueResult {
        match self {
            Value::Int(l) => match other {
                Value::Int(r) => Ok(Value::Int(*l - *r)),
                _ => Err(ValueError::new_binary(self.clone(), "-", other.clone())),
            },
            _ => Err(ValueError::new_binary(self.clone(), "-", other.clone())),
        }
    }

    pub fn unary_minus(&self) -> ValueResult {
        match self {
            Value::Int(value) => Ok(Value::Int(-(*value))),
            _ => Err(ValueError::new_unary(self.clone(), "-")),
        }
    }

    pub fn multiply(&self, other: &Self) -> ValueResult {
        match self {
            Value::Int(l) => match other {
                Value::Int(r) => Ok(Value::Int(*l * *r)),
                _ => Err(ValueError::new_binary(self.clone(), "*", other.clone())),
            },
            _ => Err(ValueError::new_binary(self.clone(), "*", other.clone())),
        }
    }

    pub fn divide(&self, other: &Self) -> ValueResult {
        match self {
            Value::Int(l) => match other {
                Value::Int(r) => {
                    if *r == 0 {
                        Err(ValueError::new_division_by_zero(self.clone()))
                    } else {
                        Ok(Value::Int(*l / *r))
                    }
                }
                _ => Err(ValueError::new_binary(self.clone(), "/", other.clone())),
            },
            _ => Err(ValueError::new_binary(self.clone(), "/", other.clone())),
        }
    }

    pub fn matches(&self, other: &Self) -> ValueResult {
        match self {
            Value::Str(l) => match other {
                Value::Str(r) => match Regex::new(r) {
                    Ok(regex) => Ok(Value::Bool(regex.is_match(l))),
                    Err(e) => Err(ValueError::new_invalid_regex(r.clone(), format!("{e}"))),
                },
                _ => Err(ValueError::new_binary(
                    self.clone(),
                    "matches",
                    other.clone(),
                )),
            },
            _ => Err(ValueError::new_binary(
                self.clone(),
                "matches",
                other.clone(),
            )),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Variables {
    values: HashMap<String, Value>,
}

pub trait ToVariables {
    fn to_variables(self) -> Variables;
}

impl Variables {
    pub fn new() -> Self {
        Variables {
            values: HashMap::new(),
        }
    }

    pub fn count(&self) -> usize {
        self.values.len()
    }

    pub fn put(&mut self, name: String, value: Value) {
        self.values.insert(name, value);
    }

    pub fn remove(&mut self, name: &str) -> bool {
        match self.values.remove(name) {
            Some(_) => true,
            None => false,
        }
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }

    pub fn extend(&mut self, other: Self) {
        self.values.extend(other.values);
    }

    pub fn show(&self, omit_empty: bool) -> String {
        let mut res = String::with_capacity(500);
        for (key, value) in &self.values {
            if omit_empty {
                if let Value::Empty = value {
                    continue;
                }
            }

            let variable = format!("{key} = {value}\n");
            res.push_str(&variable);
        }

        return res;
    }
}

impl Display for Variables {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.show(true))
    }
}

impl<T> From<T> for Variables
where
    T: ToVariables,
{
    fn from(value: T) -> Self {
        value.to_variables()
    }
}

impl SetFromAssignment for Variables {
    fn set_from_assignment(
        &mut self,
        assignment: &Assignment,
        variables: &Variables,
    ) -> SetFromAssignmentResult {
        match evaluate(&assignment.expression, variables) {
            Ok(value) => {
                self.put(assignment.identifier.clone(), value);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}

impl ContainsVariable for Variables {
    fn contains_variable(&self, identifier: &str) -> bool {
        self.values.contains_key(identifier)
    }
}

pub enum EvaluationError {
    UndeclaredIndentifier(String),
    ValueError(ValueError),
}

impl Display for EvaluationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvaluationError::UndeclaredIndentifier(i) => {
                write!(f, "undeclared identifier \"{i}\"")
            }
            EvaluationError::ValueError(e) => write!(f, "value error: {e}"),
        }
    }
}

impl From<ValueError> for EvaluationError {
    fn from(value: ValueError) -> Self {
        EvaluationError::ValueError(value)
    }
}

pub type EvaluationResult = Result<Value, EvaluationError>;

pub fn evaluate(e: &Expression, v: &Variables) -> EvaluationResult {
    match e {
        Expression::Identifier(identifier) => match v.get(&identifier) {
            Some(value) => Ok(value.clone()),
            None => Err(EvaluationError::UndeclaredIndentifier(identifier.clone())),
        },
        Expression::Literal(literal) => Ok(Value::from(literal.clone())),
        Expression::BinaryOp {
            left,
            operator,
            right,
        } => {
            let left = evaluate(left, v)?;

            match operator {
                Operator::And => match left.and_short_circuit() {
                    Some(value) => Ok(value),
                    None => Ok(left.and(&evaluate(right, v)?)?),
                },
                Operator::Nand => match left.nand_short_circuit() {
                    Some(value) => Ok(value),
                    None => Ok(left.nand(&evaluate(right, v)?)?),
                },
                Operator::Or => match left.or_short_circuit() {
                    Some(value) => Ok(value),
                    None => Ok(left.or(&evaluate(right, v)?)?),
                },
                Operator::Nor => match left.nor_short_circuit() {
                    Some(value) => Ok(value),
                    None => Ok(left.nor(&evaluate(right, v)?)?),
                },
                Operator::Xor => Ok(left.xor(&evaluate(right, v)?)?),
                Operator::Equal => Ok(left.equal(&evaluate(right, v)?)?),
                Operator::NotEqual => Ok(left.not_equal(&evaluate(right, v)?)?),
                Operator::Plus => Ok(left.plus(&evaluate(right, v)?)?),
                Operator::Minus => Ok(left.minus(&evaluate(right, v)?)?),
                Operator::Multiply => Ok(left.multiply(&evaluate(right, v)?)?),
                Operator::Divide => Ok(left.divide(&evaluate(right, v)?)?),
                Operator::Matches => Ok(left.matches(&evaluate(right, v)?)?),
                _ => panic!("invalid binary operation {:?}", operator),
            }
        }
        Expression::UnaryOp {
            expression,
            operator,
        } => {
            let value = evaluate(expression, v)?;

            match operator {
                Operator::Not => Ok(value.not()?),
                Operator::Plus => Ok(value.unary_plus()?),
                Operator::Minus => Ok(value.unary_minus()?),
                _ => panic!("invalid unary operation {:?}", operator),
            }
        }
    }
}
