use crate::{CheckError, ExternProcedure, WithMetadata, ID};
use anyhow::Result;
use super::{Stmt, Expr, Procedure, Type, Env};

pub trait ToMage {
    fn compile_to_mage(&self, ctx: &mut Env) -> Result<String, CheckError> {
        Ok(format!("{MAGE_PRELUDE}\n\n{}", self.to_mage(ctx)?))
    }

    fn to_mage(&self, ctx: &mut Env) -> Result<String, CheckError>;
}

// const RETURN_VARIABLE: &str = "__MAGE__return_value";
pub const MAGE_PRELUDE: &str = r#"extern fun clover_add(clover_a, clover_b);
extern fun clover_sub(clover_a, clover_b);
extern fun clover_mul(clover_a, clover_b);
extern fun clover_neg(clover_a);
extern fun clover_puts(clover_value);
extern fun clover_puthex(clover_value);
extern fun clover_putarr(clover_ptr, clover_len);
extern fun clover_putchar(clover_value);
extern fun clover_putln();
extern fun clover_deref(clover_value);
extern fun clover_idx(clover_ptr, clover_i);
extern fun clover_memcpy(clover_dst, clover_src, clover_count);
extern fun clover_malloc(clover_size);

let static clover_STACK = 0;
clover_STACK = clover_malloc(1024);
let static clover_SP = 0;

fun clover_new_scope() {
    return clover_SP;
}

fun clover_push(clover_value) {
    clover_SP = clover_add(clover_SP, 1);
    clover_idx(clover_STACK, clover_SP) = clover_value;
}

fun clover_pop() {
    let clover_value = clover_deref(clover_idx(clover_STACK, clover_SP));
    clover_SP = clover_add(clover_SP, -1);
    return clover_value;
}

fun clover_poparr(clover_ptr, clover_count) {
    while (clover_count) {
        clover_count = clover_add(clover_count, -1);
        clover_idx(clover_ptr, clover_count) = clover_pop();
    }
}

fun clover_pusharr(clover_ptr, clover_count) {
    let clover_i = 0;
    while (clover_count) {
        clover_count = clover_add(clover_count, -1);
        clover_push(clover_deref(clover_idx(clover_ptr, clover_i)));
        clover_i = clover_add(clover_i, 1);
    }
}

fun clover_select(clover_idx_into_struct, clover_idx_len, clover_total_size) {
    // clover_Leave clover_only clover_the clover_indexed clover_value clover_off clover_the clover_stack
    let clover_start = clover_add(clover_add(clover_sub(clover_SP, clover_total_size), 1), clover_idx_into_struct);

    // clover_Subtract clover_the clover_total clover_size clover_from clover_the clover_stack clover_pointer
    clover_SP = clover_sub(clover_SP, clover_total_size);
    while (clover_idx_len) {
        clover_push(clover_deref(clover_idx(clover_STACK, clover_start)));
        clover_start = clover_add(clover_start, 1);
        clover_idx_len = clover_sub(clover_idx_len, 1);
    }
}

fun clover_ret(clover_ebp, clover_count) {
    let offset = clover_sub(clover_count, 1);
    let clover_current_sp = clover_sub(clover_SP, offset);
    // clover_Revert clover_stack clover_pointer
    clover_SP = clover_add(clover_ebp, offset);
    // clover_Copy return clover_values clover_to clover_stack
    clover_memcpy(clover_idx(clover_STACK, clover_ebp), clover_idx(clover_STACK, clover_current_sp), clover_count);
}
"#;

impl ToMage for Procedure {
    fn to_mage(&self, ctx: &mut Env) -> Result<String, CheckError> {
        let mut new_ctx = ctx.new_function_scope();
        // pub name: Symbol,
        // pub args: Vec<(Mutability, Symbol, Type)>,
        // pub ret_ty: Option<Type>,
        // pub body: Box<Stmt>,

        // Keep the name the same
        let mut result = format!("fun {}() {{", self.name);
        // Pop the arguments off the stack in reverse order
        let mut total_size = 0;
        for (mutability, name, ty) in self.args.iter().rev() {
            let ty_size = ctx.get_type_size(ty)?;
            result.push_str(&format!("  let {} = clover_idx(clover_STACK, clover_add(clover_SP, -{}));\n", name, total_size + ty_size - 1));
            total_size += ty_size;
            new_ctx.add_var(false, name.clone(), *mutability, ty.clone());
        }
        new_ctx.add_proc(self.clone());

        result.push_str("   let clover_ebp = clover_new_scope();\n");

        result.push_str(&self.body.to_mage(&mut new_ctx)?);
        result.push_str("}\n");
        Ok(result)
    }
}


impl ToMage for ExternProcedure {
    fn to_mage(&self, ctx: &mut Env) -> Result<String, CheckError> {
        let mut result = format!("extern fun {}(", self.name);

        // Assert that all types are primitive
        for (i, (_mutability, name, ty)) in self.args.iter().enumerate() {
            if !ctx.get_type_size(ty)? == 1 {
                return Err(CheckError::MismatchType {
                    expected: ty.clone(),
                    found: Type::Int,
                    expr: Stmt::ExternProc(self.clone())
                }).with_metadata("Error while translating extern proc to mage");
            }
            if i > 0 {
                result.push_str(", ");
            }
            result.push_str(&format!("{name}"));
        }

        result.push_str(");\n");
        Ok(result)
    }
}


impl ToMage for Stmt {
    fn to_mage(&self, ctx: &mut Env) -> Result<String, CheckError> {
        use Stmt::*;
        ctx.check(self)?;
        match self {
            // Expr(expr) => expr.to_mage(ctx),
            Return(expr) => {
                // Push the return value onto the stack
                let mut result = String::new();
                let expr_size = ctx.get_expr_size(expr)?;
                result += &expr.to_mage(ctx)?;
                result += &format!("    clover_ret(clover_ebp, {expr_size});\n");
                result += &format!("    return 0;\n");

                Ok(result)
            }
            Annotated(metadata, stmt) => {
                stmt.to_mage(ctx).with_metadata(metadata.clone())
            } 
            Expr(e) => e.to_mage(ctx),
            Continue => {
                Ok("    continue;\n".to_string())
            }
            Break => {
                Ok("    break;\n".to_string())
            }

            DeclareVar {
                name,
                is_static,
                value,
                ..
            } => {
                let mut result = String::new();
                result += &value.to_mage(ctx)?;
                let var_size = ctx.get_expr_size(value)?;
                result += &format!("    let {} = clover_idx(clover_STACK, clover_add(clover_SP, -{}));\n", name, var_size - 1);
                if *is_static {
                    // Pop into a static variable
                    todo!();
                }

                Ok(result)
            }
            DeclareProc(proc) => {
                ctx.add_proc(proc.clone());
                Ok(proc.to_mage(ctx)?)
            }
            DeclareType(name, ty) => {
                ctx.add_type(name.clone(), ty.clone())?;
                Ok("".to_string())
            },
            ExternProc(proc) => {
                ctx.add_extern_proc(proc.clone());
                // todo!();
                Ok(proc.to_mage(ctx)?)
            }
            AssignVar(name, val) => {
                let mut result = String::new();
                let val_size = ctx.get_expr_size(val)?;
                result += &val.to_mage(ctx)?;
                result += &format!("    clover_poparr({}, {});\n", name, val_size);
                Ok(result)
            }
            AssignRef(ptr, val) => {
                let mut result = String::new();
                let val_size = ctx.get_expr_size(val)?;
                result += &val.to_mage(ctx)?;
                result += &ptr.to_mage(ctx)?;
                result += &format!("    clover_poparr(clover_pop(), {});\n", val_size);
                Ok(result)
            }
            While(cond, body) => {
                let mut result = String::new();
                let cond_mage = cond.to_mage(ctx)?;
                let body_mage = body.to_mage(ctx)?;
                result += &cond_mage;
                result += &format!("    while (clover_pop()) {{\n", );
                result += &body_mage;
                result += &cond_mage;
                result += "    }\n";
                Ok(result)
            }
            If(cond, then, else_) => {
                let mut result = String::new();
                let cond_mage = cond.to_mage(ctx)?;
                let then_mage = then.to_mage(ctx)?;
                let else_mage = else_.to_mage(ctx)?;
                result += &cond_mage;
                result += &format!("    if (clover_pop()) {{\n");
                result += &then_mage;
                result += "    } else {\n";
                result += &else_mage;
                result += "    }\n";
                Ok(result)
            }
            Block(stmts) => {
                let mut result = String::new();
                let mut new_ctx = ctx.new_local_scope();
                for stmt in stmts {
                    result += &stmt.to_mage(&mut new_ctx)?;
                }
                Ok(result)
            },
        }
        // todo!()
    }
}


impl ToMage for Expr {
    fn to_mage(&self, ctx: &mut Env) -> Result<String, CheckError> {
        use Expr::*;
        // let _ty = ctx.get_expr_type(self)?;
        match self.strip_annotations() {
            Annotated(..) => unreachable!(),
            LengthOfExpr(expr) => {
                let ty = ctx.reduce_type(&ctx.get_expr_type(expr)?);
                // Get the element size
                let elem_size = match ty {
                    Type::Array(elem_ty, _) => ctx.get_type_size(&*elem_ty)?,
                    other => return Err(CheckError::LengthOfNonArray {
                        ty: other,
                        expr: self.clone().into()
                    })
                };
                let arr_size = ctx.get_expr_size(expr)?;
                Ok(format!("    clover_push({});\n", arr_size / elem_size))
            }

            LengthOfType(ty) => {
                // Get the element size
                let elem_size = match ty {
                    Type::Array(elem_ty, _) => ctx.get_type_size(&*elem_ty)?,
                    other => return Err(CheckError::LengthOfNonArray {
                        ty: other.clone(),
                        expr: self.clone().into()
                    })
                };
                let arr_size = ctx.get_type_size(ty)?;
                Ok(format!("    clover_push({});\n", arr_size / elem_size))
            }

            SizeOfExpr(expr) => {
                let size = ctx.get_expr_size(expr)?;
                Ok(format!("    clover_push({});\n", size))
            }

            SizeOfType(ty) => {
                let size = ctx.get_type_size(ty)?;
                Ok(format!("    clover_push({});\n", size))
            }

            Int(val) => {
                Ok(format!("    clover_push({});\n", val))
            }
            Char(val) => {
                Ok(format!("    clover_push({:?});\n", val))
            }
            Bool(val) => {
                Ok(format!("    clover_push({});\n", if *val { 1 } else { 0 }))
            }
            Float(val) => {
                Ok(format!("    clover_push({});\n", val))
            }
            Unit => {
                Ok(format!(""))
            }
            Cast(val, _ty) => {
                Ok(val.to_mage(ctx)?)
            }
            Str(val) => {
                let mut result = format!("    clover_push([");
                for c in val.chars() {
                    result.push_str(&format!("{:?}, ", c));
                }
                result.push_str("0]);\n");
                Ok(result)
            }
            CStr(val) => {
                Ok(format!("    clover_push({:?});\n", val))
            }
            Struct(fields) => {
                let mut result = String::new();
                for (_name, val) in fields {
                    let val_mage = val.to_mage(ctx)?;
                    result += &val_mage;
                }
                Ok(result)
            }
            Var(name) => {
                if ctx.get_var(name.clone()).is_ok() {
                    let size = ctx.get_var_size(name.clone()).unwrap_or(1);
                    Ok(format!("    clover_pusharr({}, {size});\n", name))
                } else {
                    Ok(format!("    clover_push({});\n", name))
                }
            }
            App(name, args) => {
                let mut result = String::new();
                for arg in args {
                    let arg_mage = arg.to_mage(ctx)?;
                    result += &arg_mage;
                }

                let expr_ty = ctx.get_expr_type(self)?;
                let is_unit = ctx.type_equals(&expr_ty, &Type::Unit);
                // Push the procedure
                if ctx.is_extern_proc(name) {
                    // Get the name of the proc
                    let var = name.as_var().unwrap();
                    // Now, get the params of the extern func
                    let proc = ctx.get_extern_proc(var.clone()).unwrap();
                    // Pop off the params into vars in reverse order
                    let id = ID::create();
                    for (_mutability, name, _ty) in proc.args.iter().rev() {
                        result += &format!("    let __EXTERN__{}{id} = clover_pop();\n", name);
                    }
                    // Call the extern function
                    if is_unit {
                        result += &format!("    {}(", var);
                    } else {
                        result += &format!("    clover_push({}(", proc.name);
                    }
                    for (i, (_mutability, name, _ty)) in proc.args.iter().enumerate() {
                        if i > 0 {
                            result += ", ";
                        }
                        result += &format!("__EXTERN__{}{id}", name);
                    }
                    if is_unit {
                        result += ");\n";
                    } else {
                        result += "));\n";
                    }
                } else if ctx.is_proc(name) {
                    // Get the name of the proc
                    let var = name.as_var().unwrap();
                    // Call the function
                    result += &format!("    {}();\n", var);
                }
                Ok(result)
            }

            If(cond, then, else_) => {
                let mut result = String::new();
                let cond_mage = cond.to_mage(ctx)?;
                let then_mage = then.to_mage(ctx)?;
                let else_mage = else_.to_mage(ctx)?;
                result += &cond_mage;
                result += &format!("    if (clover_pop() != 0) {{\n");
                result += &then_mage;
                result += "    } else {\n";
                result += &else_mage;
                result += "    }\n";
                Ok(result)
            }

            Deref(ptr) => {
                let mut result = String::new();
                result += &ptr.to_mage(ctx)?;
                let val_size = ctx.get_expr_size(self)?;
                result += &format!("    clover_pusharr(clover_pop(), {});\n", val_size);
                Ok(result)
            }

            Index(arr, idx) => {
                let mut result = String::new();
                let arr_size = ctx.get_expr_size(arr)?;
                let arr_ty = ctx.get_expr_type(arr)?;
                let id = ID::create();
                match arr_ty {
                    Type::Pointer(_, elem_ty) => {
                        result += &arr.to_mage(ctx)?;
                        result += &idx.to_mage(ctx)?;

                        let elem_size = ctx.get_type_size(&elem_ty)?;
        
                        result += &format!("    let __EXTERN__index_{id} = clover_pop();\n");
                        result += &format!("    let __EXTERN__array_{id} = clover_pop();\n");
                        result += &format!("    clover_pusharr(clover_idx(__EXTERN__array_{id}, clover_mul(__EXTERN__index_{id}, {elem_size})), {elem_size});\n");
                    }
                    Type::Array(elem_ty, _) => {
                        // Get the index into the struct
                        let elem_size = ctx.get_type_size(&elem_ty)?;
                        result += &arr.to_mage(ctx)?;
                        result += &idx.to_mage(ctx)?;
                        result += &format!("    let __EXTERN__index_{id} = clover_pop();\n");
                        result += &format!("    clover_select(clover_mul(__EXTERN__index_{id}, {elem_size}), {elem_size}, {arr_size});\n");
                    }
                    _ => {
                        return Err(CheckError::IndexNonArray {
                            ty: arr_ty,
                            expr: self.clone().into()
                        }).with_metadata("Error while translating index to mage");
                    }
                }
                Ok(result)
            }

            Ref(desired_mutability, expr) => {
                let mut result = String::new();
                match &**expr {
                    Expr::Var(name) => {
                        Ok(format!("    clover_push({name});\n"))
                    }
                    Expr::Select(container, field) => {
                        result += Expr::Ref(*desired_mutability, container.clone().into()).to_mage(ctx)?.as_str();
                        let container_ty = ctx.get_expr_type(container)?;
                        let select_offset = ctx.get_field_offset(&container_ty, field)?;
                        result += &format!("    clover_push(clover_idx(clover_pop(), {select_offset}));\n");

                        Ok(result)
                        // match container.strip_annotations() {
                        //     Expr::Var(_) => {
                        //         let container_ty = ctx.get_expr_type(container)?;
                        //         let select_offset = ctx.get_field_offset(&container_ty, field)?;
                        //         result += &container.to_mage(ctx)?;
                        //         result += &format!("    clover_push(clover_add(clover_pop(), {select_offset}));\n");
                        //         Ok(result)
                        //     }
                        //     other => {
                        //         // Get a reference to the container
                        //     }
                        // }
                    }
                    Expr::Deref(container) => {
                        container.to_mage(ctx)
                    }
                    Expr::Index(arr, idx) => {
                        let mut result = String::new();
                        let arr_ty = ctx.get_expr_type(arr)?;
                        let id = ID::create();
                        match arr_ty {
                            Type::Pointer(_, elem_ty) => {
                                result += &arr.to_mage(ctx)?;
                                result += &idx.to_mage(ctx)?;
                                let val_size = ctx.get_type_size(&elem_ty)?;
                
                                result += &format!("    let __EXTERN__index_{id} = clover_pop();\n");
                                result += &format!("    let __EXTERN__array_{id} = clover_pop();\n");
                                result += &format!("    clover_push(clover_idx(__EXTERN__array_{id}, clover_mul(__EXTERN__index_{id}, {val_size})));\n");
                            }
                            Type::Array(elem_ty, ..) => {
                                result += &Expr::Ref(*desired_mutability, arr.clone()).to_mage(ctx)?;
                                result += &idx.to_mage(ctx)?;
                                let val_size = ctx.get_type_size(&elem_ty)?;
                
                                result += &format!("    let __EXTERN__index_{id} = clover_pop();\n");
                                result += &format!("    let __EXTERN__array_{id} = clover_pop();\n");
                                result += &format!("    clover_push(clover_idx(__EXTERN__array_{id}, clover_mul(__EXTERN__index_{id}, {val_size})));\n");
                            }
                            _ => {
                                return Err(CheckError::InvalidRef { expr: self.clone(), stmt: self.clone().into() }).with_metadata("Error while translating index to mage");
                            }
                        }

                        // if arr_size == 1 {
                        // } else {
                        //     // Get the index into the struct
                        //     let arr_ty = ctx.get_expr_type(arr)?;
                        //     let val_size = ctx.get_expr_size(self)?;
        
                        //     result += &arr.to_mage(ctx)?;
                        //     result += &idx.to_mage(ctx)?;
                        //     result += &format!("    let __EXTERN__index_{id} = clover_pop();\n");
                        //     result += &format!("    clover_select(clover_mul(__EXTERN__index_{id}, {val_size}), {val_size}, {arr_size});\n");
                        // }
                        Ok(result)
                    }
                    _ => {
                        // Get the size of the value
                        let val_size = ctx.get_expr_size(expr)?;
                        result += &expr.to_mage(ctx)?;
                        result += &format!("    clover_pusharr(clover_pop(), {val_size});\n");
                        Ok(result)
                    }
                }
                // Ok(format!("    clover_push({name});\n"))
            }
            // RefSelect(_, container, field) => {
            //     let mut result = String::new();
            //     let container_ty = ctx.get_var_type(container.clone())?;
            //     // Push the field based on the select offset
            //     let select_offset = ctx.get_field_offset(&container_ty, field)?;
            //     result += &format!("    clover_push(clover_idx({container}, {select_offset}));\n");

            //     Ok(result)
            // }
            Array(vals) => {
                let mut result = String::new();
                for val in vals {
                    let val_mage = val.to_mage(ctx)?;
                    result += &val_mage;
                }
                Ok(result)
            }

            Select(container, field) => {
                let mut result = String::new();
                match container.strip_annotations() {
                    Expr::Var(name) => {
                        let container_ty = ctx.get_var_type(name.clone())?;
                        let val_size = ctx.get_expr_size(self)?;
                        // Push the field based on the select offset
                        let select_offset = ctx.get_field_offset(&container_ty, field)?;
                        result += &format!("    clover_pusharr(clover_idx({name}, {select_offset}), {val_size});\n");
                    }
                    other => {
                        // Get the index into the struct
                        let container_ty = ctx.get_expr_type(other)?;
                        let container_size = ctx.get_expr_size(other)?;
                        let val_size = ctx.get_expr_size(self)?;
                        let select_offset = ctx.get_field_offset(&container_ty, field)?;

                        result += &other.to_mage(ctx)?;
                        result += &format!("    clover_select({select_offset}, {val_size}, {container_size});\n");
                    }
                }

                Ok(result)
            }

            Enum(ty, variant) => {
                let mut result = String::new();
                let variant_offset = ctx.get_variant_index(ty, variant)?;
                result += &format!("    clover_push({variant_offset});\n");
                Ok(result)
            }

            Union(ty, _variant, val) => {
                let mut result = String::new();
                result += &val.to_mage(ctx)?;

                // Get the difference between the size of the value and the union
                let union_size = ctx.get_type_size(ty)?;
                // Get the size of the value
                let val_size = ctx.get_expr_size(val)?;

                for _ in val_size..union_size {
                    result += &format!("    clover_push(0);\n");
                }

                Ok(result)
            }
        }
    }
}
