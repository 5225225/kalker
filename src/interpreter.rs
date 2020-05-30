use crate::ast::{compare_enums, Expr, Stmt, Unit};
use crate::lexer::TokenKind;
use crate::prelude;
use crate::symbol_table::SymbolTable;

pub struct Context<'a> {
    symbol_table: &'a mut SymbolTable,
    angle_unit: Unit,
}

impl<'a> Context<'a> {
    pub fn new(angle_unit: Unit, symbol_table: &'a mut SymbolTable) -> Self {
        Context {
            angle_unit: angle_unit.clone(),
            symbol_table,
        }
    }

    pub fn interpret(&mut self, statements: Vec<Stmt>) -> Result<Option<f64>, String> {
        for (i, stmt) in statements.iter().enumerate() {
            let value = eval_stmt(self, stmt);

            if i == statements.len() - 1 {
                if let Stmt::Expr(_) = stmt {
                    return Ok(Some(value?));
                }
            }
        }

        Ok(None)
    }
}

fn eval_stmt(context: &mut Context, stmt: &Stmt) -> Result<f64, String> {
    match stmt {
        Stmt::VarDecl(identifier, _) => eval_var_decl_stmt(context, stmt, identifier),
        Stmt::FnDecl(_, _, _) => eval_fn_decl_stmt(context),
        Stmt::Expr(expr) => eval_expr_stmt(context, &expr),
    }
}

fn eval_var_decl_stmt(context: &mut Context, stmt: &Stmt, identifier: &str) -> Result<f64, String> {
    context.symbol_table.insert(&identifier, stmt.clone());
    Ok(0f64)
}

fn eval_fn_decl_stmt(_: &mut Context) -> Result<f64, String> {
    Ok(0f64) // Nothing needs to happen here, since the parser will already have added the FnDecl's to the symbol table.
}

fn eval_expr_stmt(context: &mut Context, expr: &Expr) -> Result<f64, String> {
    eval_expr(context, &expr)
}

fn eval_expr(context: &mut Context, expr: &Expr) -> Result<f64, String> {
    match expr {
        Expr::Binary(left, op, right) => eval_binary_expr(context, &left, op, &right),
        Expr::Unary(_, expr) => eval_unary_expr(context, expr),
        Expr::Unit(expr, kind) => eval_unit_expr(context, expr, kind),
        Expr::Var(identifier) => eval_var_expr(context, identifier),
        Expr::Literal(value) => eval_literal_expr(context, value),
        Expr::Group(expr) => eval_group_expr(context, &expr),
        Expr::FnCall(identifier, expressions) => {
            eval_fn_call_expr(context, identifier, expressions)
        }
    }
}

fn eval_binary_expr(
    context: &mut Context,
    left: &Expr,
    op: &TokenKind,
    right: &Expr,
) -> Result<f64, String> {
    let left = eval_expr(context, &left)?;
    let right = eval_expr(context, &right)?;

    Ok(match op {
        TokenKind::Plus => left + right,
        TokenKind::Minus => left - right,
        TokenKind::Star => left * right,
        TokenKind::Slash => left / right,
        TokenKind::Power => left.powf(right),
        _ => 0f64,
    })
}

fn eval_unary_expr(context: &mut Context, expr: &Expr) -> Result<f64, String> {
    eval_expr(context, &expr).clone()
}

fn eval_unit_expr(context: &mut Context, expr: &Expr, kind: &TokenKind) -> Result<f64, String> {
    let x = eval_expr(context, &expr);
    let unit = kind.to_unit()?;

    // Don't do any angle conversions if the defauly angle unit is the same as the unit kind
    match unit {
        Unit::Degrees | Unit::Radians => {
            if compare_enums(&context.angle_unit, &unit) {
                return x;
            }
        }
    }

    match unit {
        Unit::Degrees => Ok(x?.to_radians()),
        Unit::Radians => Ok(x?.to_degrees()),
    }
}

fn eval_var_expr(context: &mut Context, identifier: &str) -> Result<f64, String> {
    // If there is a constant with this name, return a literal expression with its value
    if let Some(value) = prelude::CONSTANTS.get(identifier) {
        return eval_expr(context, &Expr::Literal(value.to_string()));
    }

    // Look for the variable in the symbol table
    let var_decl = context.symbol_table.get(identifier).cloned();
    match var_decl {
        Some(Stmt::VarDecl(_, expr)) => eval_expr(context, &expr),
        _ => Err(format!("Undefined variable: '{}'.", identifier)),
    }
}

fn eval_literal_expr(_: &mut Context, value: &str) -> Result<f64, String> {
    match value.parse() {
        Ok(parsed_value) => Ok(parsed_value),
        Err(_) => Err(format!("Invalid number literal: '{}'.", value)),
    }
}

fn eval_group_expr(context: &mut Context, expr: &Expr) -> Result<f64, String> {
    eval_expr(context, expr)
}

fn eval_fn_call_expr(
    context: &mut Context,
    identifier: &str,
    expressions: &Vec<Expr>,
) -> Result<f64, String> {
    let prelude_func = match expressions.len() {
        1 => {
            let x = eval_expr(context, &expressions[0])?;
            prelude::call_unary_func(identifier, x, &context.angle_unit)
        }
        2 => {
            let x = eval_expr(context, &expressions[0])?;
            let y = eval_expr(context, &expressions[1])?;
            prelude::call_binary_func(identifier, x, y, &context.angle_unit)
        }
        _ => None,
    };

    if let Some(result) = prelude_func {
        return Ok(result);
    }

    let stmt_definition = context
        .symbol_table
        .get(&format!("{}()", identifier))
        .cloned();

    match stmt_definition {
        Some(Stmt::FnDecl(_, arguments, fn_body)) => {
            if arguments.len() != expressions.len() {
                return Err(format!(
                    "Expected {} arguments in function '{}' but found {}.",
                    arguments.len(),
                    identifier,
                    expressions.len()
                ));
            }

            // Initialise the arguments as their own variables.
            for (i, argument) in arguments.iter().enumerate() {
                eval_stmt(
                    context,
                    &Stmt::VarDecl(argument.clone(), Box::new(expressions[i].clone())),
                )?;
            }

            return eval_expr(context, &*fn_body);
        }
        _ => Err(format!("Undefined function: '{}'.", identifier)),
    }
}
