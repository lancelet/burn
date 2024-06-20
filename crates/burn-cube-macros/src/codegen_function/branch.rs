use proc_macro2::TokenStream;

use crate::{codegen_function::expr::codegen_expr, tracker::VariableTracker};

use super::{
    base::{codegen_block, Codegen},
    function::codegen_call,
    operation::codegen_binary,
    variable::{codegen_lit, codegen_path_var},
};

/// Codegen of for loops
/// Supports range:
/// for i in range(start, end, unroll) {...}
pub(crate) fn codegen_for_loop(
    for_loop: &syn::ExprForLoop,
    loop_level: usize,
    variable_tracker: &mut VariableTracker,
) -> TokenStream {
    let i = &for_loop.pat;

    if let syn::Pat::Ident(pat_ident) = &*for_loop.pat {
        let id = &pat_ident.ident;
        variable_tracker.codegen_declare(id.to_string(), loop_level as u8 + 1);
    }

    let invalid_for_loop = || {
        syn::Error::new_spanned(
            &for_loop.expr,
            "Invalid for loop: use [range](cubecl::prelude::range] instead.",
        )
        .into_compile_error()
    };

    match for_loop.expr.as_ref() {
        syn::Expr::Call(call) => {
            let func_name = match call.func.as_ref() {
                syn::Expr::Path(path) => match path.path.get_ident() {
                    Some(ident) => ident,
                    None => return invalid_for_loop(),
                },
                _ => {
                    return invalid_for_loop();
                }
            };

            if &func_name.to_string() == "range" {
                let mut args = call.args.clone();

                let unroll = codegen_expr(
                    &args.pop().unwrap().into_value(),
                    loop_level,
                    variable_tracker,
                );
                let end = codegen_expr(
                    &args.pop().unwrap().into_value(),
                    loop_level,
                    variable_tracker,
                );
                let start = codegen_expr(
                    &args.pop().unwrap().into_value(),
                    loop_level,
                    variable_tracker,
                );

                let block = codegen_block(&for_loop.body, loop_level + 1, variable_tracker);

                quote::quote! {
                    {
                        let _start = #start;
                        let _end = #end;
                        let _unroll = #unroll;
                        burn_cube::frontend::branch::range_expand(context, _start, _end, _unroll, |context, #i| #block);
                    }
                }
            } else {
                invalid_for_loop()
            }
        }
        _ => invalid_for_loop(),
    }
}

/// Codegen for condition of an if or a while
pub(crate) fn codegen_cond(
    cond: &syn::Expr,
    loop_level: usize,
    variable_tracker: &mut VariableTracker,
) -> Codegen {
    match cond {
        syn::Expr::Binary(expr) => codegen_binary(expr, loop_level, variable_tracker),
        syn::Expr::Lit(expr) => Codegen::new(codegen_lit(expr), false),
        syn::Expr::Path(expr) => codegen_path_var(expr, loop_level, variable_tracker),
        syn::Expr::Call(expr) => codegen_call(expr, loop_level, variable_tracker),
        _ => todo!("{cond:?} cond not supported"),
    }
}

/// Codegen for break statement
pub(crate) fn codegen_break() -> TokenStream {
    quote::quote! {
        burn_cube::frontend::branch::break_expand(context);
    }
}

/// Codegen for return statement
pub(crate) fn codegen_return(expr_return: &syn::ExprReturn) -> TokenStream {
    if expr_return.expr.is_some() {
        return syn::Error::new_spanned(expr_return, "Only void return is supported.")
            .into_compile_error();
    }

    quote::quote! {
        burn_cube::frontend::branch::return_expand(context);
    }
}

/// Codegen for if and if/else statements
/// Supports:
/// if cond {...}
/// if cond {...} else {...}
/// if Comptime::get(...) {...} [else {...}]
pub(crate) fn codegen_if(
    expr_if: &syn::ExprIf,
    loop_level: usize,
    variable_tracker: &mut VariableTracker,
) -> TokenStream {
    let (cond, is_comptime) = codegen_cond(&expr_if.cond, loop_level, variable_tracker).split();
    let comptime_bool = if is_comptime {
        quote::quote! { Some(#cond) }
    } else {
        quote::quote! { None }
    };

    let then_block = codegen_block(&expr_if.then_branch, loop_level + 1, variable_tracker);

    if let Some((_, expr)) = &expr_if.else_branch {
        if let syn::Expr::Block(expr_block) = &**expr {
            let else_block = codegen_block(&expr_block.block, loop_level + 1, variable_tracker);

            quote::quote! {
                let _cond = #cond;
                burn_cube::frontend::branch::if_else_expand(context, #comptime_bool, _cond.into(), |context| #then_block, |context| #else_block);
            }
        } else {
            syn::Error::new_spanned(
                expr,
                "Unsupported: only `else` block is allowed after an `if` statement.",
            )
            .into_compile_error()
        }
    } else {
        quote::quote! {
            let _cond = #cond;
            burn_cube::frontend::branch::if_expand(context, #comptime_bool, _cond.into(), |context| #then_block);
        }
    }
}

/// Codegen of loop
pub(crate) fn codegen_loop(
    loop_expr: &syn::ExprLoop,
    loop_level: usize,
    variable_tracker: &mut VariableTracker,
) -> TokenStream {
    let block = codegen_block(&loop_expr.body, loop_level + 1, variable_tracker);

    quote::quote! {
        burn_cube::frontend::branch::loop_expand(context, |context| #block);
    }
}

/// Codegen for while loop
pub(crate) fn codegen_while_loop(
    while_loop: &syn::ExprWhile,
    loop_level: usize,
    variable_tracker: &mut VariableTracker,
) -> TokenStream {
    let (cond, is_comptime) =
        codegen_cond(&while_loop.cond, loop_level + 1, variable_tracker).split();

    if is_comptime {
        return syn::Error::new_spanned(while_loop.while_token, "Comptime not supported for while")
            .into_compile_error();
    }

    let block = codegen_block(&while_loop.body, loop_level + 1, variable_tracker);

    quote::quote! {
        burn_cube::frontend::branch::while_loop_expand(context, |context| #cond, |context| #block);
    }
}
