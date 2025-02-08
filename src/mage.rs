use crate::{CheckError, ExternProcedure, WithContext, ID};

use super::{Stmt, Expr, Procedure, Type, Env, Mutability};

pub trait ToMage {
    fn compile_to_mage(&self, ctx: &mut Env) -> Result<String, CheckError> {
        Ok(format!("{MAGE_PRELUDE}\n\n{}", self.to_mage(ctx)?))
    }

    fn to_mage(&self, ctx: &mut Env) -> Result<String, CheckError>;
}

// const RETURN_VARIABLE: &str = "__MAGE__return_value";
const MAGE_PRELUDE: &str = r#"extern fun add(a, b);
extern fun sub(a, b);
extern fun puts(value);
extern fun puthex(value);
extern fun putarr(ptr, len);
extern fun putchar(value);
extern fun putln();
extern fun deref(value);
extern fun idx(ptr, i);
extern fun memcpy(dst, src, count);
extern fun malloc(size);

let static STACK = 0;
STACK = malloc(1024);
let static SP = 0;

fun new_scope() {
    return SP;
}

fun push(value) {
    SP = add(SP, 1);
    idx(STACK, SP) = value;
}

fun pop() {
    let value = deref(idx(STACK, SP));
    SP = add(SP, -1);
    return value;
}

fun poparr(ptr, count) {
    while (count) {
        count = add(count, -1);
        idx(ptr, count) = pop();
    }
}

fun pusharr(ptr, count) {
    let i = 0;
    while (count) {
        count = add(count, -1);
        push(deref(idx(ptr, i)));
        i = add(i, 1);
    }
}

fun ret(ebp, count) {
    let current_sp = SP;
    // Revert stack pointer
    SP = add(ebp, count);
    // Copy return values to stack
    memcpy(idx(STACK, ebp), idx(STACK, current_sp), count);
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
            let ty_size = ctx.get_type_size(ty);
            result.push_str(&format!("  let {} = idx(STACK, add(SP, -{}));\n", name, total_size + ty_size - 1));
            total_size += ty_size;
            new_ctx.add_var(false, name.clone(), *mutability, ty.clone());
        }
        new_ctx.add_proc(self.clone());

        result.push_str("   let ebp = new_scope();\n");

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
            if !ty.is_primitive() {
                return Err(CheckError::MismatchType {
                    expected: ty.clone(),
                    found: Type::Int,
                    expr: Stmt::ExternProc(self.clone())
                });
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
                result += &format!("    ret(ebp, {expr_size});\n");

                Ok(result)
            }
            Annotated(metadata, stmt) => {
                stmt.to_mage(ctx).with_context(metadata.clone())
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
                result += &format!("    let {} = idx(STACK, add(SP, -{}));\n", name, var_size - 1);
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
                ctx.add_type(name.clone(), ty.clone());
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
                result += &format!("    poparr({}, {});\n", name, val_size);
                Ok(result)
            }
            AssignRef(ptr, val) => {
                let mut result = String::new();
                let val_size = ctx.get_expr_size(val)?;
                result += &val.to_mage(ctx)?;
                result += &ptr.to_mage(ctx)?;
                result += &format!("    poparr(pop(), {});\n", val_size);
                Ok(result)
            }
            While(cond, body) => {
                let mut result = String::new();
                let cond_mage = cond.to_mage(ctx)?;
                let body_mage = body.to_mage(ctx)?;
                result += &cond_mage;
                result += &format!("    while (pop() != 0) {{\n", );
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
                result += &format!("    if (pop() != 0) {{\n");
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
            Int(val) => {
                Ok(format!("    push({});\n", val))
            }
            Char(val) => {
                Ok(format!("    push({:?});\n", val))
            }
            Bool(val) => {
                Ok(format!("    push({});\n", if *val { 1 } else { 0 }))
            }
            Float(val) => {
                Ok(format!("    push({});\n", val))
            }
            Struct(fields) => {
                let mut result = String::new();
                for (name, val) in fields {
                    let val_mage = val.to_mage(ctx)?;
                    result += &val_mage;
                }
                Ok(result)
            }
            Var(name) => {
                if ctx.get_var(name.clone()).is_ok() {
                    let size = ctx.get_var_size(name.clone()).unwrap_or(1);
                    Ok(format!("    pusharr({}, {size});\n", name))
                } else {
                    Ok(format!("    push({});\n", name))
                }
            }
            App(name, args) => {
                let mut result = String::new();
                for arg in args {
                    let arg_mage = arg.to_mage(ctx)?;
                    result += &arg_mage;
                }

                // Push the procedure
                if ctx.is_extern_proc(name) {
                    // Get the name of the proc
                    let var = name.as_var().unwrap();
                    // Now, get the params of the extern func
                    let proc = ctx.get_extern_proc(var.clone()).unwrap();
                    // Pop off the params into vars in reverse order
                    let id = ID::create();
                    for (_mutability, name, ty) in proc.args.iter().rev() {
                        result += &format!("    let __EXTERN__{}{id} = pop();\n", name);
                    }
                    // Call the extern function
                    result += &format!("    push({}(", proc.name);
                    for (i, (_mutability, name, _ty)) in proc.args.iter().enumerate() {
                        if i > 0 {
                            result += ", ";
                        }
                        result += &format!("__EXTERN__{}{id}", name);
                    }
                    result += "));\n";
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
                result += &format!("    if (pop() != 0) {{\n");
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
                result += &format!("    pusharr(pop(), {});\n", val_size);
                Ok(result)
            }

            Index(arr, idx) => {
                let mut result = String::new();
                result += &arr.to_mage(ctx)?;
                result += &idx.to_mage(ctx)?;
                let val_size = ctx.get_expr_size(self)?;

                result += &format!("    let idx = pop(); let ptr = pop(); pusharr(ptr, {val_size});\n");
                Ok(result)
            }

            Ref(_, name) => {
                Ok(format!("    push({name});\n"))
            }

            RefSelect(_, container, field) => {
                let mut result = String::new();
                let container_ty = ctx.get_var_type(container.clone())?;
                // Push the field based on the select offset
                let select_offset = ctx.get_field_offset(&container_ty, field)?;
                result += &format!("    push(idx({container}, {select_offset}));\n");

                Ok(result)
            }
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
                let container_ty = ctx.get_var_type(container.clone())?;
                let val_size = ctx.get_expr_size(self)?;
                // Push the field based on the select offset
                let select_offset = ctx.get_field_offset(&container_ty, field)?;
                result += &format!("    pusharr(idx({container}, {select_offset}), {val_size});\n");

                Ok(result)
            }

            Enum(ty, variant) => {
                let mut result = String::new();
                let variant_offset = ty.get_variant_offset(variant)?;
                result += &format!("    push({variant_offset});\n");
                Ok(result)
            }

            Union(ty, variant, val) => {
                let mut result = String::new();
                result += &val.to_mage(ctx)?;
                Ok(result)
            }
        }
    }
}
