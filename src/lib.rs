use lazy_static::lazy_static;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet}, sync::{Mutex, RwLock},
    fmt::{Debug, Display, Formatter, Result as FmtResult},
};

mod symbol;
pub use symbol::*;

mod parser;
pub use parser::*;

pub mod mage;
use mage::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Mutability {
    Mutable,
    Immutable,
}

impl Mutability {
    pub fn can_use_as(&self, desired_mutability: Self) -> bool {
        match (self, desired_mutability) {
            (Mutability::Mutable, _) => true,
            (Mutability::Immutable, Mutability::Immutable) => true,
            _ => false,
        }
    }
}

impl From<bool> for Mutability {
    fn from(value: bool) -> Self {
        if value {
            Mutability::Mutable
        } else {
            Mutability::Immutable
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Type {
    Named(Symbol),
    Int,
    Float,
    Bool,
    Char,
    Unit,
    Pointer(Mutability, Box<Type>),
    Array(Box<Type>, usize),
    Struct(BTreeMap<Symbol, Type>),
    Enum(BTreeSet<Symbol>),
    Union(BTreeMap<Symbol, Type>),
    Procedure(Vec<Type>, Box<Type>),
}

impl Type {
    pub fn is_primitive(&self) -> bool {
        matches!(self, Type::Int | Type::Float | Type::Bool | Type::Char | Type::Unit)
    }

    pub fn is_pointer(&self) -> bool {
        matches!(self, Type::Pointer(_, _))
    }

    pub fn is_array(&self) -> bool {
        matches!(self, Type::Array(..))
    }

    pub fn is_struct(&self) -> bool {
        matches!(self, Type::Struct(_))
    }

    pub fn is_enum(&self) -> bool {
        matches!(self, Type::Enum(_))
    }

    pub fn is_union(&self) -> bool {
        matches!(self, Type::Union(_))
    }

    pub fn get_variant_offset(&self, variant: &Symbol) -> Result<usize, CheckError> {
        use Type::*;
        match self {
            Enum(variants) => {
                variants.iter().position(|v| v == variant).ok_or(CheckError::FieldNotFound {
                    container: self.clone(),
                    name: variant.clone(),
                    expr: Stmt::Expr(Expr::Var(variant.clone())).into(),
                })
            },
            _ => Err(CheckError::SelectNonStruct {
                container: self.clone(),
                field: variant.clone(),
                expr: Stmt::Expr(Expr::Var(variant.clone())).into(),
            }),
        }
    }

    pub fn refer(&self, mutability: Mutability) -> Type {
        Type::Pointer(mutability, Box::new(self.clone()))
    }

    pub fn deref(&self, expected_mutability: Mutability) -> Option<Type> {
        match self {
            Type::Pointer(found_mutability, ty) if found_mutability.can_use_as(expected_mutability) => Some(*ty.clone()),
            _ => None,
        }
    }

    pub fn array(&self, size: usize) -> Type {
        Type::Array(Box::new(self.clone()), size)
    }

    pub fn record(fields: impl IntoIterator<Item=(impl AsRef<str>, Type)>) -> Type {
        Type::Struct(fields.into_iter().map(|(name, ty)| (name.as_ref().into(), ty)).collect())
    }

    pub fn procedure(params: impl IntoIterator<Item=Type>, ret_ty: Option<Type>) -> Type {
        Type::Procedure(params.into_iter().collect(), Box::new(ret_ty.unwrap_or(Type::Unit)))
    }
    pub fn enum_(variants: impl IntoIterator<Item=Symbol>) -> Type {
        Type::Enum(variants.into_iter().collect())
    }

    pub fn union(fields: impl IntoIterator<Item=(Symbol, Type)>) -> Type {
        Type::Union(fields.into_iter().collect())
    }

    pub fn proc(args: impl IntoIterator<Item=Type>, ret: Type) -> Type {
        Type::Procedure(args.into_iter().collect(), Box::new(ret))
    }
}


#[derive(Debug, Clone)]
pub struct SourceCodeLocation {
    line: usize,
    column: usize,
    length: usize,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ID {
    id: usize,
}

lazy_static! {
    static ref IDS: RwLock<HashMap<String, ID>> = RwLock::new(HashMap::new());
    static ref NAMES: RwLock<HashMap<ID, String>> = RwLock::new(HashMap::new());
}

impl ID {
    fn create() -> Self {
        lazy_static! {
            static ref COUNTER: Mutex<usize> = Mutex::new(0);
        }

        let mut counter = COUNTER.lock().unwrap();
        *counter += 1;
        Self { id: *counter }
    }

    pub fn new(name: &str) -> Self {
        // Check if it already exists
        {
            let ids = IDS.read().unwrap();
            if let Some(id) = ids.get(name) {
                return *id;
            }
        }

        // Create a new one
        let id = Self::create();
        let mut ids = IDS.write().unwrap();
        ids.insert(name.to_string(), id);
        let mut names = NAMES.write().unwrap();
        names.insert(id, name.to_string());
        id
    }

    pub fn get_name(&self) -> String {
        let names = NAMES.read().unwrap();
        names.get(self).unwrap().clone()
    }
}

impl Display for ID {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "ID_{}", self.id)
    }
}

#[derive(Debug, Clone)]
pub enum Metadata {
    Many(Vec<Self>),
    Location(SourceCodeLocation),
}

impl std::fmt::Display for Metadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Metadata::Many(metadata) => {
                for m in metadata {
                    write!(f, "{}", m)?;
                }
                Ok(())
            }
            Metadata::Location(location) => {
                write!(f, "line: {}, column: {}, length: {}", location.line, location.column, location.length)
            }
        }
    }
}

impl From<SourceCodeLocation> for Metadata {
    fn from(location: SourceCodeLocation) -> Self {
        Metadata::Location(location)
    }
}

#[derive(Debug, Clone)]
pub enum Expr {
    Annotated(Metadata, Box<Self>),
    If(Box<Self>, Box<Self>, Box<Self>),
    Int(i64),
    Char(char),
    Float(f64),
    Bool(bool),
    Var(Symbol),
    Ref(Mutability, Symbol),
    RefSelect(Mutability, Symbol, Symbol),
    App(Box<Self>, Vec<Self>),
    Array(Vec<Self>),

    Select(Symbol, Symbol),
    Index(Box<Self>, Box<Self>),
    Deref(Box<Self>),

    Struct(BTreeMap<Symbol, Self>),
    Enum(Type, Symbol),
    Union(Type, Symbol, Box<Self>),
}

impl Expr {
    pub fn annotate(self, metadata: impl Into<Metadata>) -> Self {
        Self::Annotated(metadata.into(), Box::new(self))
    }

    pub fn as_var(&self) -> Option<&Symbol> {
        match self {
            Self::Var(name) => Some(name),
            _ => None,
        }
    }

    pub fn strip_annotations(&self) -> &Self {
        match self {
            Self::Annotated(_, expr) => expr.strip_annotations(),
            _ => self,
        }
    }
}

pub fn ref_(mutability: impl Into<Mutability>, name: impl ToString) -> Expr {
    Expr::Ref(mutability.into(), name.to_string().into())
}

pub fn var(name: impl ToString) -> Expr {
    Expr::Var(name.to_string().into())
}

pub fn if_expr(cond: impl Into<Expr>, then: impl Into<Expr>, else_: impl Into<Expr>) -> Expr {
    Expr::If(Box::new(cond.into()), Box::new(then.into()), Box::new(else_.into()))
}

pub fn app(func: impl Into<Expr>, args: Vec<impl Into<Expr>>) -> Expr {
    let func = func.into();
    let args = args.into_iter().map(|arg| arg.into()).collect();
    Expr::App(Box::new(func), args)
}

impl Expr {
    pub fn app(self, args: Vec<Self>) -> Self {
        Self::App(Box::new(self), args)
    }
}

impl From<String> for Expr {
    fn from(value: String) -> Self {
        Expr::Var(value.into())
    }
}

impl From<&str> for Expr {
    fn from(value: &str) -> Self {
        Expr::Var(value.into())
    }
}

impl From<i64> for Expr {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<char> for Expr {
    fn from(value: char) -> Self {
        Self::Char(value)
    }
}

impl From<f64> for Expr {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

impl From<bool> for Expr {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

#[derive(Debug, Clone)]
pub struct Procedure {
    pub name: Symbol,
    pub args: Vec<(Mutability, Symbol, Type)>,
    pub ret_ty: Option<Type>,
    pub body: Box<Stmt>,
}

impl Procedure {
    pub fn get_type(&self) -> Type {
        Type::Procedure(
            self.args.iter().map(|(_, _, ty)| ty.clone()).collect(),
            Box::new(self.ret_ty.clone().unwrap_or(Type::Unit)),
        )
    }
}

#[derive(Debug, Clone)]
pub struct ExternProcedure {
    pub name: Symbol,
    pub args: Vec<(Mutability, Symbol, Type)>,
    pub ret_ty: Option<Type>,
    pub body: Option<Box<Stmt>>,
}

impl ExternProcedure {
    pub fn get_type(&self) -> Type {
        Type::Procedure(
            self.args.iter().map(|(_, _, ty)| ty.clone()).collect(),
            Box::new(self.ret_ty.clone().unwrap_or(Type::Unit)),
        )
    }
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Annotated(Metadata, Box<Self>),
    Expr(Expr),
    Return(Expr),
    Continue,
    Break,
    DeclareVar {
        mutability: Mutability,
        name: Symbol,
        is_static: bool,
        ty: Option<Type>,
        value: Box<Expr>,
    },
    DeclareProc(Procedure),
    DeclareType(Symbol, Type),
    ExternProc(ExternProcedure),
    AssignVar(Symbol, Box<Expr>),
    AssignRef(Box<Expr>, Box<Expr>),
    While(Box<Expr>, Box<Self>),
    If(Box<Expr>, Box<Self>, Box<Self>),
    Block(Vec<Self>),
}

impl Stmt {
    pub fn annotate(self, metadata: impl Into<Metadata>) -> Self {
        Self::Annotated(metadata.into(), Box::new(self))
    }

    pub fn strip_annotations(&self) -> &Self {
        match self {
            Self::Annotated(_, stmt) => stmt.strip_annotations(),
            _ => self,
        }
    }
}

impl From<Expr> for Stmt {
    fn from(value: Expr) -> Self {
        Stmt::Expr(value)
    }
}

impl From<Box<Expr>> for Stmt {
    fn from(value: Box<Expr>) -> Self {
        Stmt::Expr(*value)
    }
}

pub fn let_var(mutability: impl Into<Mutability>, name: impl ToString, ty: Option<Type>, value: impl Into<Expr>) -> Stmt {
    Stmt::DeclareVar {
        mutability: mutability.into(),
        name: Symbol::new(&name.to_string()),
        is_static: false,
        ty,
        value: Box::new(value.into()),
    }
}

pub fn let_static(mutability: impl Into<Mutability>, name: impl ToString, ty: Option<Type>, value: impl Into<Expr>) -> Stmt {
    Stmt::DeclareVar {
        mutability: mutability.into(),
        name: Symbol::new(&name.to_string()),
        is_static: true,
        ty,
        value: Box::new(value.into()),
    }
}

pub fn proc(name: impl ToString, args: Vec<(Mutability, impl ToString, Type)>, ret_ty: Option<Type>, body: impl Into<Stmt>) -> Stmt {
    Stmt::DeclareProc(Procedure {
        name: Symbol::new(&name.to_string()),
        args: args.into_iter().map(|(mutability, arg, ty)| (mutability, Symbol::new(&arg.to_string()), ty)).collect(),
        ret_ty,
        body: Box::new(body.into()),
    })
}

pub fn extern_proc(name: impl ToString, args: Vec<(Mutability, impl ToString, Type)>, ret_ty: Option<Type>, body: Option<Stmt>) -> Stmt {
    Stmt::ExternProc(ExternProcedure {
        name: Symbol::new(&name.to_string()),
        args: args.into_iter().map(|(mutability, arg, ty)| (mutability, Symbol::new(&arg.to_string()), ty)).collect(),
        ret_ty,
        body: body.map(Box::new),
    })
}

pub fn assign_var(name: impl Into<Symbol>, value: impl Into<Expr>) -> Stmt {
    Stmt::AssignVar(name.into(), Box::new(value.into()))
}

pub fn assign_ref(dst: impl Into<Expr>, src: impl Into<Expr>) -> Stmt {
    Stmt::AssignRef(Box::new(dst.into()), Box::new(src.into()))
}

pub fn while_(cond: impl Into<Expr>, body: impl Into<Stmt>) -> Stmt {
    Stmt::While(Box::new(cond.into()), Box::new(body.into()))
}

fn if_(cond: impl Into<Expr>, then: impl Into<Stmt>, else_: impl Into<Stmt>) -> Stmt {
    Stmt::If(Box::new(cond.into()), Box::new(then.into()), Box::new(else_.into()))
}

fn return_(value: impl Into<Expr>) -> Stmt {
    Stmt::Return(value.into())
}

pub fn block(stmts: Vec<Stmt>) -> Stmt {
    Stmt::Block(stmts)
}

pub fn stmt(expr: impl Into<Expr>) -> Stmt {
    Stmt::Expr(expr.into())
}

#[derive(Debug, Clone)]
pub enum CheckError {
    MismatchMutability {
        expected: Mutability,
        found: Mutability,
        expr: Stmt,
    },
    MismatchType {
        expected: Type,
        found: Type,
        expr: Stmt,
    },
    FieldNotFound {
        container: Type,
        name: Symbol,
        expr: Stmt,
    },
    VariableNotFound {
        name: Symbol,
        expr: Stmt,
    },
    SelectNonStruct {
        container: Type,
        field: Symbol,
        expr: Stmt,
    },
    TypeNotFound(Symbol),
    ProcNotFound(Symbol),
    WithContext(Box<CheckError>, Metadata),
}

trait WithContext {
    fn with_context(self, metadata: impl Into<Metadata>) -> Self;
}

impl WithContext for CheckError {
    fn with_context(self, metadata: impl Into<Metadata>) -> Self {
        CheckError::WithContext(Box::new(self), metadata.into())
    }
}

impl<T> WithContext for Result<T, CheckError> {
    fn with_context(self, metadata: impl Into<Metadata>) -> Self {
        self.map_err(|e| e.with_context(metadata))
    }
}

#[derive(Debug, Clone)]
pub struct Env {
    types: HashMap<Symbol, Type>,
    locals: HashMap<Symbol, (Mutability, Type)>,
    statics: HashMap<Symbol, (Mutability, Type)>,
    procs: HashMap<Symbol, Procedure>,
    extern_procs: HashMap<Symbol, ExternProcedure>,
    scope: ID,
}

impl Env {
    pub fn new_function_scope(&self) -> Self {
        Self {
            types: self.types.clone(),
            locals: HashMap::new(),
            statics: self.statics.clone(),
            scope: ID::create(),
            procs: self.procs.clone(),
            extern_procs: self.extern_procs.clone(),
        }
    }

    pub fn new_local_scope(&self) -> Self {
        Self {
            types: self.types.clone(),
            locals: self.locals.clone(),
            statics: self.statics.clone(),
            scope: ID::create(),
            procs: self.procs.clone(),
            extern_procs: self.extern_procs.clone(),
        }
    }

    pub fn add_type(&mut self, name: impl Into<Symbol>, ty: Type) {
        self.types.insert(name.into(), ty);
    }

    pub fn add_var(&mut self, is_static: bool, name: impl Into<Symbol>, mutability: Mutability, ty: Type) {
        if is_static {
            self.statics.insert(name.into(), (mutability, ty));
        } else {
            self.locals.insert(name.into(), (mutability, ty));
        }
    }

    pub fn is_proc(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Var(name) => self.procs.contains_key(name),
            _ => false,
        }
    }

    pub fn is_extern_proc(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Var(name) => self.extern_procs.contains_key(name),
            _ => false,
        }
    }

    pub fn add_proc(&mut self, proc: Procedure) {
        self.procs.insert(proc.name.clone(), proc);
    }

    pub fn add_extern_proc(&mut self, proc: ExternProcedure) {
        self.extern_procs.insert(proc.name.clone(), proc);
    }

    pub fn get_type(&self, name: impl Into<Symbol>) -> Result<Type, CheckError> {
        let name = name.into();
        self.types.get(&name).cloned().ok_or(CheckError::TypeNotFound(name))
    }

    pub fn get_var(&self, name: impl Into<Symbol>) -> Result<(Mutability, Type), CheckError> {
        let name = name.into();
        self.locals.get(&name).cloned().or_else(|| self.statics.get(&name).cloned()).ok_or(CheckError::VariableNotFound {
            expr: Expr::Var(name.clone()).into(),
            name,
        })
    }

    pub fn get_proc(&self, name: impl Into<Symbol>) -> Result<Procedure, CheckError> {
        let name = name.into();
        self.procs.get(&name).cloned().ok_or(CheckError::ProcNotFound(name))
    }

    pub fn get_extern_proc(&self, name: impl Into<Symbol>) -> Result<ExternProcedure, CheckError> {
        let name = name.into();
        self.extern_procs.get(&name).cloned().ok_or(CheckError::ProcNotFound(name))
    }

    pub fn get_proc_type(&self, name: impl Into<Symbol>) -> Result<Type, CheckError> {
        let name = name.into();
        self.get_proc(name.clone()).map(|proc| proc.get_type())
            .or_else(|_| self.get_extern_proc(name).map(|proc| proc.get_type()))
    }

    pub fn get_var_type(&self, name: impl Into<Symbol>) -> Result<Type, CheckError> {
        self.get_var(name).map(|(_, ty)| ty)
    }

    pub fn get_var_mutability(&self, name: impl Into<Symbol>) -> Result<Mutability, CheckError> {
        self.get_var(name).map(|(m, _)| m)
    }

    pub fn can_coerce_to(&self, found: &Type, desired: &Type) -> bool {
        if found == desired {
            return true;
        }
        use Type::*;
        match (found, desired) {
            (Array(t1, len1), Array(t2, len2)) => {
                if len1 != len2 {
                    return false;
                }
                self.can_coerce_to(t1, t2)
            },
            (Array(t1, _), Pointer(_, t2)) => self.can_coerce_to(t1, t2),
            (Pointer(m1, t1), Pointer(m2, t2)) => {
                m1.can_use_as(*m2)
                && self.can_coerce_to(t1, t2)
            },

            _ => false,
        }
    }

    pub fn type_equals(&self, a: &Type, b: &Type) -> bool {
        use Type::*;
        println!("Comparing {:?} with {:?}", a, b);
        match (a, b) {
            (Int, Int) => true,
            (Float, Float) => true,
            (Bool, Bool) => true,
            (Char, Char) => true,
            (Unit, Unit) => true,

            (Named(a_name), Named(b_name)) => {
                a_name == b_name || self.type_equals(self.types.get(a_name).unwrap_or(a), b)
            }
            (Named(a_name), _) => {
                if let Some(ty) = self.types.get(a_name) {
                    self.type_equals(ty, b)
                } else {
                    false
                }
            }
            (_, Named(b_name)) => {
                if let Some(ty) = self.types.get(b_name) {
                    self.type_equals(a, ty)
                } else {
                    false
                }
            }

            (Struct(a_fields), Struct(b_fields)) => {
                a_fields.len() == b_fields.len() && a_fields.iter().all(|(name, ty)| {
                    b_fields.get(name).map_or(false, |b_ty| self.type_equals(ty, b_ty))
                })
            }

            (Enum(a_variants), Enum(b_variants)) => a_variants == b_variants,

            (Union(a_fields), Union(b_fields)) => {
                a_fields.len() == b_fields.len() && a_fields.iter().all(|(name, ty)| {
                    b_fields.get(name).map_or(false, |b_ty| self.type_equals(ty, b_ty))
                })
            }

            (Procedure(a_args, a_ret), Procedure(b_args, b_ret)) => {
                a_args.len() == b_args.len() && a_ret == b_ret && a_args.iter().zip(b_args).all(|(a, b)| self.type_equals(a, &b))
            }

            (Array(a_ty, a_size), Array(b_ty, b_size)) => a_size == b_size && self.type_equals(a_ty, b_ty),

            (Pointer(a_mut, a_ty), Pointer(b_mut, b_ty)) => a_mut == b_mut && self.type_equals(a_ty, b_ty),

            (a, b) => {
                // println!("Type mismatch: {:?} != {:?}", a, b);
                // false
                self.can_coerce_to(a, b) || self.can_coerce_to(b, a)
            },
        }
    }

    pub fn reduce_type(&self, ty: &Type) -> Type {
        use Type::*;
        match ty {
            Named(name) => self.types.get(name).cloned().unwrap_or(ty.clone()),
            Pointer(mutability, ty) => Pointer(*mutability, Box::new(self.reduce_type(ty))),
            Array(ty, size) => Array(Box::new(self.reduce_type(ty)), *size),
            Struct(fields) => Struct(fields.iter().map(|(name, ty)| (name.clone(), self.reduce_type(ty))).collect()),
            Enum(variants) => Enum(variants.clone()),
            Union(fields) => Union(fields.iter().map(|(name, ty)| (name.clone(), self.reduce_type(ty))).collect()),
            Procedure(args, ret) => Procedure(args.iter().map(|arg| self.reduce_type(arg)).collect(), Box::new(self.reduce_type(ret))),
            Int | Float | Bool | Char | Unit => ty.clone(),
        }
    }

    pub fn get_expr_type(&self, expr: &Expr) -> Result<Type, CheckError> {
        use Expr::*;
        use CheckError::*;
        match expr {
            Var(name) => {
                match self.get_var_type(name.clone()) {
                    Ok(ty) => Ok(ty),
                    Err(e) => {
                        println!("Could not find variable {name:?} in scope {self:#?}");
                        self.get_proc_type(name.clone()).map_err(|_| e)
                    }
                }
            },

            Deref(expr) => {
                let ty = self.get_expr_type(expr)?;
                let ty = self.reduce_type(&ty);
                ty.deref(Mutability::Immutable).ok_or(MismatchType {
                    expected: Type::Pointer(Mutability::Immutable, Box::new(Type::Unit)),
                    found: ty,
                    expr: expr.clone().into(),
                })
            }

            Ref(desired_mutability, name) => {
                let (found_mutability, ty) = self.get_var(name.clone())?;
                if !found_mutability.can_use_as(*desired_mutability) {
                    return Err(MismatchMutability {
                        expected: *desired_mutability,
                        found: found_mutability,
                        expr: expr.clone().into(),
                    });
                }
                Ok(ty.refer(*desired_mutability))
            },
            RefSelect(desired_mutability, container, name) => {
                // let container_ty = self.get_expr_type(&Expr::Var(container.clone()))?;
                let (found_container_mutability, container_ty) = self.get_var(container.clone())?;
                let container_ty = self.reduce_type(&container_ty);
                match &container_ty {
                    Type::Struct(fields) | Type::Union(fields) => {
                        if !found_container_mutability.can_use_as(*desired_mutability) {
                            return Err(MismatchMutability {
                                expected: *desired_mutability,
                                found: found_container_mutability,
                                expr: expr.clone().into(),
                            });
                        }

                        let deref_ty = fields.get(name).cloned().ok_or(FieldNotFound {
                            container: container_ty,
                            name: name.clone(),
                            expr: expr.clone().into(),
                        })?;

                        Ok(deref_ty.refer(*desired_mutability))
                    },
                    /*
                    Type::Pointer(found_mutability, ty) => {
                        if !found_mutability.can_use_as(*desired_mutability) {
                            return Err(MismatchMutability {
                                expected: *desired_mutability,
                                found: *found_mutability,
                                expr: expr.clone().into(),
                            });
                        }

                        match &**ty {
                            Type::Struct(fields) | Type::Union(fields) => {
                                let deref_ty = fields.get(name).cloned().ok_or(FieldNotFound {
                                    container: container_ty,
                                    name: name.clone(),
                                    expr: expr.clone().into(),
                                })?;

                                Ok(deref_ty.refer(*desired_mutability))
                            },
                            _ => Err(MismatchType {
                                expected: Type::Struct(BTreeMap::new()),
                                found: container_ty,
                                expr: expr.clone().into(),
                            }),
                        }
                    }
                     */
                    _ => Err(MismatchType {
                        expected: Type::Struct(BTreeMap::new()),
                        found: container_ty,
                        expr: expr.clone().into(),
                    }),
                }
            },

            Annotated(metadata, expr) => {
                self.get_expr_type(&expr.strip_annotations()).with_context(metadata.clone())
            }

            Select(struct_, field) => {
                let struct_ = Expr::Var(struct_.clone());
                let struct_ty = self.get_expr_type(&struct_)?;
                let struct_ty = self.reduce_type(&struct_ty);
                match &struct_ty {
                    Type::Union(fields) | Type::Struct(fields) => {
                        fields.get(field).cloned().ok_or(MismatchType {
                            expected: Type::Unit,
                            found: Type::Unit,
                            expr: expr.clone().into(),
                        })
                    },
                    /*
                    Type::Pointer(_, ty) => {
                        let ty = self.reduce_type(ty);
                        match &ty {
                            Type::Union(fields) | Type::Struct(fields) => {
                                fields.get(field).cloned().ok_or(MismatchType {
                                    expected: Type::Unit,
                                    found: Type::Unit,
                                    expr: expr.clone().into(),
                                })
                            },
                            _ => Err(MismatchType {
                                expected: Type::Struct(BTreeMap::new()),
                                found: ty,
                                expr: expr.clone().into(),
                            }),
                        }
                    }
                     */
                    _ => Err(MismatchType {
                        expected: Type::Struct(BTreeMap::new()),
                        found: struct_ty,
                        expr: expr.clone().into(),
                    }),
                }
            }

            Index(array, index) => {
                let array_ty = self.get_expr_type(array)?;
                let array_ty = self.reduce_type(&array_ty);
                let index_ty = self.get_expr_type(index)?;
                if !self.type_equals(&index_ty, &Type::Int) {
                    return Err(MismatchType {
                        expected: Type::Int,
                        found: index_ty,
                        expr: expr.clone().into(),
                    });
                }

                match array_ty {
                    Type::Array(ty, _) => Ok(*ty),
                    _ => Err(MismatchType {
                        expected: Type::Array(Box::new(Type::Unit), 0),
                        found: array_ty,
                        expr: expr.clone().into(),
                    }),
                }
            }

            If(cond, then, else_) => {
                let cond_ty = self.get_expr_type(cond)?;
                if !self.type_equals(&cond_ty, &Type::Bool) {
                    return Err(MismatchType {
                        expected: Type::Bool,
                        found: cond_ty,
                        expr: cond.clone().into(),
                    });
                }
                let then_ty = self.get_expr_type(then)?;
                let else_ty = self.get_expr_type(else_)?;
                if !self.type_equals(&then_ty, &else_ty) {
                    return Err(MismatchType {
                        expected: then_ty,
                        found: else_ty,
                        expr: expr.clone().into(),
                    });
                }
                Ok(then_ty)
            }

            Int(_) => Ok(Type::Int),
            Char(_) => Ok(Type::Char),
            Float(_) => Ok(Type::Float),
            Bool(_) => Ok(Type::Bool),

            App(f, args) => {
                // Get the type of `f`
                let f_ty = self.get_expr_type(f)?;
                // Get the params and return type of `f`
                let f_ty = self.reduce_type(&f_ty);

                // Get the types of `args`
                let arg_tys = args.iter().map(|arg| self.get_expr_type(arg)).collect::<Result<Vec<_>, _>>()?;
                
                // Check if `f` is a procedure
                let (param_tys, ret_ty) = match &f_ty {
                    Type::Procedure(param_tys, ret_ty) => (param_tys, ret_ty),
                    _ => return Err(MismatchType {
                        expected: Type::Procedure(Vec::new(), Box::new(Type::Unit)),
                        found: f_ty,
                        expr: expr.clone().into(),
                    }),
                };

                // Check if the number of arguments match
                if arg_tys.len() != args.len() {
                    return Err(MismatchType {
                        expected: Type::Procedure(arg_tys.clone(), Box::new(Type::Unit)),
                        found: f_ty,
                        expr: expr.clone().into(),
                    });
                }

                // Check if the types of `args` match the params of `f`
                for (arg_ty, param_ty) in arg_tys.iter().zip(param_tys) {
                    if !self.type_equals(arg_ty, param_ty) {
                        return Err(MismatchType {
                            expected: Type::Procedure(arg_tys.clone(), Box::new(Type::Unit)),
                            found: f_ty,
                            expr: expr.clone().into(),
                        });
                    }
                }

                Ok(*ret_ty.clone())
            }

            Array(elems) => {
                let elem_tys = elems.iter().map(|elem| self.get_expr_type(elem)).collect::<Result<Vec<_>, _>>()?;
                let first_elem_ty = elem_tys.first().cloned().ok_or(MismatchType {
                    expected: Type::Unit,
                    found: Type::Unit,
                    expr: expr.clone().into(),
                })?;

                if !elem_tys.iter().all(|ty| self.type_equals(ty, &first_elem_ty)) {
                    return Err(MismatchType {
                        expected: first_elem_ty.clone(),
                        found: Type::Unit,
                        expr: expr.clone().into(),
                    });
                }

                Ok(first_elem_ty.array(elems.len()))
            }

            Struct(fields) => {
                let field_tys = fields.iter().map(|(name, value)| {
                    self.get_expr_type(value).map(|ty| (name.clone(), ty))
                }).collect::<Result<BTreeMap<_, _>, _>>()?;

                Ok(Type::Struct(field_tys))
            }

            Enum(ty, variant) => {
                // Check if the enum type is actually an enum
                let ty = self.reduce_type(ty);
                if !ty.is_enum() {
                    return Err(MismatchType {
                        expected: Type::Enum(BTreeSet::new()),
                        found: ty,
                        expr: expr.clone().into(),
                    });
                }

                // Check if the variant is actually a variant of the enum
                if let Type::Enum(variants) = &ty {
                    if !variants.contains(variant) {
                        return Err(MismatchType {
                            expected: Type::Enum(variants.clone()),
                            found: Type::Unit,
                            expr: expr.clone().into(),
                        });
                    }
                } else {
                    return Err(MismatchType {
                        expected: Type::Enum(BTreeSet::new()),
                        found: ty,
                        expr: expr.clone().into(),
                    });
                }

                Ok(ty)
            }

            Union(ty, variant, value) => {
                // Check if the union type is actually a union
                let ty = self.reduce_type(ty);
                if !ty.is_union() {
                    return Err(MismatchType {
                        expected: Type::Union(BTreeMap::new()),
                        found: ty,
                        expr: expr.clone().into(),
                    });
                }

                // Check if the variant is actually a variant of the union
                let value_ty = self.get_expr_type(value)?;
                if let Type::Union(variants) = &ty {
                    if !variants.contains_key(variant) {
                        return Err(MismatchType {
                            expected: Type::Union(variants.clone()),
                            found: Type::Unit,
                            expr: expr.clone().into(),
                        });
                    }

                    let expected_ty = variants.get(variant).unwrap();
                    if !self.type_equals(&value_ty, expected_ty) {
                        return Err(MismatchType {
                            expected: expected_ty.clone(),
                            found: value_ty,
                            expr: expr.clone().into(),
                        });
                    }
                } else {
                    return Err(MismatchType {
                        expected: Type::Union(BTreeMap::new()),
                        found: ty,
                        expr: expr.clone().into(),
                    });
                }

                Ok(ty)
            }
        }
    }

    /// Returns the cell index of the field in the struct
    /// 
    /// In the `mage` output, this will be used to calculate the offset of the field in the struct
    pub fn get_field_offset(&self, ty: &Type, desired_field_name: &Symbol) -> Result<usize, CheckError> {
        use Type::*;
        match self.reduce_type(ty) {
            Struct(fields) => {
                let mut idx = 0;
                for (field_name, field_ty) in fields {
                    if field_name == *desired_field_name {
                        return Ok(idx);
                    }
                    idx += self.get_type_size(&field_ty);
                }
                Err(CheckError::FieldNotFound {
                    container: ty.clone(),
                    name: desired_field_name.clone(),
                    expr: Stmt::Expr(Expr::Var(desired_field_name.clone())).into(),
                })
            },
            Union(..) => Ok(0),
            _ => {
                Err(CheckError::SelectNonStruct {
                    container: ty.clone(),
                    field: desired_field_name.clone(),
                    expr: Stmt::Expr(Expr::Var(desired_field_name.clone())).into(),
                })
            }
        }
    }

    /// Get the size of the type, in 64 bit integers
    pub fn get_type_size(&self, ty: &Type) -> usize {
        use Type::*;
        match ty {
            Int => 1,
            Float => 1,
            Bool => 1,
            Char => 1,
            Unit => 0,

            Named(name) => self.get_type_size(self.types.get(name).unwrap_or(ty)),

            Pointer(_, _) => 1,

            Array(ty, len) => self.get_type_size(ty) * len,

            Struct(fields) => fields.values().map(|ty| self.get_type_size(ty)).sum(),
            Union(fields) => fields.values().map(|ty| self.get_type_size(ty)).max().unwrap_or(0),

            Enum(_) => 1,

            Procedure(_, _) => 1,
        }
    }

    /// Get the size of an expression, in 64 bit integers
    pub fn get_expr_size(&self, expr: &Expr) -> Result<usize, CheckError> {
        let ty = self.get_expr_type(expr)?;
        Ok(self.get_type_size(&ty))
    }

    /// Get the size of a variable, in 64 bit integers
    pub fn get_var_size(&self, name: impl Into<Symbol>) -> Result<usize, CheckError> {
        let ty = self.get_var_type(name)?;
        Ok(self.get_type_size(&ty))
    }

    pub fn check(&mut self, stmt: &Stmt) -> Result<(), CheckError> {
        self.check_helper(stmt, &None)
    }

    fn check_helper(&mut self, stmt: &Stmt, expected_ret_ty: &Option<Type>) -> Result<(), CheckError> {
        match stmt {
            Stmt::Annotated(metadata, stmt) => {
                self.check_helper(stmt.strip_annotations(), expected_ret_ty).with_context(metadata.clone())
            }

            Stmt::Expr(expr) => {
                self.get_expr_type(&expr).map(|_| ())
            }

            Stmt::Return(value) => {
                let ret_ty = self.get_expr_type(value)?;
                if let Some(expected_ret_ty) = expected_ret_ty {
                    if !self.type_equals(&ret_ty, expected_ret_ty) {
                        return Err(CheckError::MismatchType {
                            expected: expected_ret_ty.clone(),
                            found: ret_ty,
                            expr: stmt.clone(),
                        });
                    }
                }
                Ok(())
            }

            Stmt::Continue | Stmt::Break => Ok(()),

            Stmt::DeclareVar { mutability, name, is_static, ty, value } => {
                let value_ty = self.get_expr_type(value)?;
                let ty = ty.as_ref().unwrap_or(&value_ty);
                if !self.type_equals(ty, &value_ty) {
                    return Err(CheckError::MismatchType {
                        expected: ty.clone(),
                        found: value_ty,
                        expr: stmt.clone(),
                    });
                }
                self.add_var(*is_static, name.clone(), *mutability, value_ty);
                Ok(())
            }

            Stmt::DeclareProc(proc) => {
                let mut proc = proc.clone();
                for (_, _, ty) in &mut proc.args {
                    *ty = self.reduce_type(ty);
                }
                proc.ret_ty = proc.ret_ty.map(|ty| self.reduce_type(&ty));
                self.add_proc(proc.clone());

                // Check the body of the procedure
                let mut new_env = self.new_function_scope();
                // Add the arguments to the new environment
                for (mutability, name, ty) in &proc.args {
                    new_env.add_var(false, name.clone(), *mutability, ty.clone());
                }
                // Check the body of the procedure
                println!("\n\nChecking {proc:?} in scope:\n{new_env:#?}\n\n");
                new_env.check_helper(&proc.body, &proc.ret_ty)
            }

            Stmt::DeclareType(name, ty) => {
                self.add_type(name.clone(), ty.clone());
                Ok(())
            }

            Stmt::ExternProc(proc) => {
                let mut proc = proc.clone();
                for (mutability, _, ty) in &mut proc.args {
                    *ty = self.reduce_type(ty);
                    if *mutability == Mutability::Immutable && ty.is_pointer() {
                        *mutability = Mutability::Mutable;
                    }
                }
                proc.ret_ty = proc.ret_ty.map(|ty| self.reduce_type(&ty));
                self.add_extern_proc(proc.clone());
                Ok(())
            }

            Stmt::AssignVar(name, value) => {
                let found_ty = self.get_expr_type(value)?;
                let (expected_mutability, expected_ty) = self.get_var(name.clone())?;
                
                if !expected_mutability.can_use_as(Mutability::Mutable) {
                    return Err(CheckError::MismatchMutability {
                        expected: Mutability::Mutable,
                        found: expected_mutability,
                        expr: stmt.clone(),
                    });
                }

                if !self.type_equals(&found_ty, &expected_ty) {
                    return Err(CheckError::MismatchType {
                        expected: expected_ty,
                        found: found_ty,
                        expr: stmt.clone(),
                    })
                }

                Ok(())
            }

            Stmt::AssignRef(dst, src) => {
                let dst_ty = self.get_expr_type(dst)?;
                let src_ty = self.get_expr_type(src)?;

                let dst_ty_deref = self.reduce_type(&dst_ty).deref(Mutability::Mutable).ok_or(CheckError::MismatchType {
                    expected: Type::Pointer(Mutability::Mutable, Box::new(Type::Unit)),
                    found: dst_ty,
                    expr: stmt.clone(),
                })?;

                if !self.type_equals(&dst_ty_deref, &src_ty) {
                    return Err(CheckError::MismatchType {
                        expected: dst_ty_deref.clone(),
                        found: src_ty,
                        expr: stmt.clone(),
                    });
                }

                Ok(())
            }

            Stmt::While(cond, body) => {
                let cond_ty = self.get_expr_type(cond)?;
                if !self.type_equals(&cond_ty, &Type::Bool) {
                    return Err(CheckError::MismatchType {
                        expected: Type::Bool,
                        found: cond_ty,
                        expr: stmt.clone(),
                    });
                }
                self.check_helper(body, expected_ret_ty)
            }

            Stmt::If(cond, then, else_) => {
                let cond_ty = self.get_expr_type(cond)?;
                if !self.type_equals(&cond_ty, &Type::Bool) {
                    return Err(CheckError::MismatchType {
                        expected: Type::Bool,
                        found: cond_ty,
                        expr: stmt.clone(),
                    });
                }
                self.check_helper(then, expected_ret_ty)?;
                self.check_helper(else_, expected_ret_ty)
            }

            Stmt::Block(stmts) => {
                let mut env = self.new_local_scope();
                for stmt in stmts {
                    env.check_helper(stmt, expected_ret_ty)?;
                }
                println!("\n\nChild scope:\n{env:#?}\n\n");
                Ok(())
            }
        }
    }
}

impl Default for Env {
    fn default() -> Self {
        Self {
            locals: HashMap::new(),
            statics: HashMap::new(),
            procs: HashMap::new(),
            types: HashMap::new(),
            extern_procs: HashMap::new(),
            scope: ID::create(),
        }
    }
}

#[cfg(test)]
mod tests {
    use nom::error::VerboseError;

    use super::*;

    #[test]
    fn test_symbol() {
        let symbol1 = Symbol::new("test");
        let symbol2 = Symbol::new("test");
        assert_eq!(symbol1, symbol2);
    }

    #[test]
    fn test_get_type() {
        let mut env = Env::default();
        env.types.insert("int".into(), Type::Int);
        env.types.insert("N".into(), Type::Int);
        env.types.insert("float".into(), Type::Float);

        assert_eq!(env.get_type("int").unwrap(), Type::Int);
        assert_eq!(env.get_type("float").unwrap(), Type::Float);
        assert!(env.get_type("string").is_err());

        println!("\n\n{:#?}\n\n", env);
        assert!(env.type_equals(&Type::Named("N".into()), &Type::Named("int".into())));
    }

    #[test]
    fn test_parse_struct() {
        let input = r#"struct { x: Int, y: Int }"#;

        let (rest, ty) = parse_struct_type::<VerboseError<&str>>(input).unwrap();
        println!("{:#?}", ty);
    }

    #[test]
    fn test_check_stmt() {
        let input = r#"
        type Point = struct { x: Int, y: Int, z: Int };
        let mut p: Point = { x: 1, y: 2, z: 3 };

        extern fun putint(n: Int);
        extern fun putstr(s: &Char);

        fun putpoint(p: Point) {
            putstr("Point { x: ");
            putint(p.x);
            putstr(", y: ");
            putint(p.y);
            putstr(", z: ");
            putint(p.z);
            putstr("}");
        }

        fun make_point(x: Int, y: Int, z: Int) -> Point {
            return { x, y, z };
        }

        p = { x: 3, y: 4, z: 5 };
        putpoint(p);
        p.x = 5;
        putpoint(p);
        "#;

        let program = match parse(input) {
            Ok(program) => program,
            Err(e) => panic!("Failed to parse: {e:#?}"),
        };
        println!("{:#?}", program);
        let mut ctx = Env::default();
        match ctx.check(&program) {
            Ok(_) => println!("Check successful"),
            Err(e) => println!("Check failed: {e:#?}"),
        }
        println!("{:#?}\n\n", ctx);
    }


    #[test]
    fn test_to_mage() {
        let input = r#"
        type Point = struct { x: Int, y: Int, z: Int };
        let p = { x: 1, y: 2, z: 3 };
        
        extern fun puti(n: Int);
        extern fun putc(c: Char);
        extern fun addi(X: Int, Y: Int) -> Int;

        fun test(x: Int, y: Int) -> Int {
            let z = addi(x, y);
            return z;
        }

        fun test2(p: Point) {
            puti(p.x);
            putc(' ');
            puti(p.y);
            putc(' ');
            puti(p.z);
            putc('\n');
        }

        fun shift(mut p: Point, dx: Int, dy: Int) -> Point {
            p.x = addi(p.x, dx);
            p.y = addi(p.y, dy);
            return p;
        }

        puti(test(1, 2));
        putc('\n');

        test2(p);
        
        test2(shift(p, 1, 2));
        "#;

        let program = match parse(input) {
            Ok(program) => program,
            Err(e) => panic!("Failed to parse: {e:#?}"),
        };
        println!("{:#?}", program);
        let mut ctx = Env::default();
        match ctx.check(&program) {
            Ok(_) => println!("Check successful"),
            Err(e) => println!("Check failed: {e:#?}"),
        }
        // println!("{:#?}\n\n", ctx);

        // // Compile to mage
        let mut ctx = Env::default();
        let mage = program.compile_to_mage(&mut ctx).unwrap();
        println!("MAGE:\n\n{}", mage);

        // Write to `test.mg`
        std::fs::write("test.mg", mage).unwrap();
    }
}