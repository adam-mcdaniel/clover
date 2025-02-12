use lazy_static::lazy_static;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet}, sync::{Mutex, RwLock},
    fmt::{Debug, Display, Formatter, Result as FmtResult},
};
use anyhow::Result;
use thiserror::Error;
use tracing::*;

mod symbol;
pub use symbol::*;

mod parser;
pub use parser::*;

pub mod codegen;

mod interpreter;
pub use interpreter::*;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

impl Display for Mutability {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Mutability::Mutable => write!(f, "mutable"),
            Mutability::Immutable => write!(f, "immutable"),
        }
    }
}

impl Debug for Mutability {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
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

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Type {
    Named(Symbol),
    Int,
    Float,
    Bool,
    Char,
    Cell,
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
        matches!(self, Type::Int | Type::Float | Type::Bool | Type::Char | Type::Cell | Type::Unit)
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

    pub fn get_element_type(&self) -> Option<&Type> {
        match self {
            Type::Pointer(_, ty) => Some(ty),
            Type::Array(ty, _) => Some(ty),
            _ => None,
        }
    }

    fn get_variant_index(&self, variant: &Symbol) -> Result<usize, CheckError> {
        use Type::*;
        match self {
            Enum(variants) => {
                variants.iter().position(|v| v == variant).ok_or(CheckError::FieldNotFound {
                    container: self.clone(),
                    name: variant.clone(),
                    expr: Stmt::Expr(Expr::Var(variant.clone())).into(),
                })
            },
            _ => Err(CheckError::VariantNotFound {
                container: self.clone(),
                variant: variant.clone(),
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


impl Display for Type {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Type::Named(name) => write!(f, "{}", name),
            Type::Int => write!(f, "Int"),
            Type::Float => write!(f, "Float"),
            Type::Bool => write!(f, "Bool"),
            Type::Char => write!(f, "Char"),
            Type::Cell => write!(f, "Cell"),
            Type::Unit => write!(f, "Unit"),
            Type::Pointer(Mutability::Immutable, ty) => write!(f, "&{}", ty),
            Type::Pointer(Mutability::Mutable, ty) => write!(f, "&mut {}", ty),
            Type::Array(ty, size) => write!(f, "[{} * {}]", ty, size),
            Type::Struct(fields) => {
                write!(f, "struct {{")?;
                for (i, (field, ty)) in fields.iter().enumerate() {
                    write!(f, "{}: {}", field, ty)?;
                    if i < fields.len() - 1 {
                        write!(f, ", ")?;
                    }
                }
                write!(f, "}}")
            }
            Type::Enum(variants) => {
                write!(f, "enum {{")?;
                for (i, variant) in variants.iter().enumerate() {
                    write!(f, "{}", variant)?;
                    if i < variants.len() - 1 {
                        write!(f, ", ")?;
                    }
                }
                write!(f, "}}")
            }
            Type::Union(fields) => {
                write!(f, "union {{")?;
                for (i, (field, ty)) in fields.iter().enumerate() {
                    write!(f, "{}: {}", field, ty)?;
                    if i < fields.len() - 1 {
                        write!(f, ", ")?;
                    }
                }
                write!(f, "}}")
            }
            Type::Procedure(args, ret) => {
                write!(f, "fun(")?;
                for (i, arg) in args.iter().enumerate() {
                    write!(f, "{}", arg)?;
                    if i < args.len() - 1 {
                        write!(f, ", ")?;
                    }
                }
                write!(f, ") -> {}", ret)
            }
        }
    }
}

impl Debug for Type {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
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
    Description(String),
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
            Metadata::Description(description) => {
                write!(f, "{}", description)
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

impl From<&str> for Metadata {
    fn from(desc: &str) -> Self {
        Metadata::Description(desc.to_string())
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
    Unit,
    Str(String),
    CStr(String),
    Var(Symbol),
    Ref(Mutability, Box<Self>),
    // RefSelect(Mutability, Symbol, Symbol),
    App(Box<Self>, Vec<Self>),
    Array(Vec<Self>),
    Cast(Box<Self>, Type),

    Select(Box<Self>, Symbol),
    Index(Box<Self>, Box<Self>),
    Deref(Box<Self>),

    Struct(BTreeMap<Symbol, Self>),
    Enum(Type, Symbol),
    Union(Type, Symbol, Box<Self>),

    LengthOfExpr(Box<Self>),
    LengthOfType(Type),
    SizeOfExpr(Box<Self>),
    SizeOfType(Type),
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

impl Display for Expr {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        use Expr::*;
        match self.strip_annotations() {
            SizeOfExpr(value) => write!(f, "sizeof({})", value),
            SizeOfType(ty) => write!(f, "sizeof<{}>()", ty),
            LengthOfExpr(value) => write!(f, "lengthof({})", value),
            LengthOfType(ty) => write!(f, "lengthof<{}>()", ty),
            Unit => write!(f, "()"),
            Int(value) => write!(f, "{}", value),
            Char(value) => write!(f, "{}", value),
            Float(value) => write!(f, "{}", value),
            Bool(value) => write!(f, "{}", value),
            Str(value) => write!(f, "{:?}", value),
            CStr(value) => write!(f, "c{:?}", value),
            Cast(value, ty) => write!(f, "({} as {})", value, ty),
            If(cond, then, else_) => write!(f, "if {} {} else {}", cond, then, else_),
            Var(name) => write!(f, "{}", name),
            Ref(mutability, name) => {
                if *mutability == Mutability::Immutable {
                    write!(f, "&({})", name)
                } else {
                    write!(f, "&mut ({})", name)
                }
            }
            // RefSelect(mutability, container, field) => {
            //     if *mutability == Mutability::Immutable {
            //         write!(f, "&{}.{}", container, field)
            //     } else {
            //         write!(f, "&mut {}.{}", container, field)
            //     }
            // }
            App(func, args) => {
                write!(f, "{}(", func)?;
                for (i, arg) in args.iter().enumerate() {
                    write!(f, "{}", arg)?;
                    if i < args.len() - 1 {
                        write!(f, ", ")?;
                    }
                }
                write!(f, ")")
            }
            Array(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    write!(f, "{}", item)?;
                    if i < items.len() - 1 {
                        write!(f, ", ")?;
                    }
                }
                write!(f, "]")
            }
            Select(container, field) => write!(f, "({}).{}", container, field),
            Index(container, index) => write!(f, "({})[{}]", container, index),
            Deref(value) => write!(f, "*{}", value),
            Struct(fields) => {
                write!(f, "{{")?;
                for (field, value) in fields {
                    write!(f, "{}: {}, ", field, value)?;
                }
                write!(f, "}}")
            }
            Enum(_, variant) => write!(f, "{}", variant),
            Union(_, variant, value) => write!(f, "{}({})", variant, value),
            Annotated(..) => unreachable!(),
        }
    }
}

pub fn ref_(mutability: impl Into<Mutability>, expr: impl Into<Expr>) -> Expr {
    Expr::Ref(mutability.into(), Box::new(expr.into()))
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

#[derive(Clone)]
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

impl Display for Procedure {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "fun {}(", self.name)?;
        for (i, (mutability, name, ty)) in self.args.iter().enumerate() {
            write!(f, "{} {}: {} ", mutability, name, ty)?;
            if i < self.args.len() - 1 {
                write!(f, ", ")?;
            }
        }
        write!(f, ") -> {}", self.ret_ty.clone().unwrap_or(Type::Unit))?;
        write!(f, " {}", self.body)
    }
}

impl Debug for Procedure {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
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

impl Display for ExternProcedure {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "extern fun {}(", self.name)?;
        for (mutability, name, ty) in &self.args {
            write!(f, "{} {}: {}, ", mutability, name, ty)?;
        }
        write!(f, ") -> {}", self.ret_ty.clone().unwrap_or(Type::Unit))?;
        if let Some(body) = &self.body {
            write!(f, " {}", body)
        } else {
            write!(f, ";")?;
            Ok(())
        }
    }
}

#[derive(Clone)]
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

impl Display for Stmt {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        use Stmt::*;
        match self.strip_annotations() {
            Expr(expr) => write!(f, "{};", expr),
            Return(expr) => write!(f, "return {};", expr),
            Continue => write!(f, "continue;"),
            Break => write!(f, "break;"),
            DeclareVar { mutability: Mutability::Immutable, name, is_static, ty, value } => {
                write!(f, "let ")?;
                if *is_static {
                    write!(f, "static ")?;
                }
                write!(f, "{name}")?;
                if let Some(ty) = ty {
                    write!(f, ": {ty}")?;
                }
                write!(f, " = {value};")
            }
            DeclareVar { mutability: Mutability::Mutable, name, is_static, ty, value } => {
                write!(f, "let ")?;
                if *is_static {
                    write!(f, "static ")?;
                }
                write!(f, "mut {name}")?;
                if let Some(ty) = ty {
                    write!(f, ": {ty}")?;
                }
                write!(f, " = {value};")
            }

            DeclareProc(proc) => write!(f, "{};", proc),
            DeclareType(name, ty) => write!(f, "type {} = {};", name, ty),
            ExternProc(proc) => write!(f, "{}", proc),
            AssignVar(name, value) => write!(f, "{} = {};", name, value),
            AssignRef(dst, src) => write!(f, "{} = {};", dst, src),
            While(cond, body) => write!(f, "while {} {};", cond, body),
            If(cond, then, else_) => write!(f, "if {} {} else {}", cond, then, else_),
            Block(stmts) => {
                write!(f, "{{")?;
                for stmt in stmts {
                    write!(f, "{} ", stmt)?;
                }
                write!(f, "}}")
            }
            _ => unreachable!()
        }
    }
}

impl Debug for Stmt {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self, f)
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

#[derive(Error, Debug, Clone)]
pub enum CheckError {
    #[error("Mismatched mutability in {expr} (expected {expected}, but found {found})")]
    MismatchMutability {
        expected: Mutability,
        found: Mutability,
        expr: Stmt,
    },
    #[error("Mismatched types in {expr} (expected {expected}, but found {found})")]
    MismatchType {
        expected: Type,
        found: Type,
        expr: Stmt,
    },
    #[error("Field \"{name}\" not found {expr} (found type {container})")]
    FieldNotFound {
        container: Type,
        name: Symbol,
        expr: Stmt,
    },
    #[error("Tried to get length of non-array type {ty} used in {expr}")]
    LengthOfNonArray {
        ty: Type,
        expr: Stmt,
    },
    #[error("Tried to index a non-array type {ty} used in {expr}")]
    IndexNonArray {
        ty: Type,
        expr: Stmt,
    },
    #[error("Variable \"{name}\" not found in {expr}")]
    VariableNotFound {
        name: Symbol,
        expr: Stmt,
    },
    #[error("Variant \"{variant}\" not found in type {container} used in {expr}")]
    VariantNotFound {
        container: Type,
        variant: Symbol,
        expr: Stmt,
    },
    #[error("Tried to get field \"{field}\" of non-struct type {container} used in {expr}")]
    SelectNonStruct {
        container: Type,
        field: Symbol,
        expr: Stmt,
    },
    #[error("Type {ty} has infinite size in {expr}")]
    InfiniteSize {
        ty: Type,
        expr: Stmt,
    },
    #[error("Tried to take invalid reference of {expr} in {stmt}")]
    InvalidRef {
        expr: Expr,
        stmt: Stmt,
    },
    #[error("Type {0} not found")]
    TypeNotFound(Symbol),
    #[error("Function {0} not found")]
    ProcNotFound(Symbol),
    #[error("{0} ({1})")]
    WithMetadata(Box<CheckError>, Metadata),

    /// An invalid assignment
    #[error("Invalid assignment in {stmt}")]
    InvalidAssignment {
        stmt: Stmt,
    },
}

pub trait WithMetadata {
    fn with_metadata(self, metadata: impl Into<Metadata>) -> Self;
}

impl WithMetadata for CheckError {
    fn with_metadata(self, metadata: impl Into<Metadata>) -> Self {
        CheckError::WithMetadata(Box::new(self), metadata.into())
    }
}

impl<T> WithMetadata for Result<T, CheckError> {
    fn with_metadata(self, metadata: impl Into<Metadata>) -> Self {
        self.map_err(|e| e.with_metadata(metadata))
    }
}

#[derive(Debug, Clone)]
pub struct Env {
    types: HashMap<Symbol, Type>,
    locals: HashMap<Symbol, (Mutability, Type)>,
    statics: HashMap<Symbol, (Mutability, Type)>,
    procs: HashMap<Symbol, Procedure>,
    extern_procs: HashMap<Symbol, ExternProcedure>,
}

impl Env {
    pub fn new_function_scope(&self) -> Self {
        Self {
            types: self.types.clone(),
            locals: HashMap::new(),
            statics: self.statics.clone(),
            procs: self.procs.clone(),
            extern_procs: self.extern_procs.clone(),
        }
    }

    pub fn new_local_scope(&self) -> Self {
        Self {
            types: self.types.clone(),
            locals: self.locals.clone(),
            statics: self.statics.clone(),
            procs: self.procs.clone(),
            extern_procs: self.extern_procs.clone(),
        }
    }

    pub fn add_type(&mut self, name: impl Into<Symbol>, ty: Type) -> Result<(), CheckError> {
        let name = name.into();
        self.types.insert(name.clone(), ty.clone());
        self.detect_infinite_size_helper(&ty, &Stmt::DeclareType(name.clone(), ty.clone()), &mut HashSet::new())
    }

    fn detect_infinite_size_helper(&self, ty: &Type, expr: &Stmt, visited: &mut HashSet<Type>) -> Result<(), CheckError> {
        if visited.contains(ty) {
            return Err(CheckError::InfiniteSize {
                ty: ty.clone(),
                expr: expr.clone(),
            });
        }

        match ty {
            Type::Named(name) => {
                // Add the type to the visited set
                visited.insert(ty.clone());

                if let Some(ty) = self.types.get(name) {
                    self.detect_infinite_size_helper(ty, expr, visited)
                } else {
                    Ok(())
                }
            }
            Type::Array(ty, _) => {
                self.detect_infinite_size_helper(ty, expr, visited)
            }
            Type::Struct(fields) => {
                for (_, ty) in fields {
                    self.detect_infinite_size_helper(ty, expr, visited)?;
                }
                Ok(())
            }
            Type::Union(fields) => {
                for (_, ty) in fields {
                    self.detect_infinite_size_helper(ty, expr, visited)?;
                }
                Ok(())
            }
            Type::Procedure(args, ret) => {
                for arg in args {
                    self.detect_infinite_size_helper(arg, expr, visited)?;
                }
                self.detect_infinite_size_helper(ret, expr, visited)
            }
            Type::Pointer(..) | Type::Enum(..) | Type::Bool | Type::Char | Type::Float | Type::Int | Type::Cell | Type::Unit => Ok(()),
        }
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
            (Named(a_name), Named(b_name)) => {
                a_name == b_name || self.can_coerce_to(self.types.get(a_name).unwrap_or(found), desired)
            }
            (Named(a_name), _) => {
                if let Some(ty) = self.types.get(a_name) {
                    self.can_coerce_to(ty, desired)
                } else {
                    false
                }
            }
            (_, Named(b_name)) => {
                if let Some(ty) = self.types.get(b_name) {
                    self.can_coerce_to(found, ty)
                } else {
                    false
                }
            }
            (Array(t1, len1), Array(t2, len2)) => {
                if len1 != len2 {
                    return false;
                }
                self.can_coerce_to(t1, t2)
            },
            (Array(t1, _), Pointer(_, t2)) => self.can_coerce_to(t1, t2),
            (Pointer(m1, t1), Pointer(m2, t2)) => {
                // Check if the found type is an array
                m1.can_use_as(*m2)
                && (match &**t1 {
                    Type::Array(elem_ty, _) => {
                        // If it is, we can coerce it to a pointer
                        self.can_coerce_to(&elem_ty, t2)
                        || self.can_coerce_to(t1, t2)
                    }
                    _ => {
                        // Otherwise, we need to check if the mutability matches
                        self.can_coerce_to(t1, t2)
                    }
                })
            },

            (Struct(fields1), Struct(fields2)) => {
                fields1.len() == fields2.len()
                && fields1.iter().all(|(name, ty1)| {
                    fields2.get(name).map_or(false, |ty2| self.can_coerce_to(ty1, ty2))
                })
            }
            (Enum(variants1), Enum(variants2)) => {
                variants1 == variants2
            }

            (Union(fields1), Union(fields2)) => {
                fields1.len() == fields2.len()
                && fields1.iter().all(|(name, ty1)| {
                    fields2.get(name).map_or(false, |ty2| self.can_coerce_to(ty1, ty2))
                })
            }

            (Procedure(args1, ret1), Procedure(args2, ret2)) => {
                args1.len() == args2.len()
                && self.can_coerce_to(ret1, ret2)
                && args1.iter().zip(args2).all(|(a1, a2)| self.can_coerce_to(a1, &a2))
            }

            (Union(variants), found)  | (found, Union(variants)) => {
                if variants.iter().find(|(_, variant)| self.can_coerce_to(found, variant)).is_some() {
                    match (self.get_type_size(found), self.get_type_size(desired)) {
                        (Ok(found_size), Ok(desired_size)) => {
                            if found_size != desired_size {
                                return false;
                            }
                        }
                        _ => {}
                    }
                    true
                } else {
                    false
                }
            }

            (a, b) if a.is_primitive() && b.is_primitive() => {
                match (self.get_type_size(a), self.get_type_size(b)) {
                    (Ok(a_size), Ok(b_size)) => a_size == b_size,
                    _ => false,
                }
            }
            
            (a, b) => {
                debug!("Type mismatch, cannot coerce: {:?} != {:?}", a, b);
                false
            },
        }
    }

    pub fn type_equals(&self, found: &Type, desired: &Type) -> bool {
        trace!("ROOT CHECK: {:?} == {:?}", found, desired);
        let result = self.type_equals_helper(found, desired, 20);
        trace!("\n\n\n");
        result
    }

    fn type_equals_helper(&self, found: &Type, desired: &Type, mut depth: usize) -> bool {
        use Type::*;
        let a = found;
        let b = desired;
        trace!("Comparing {:?} with {:?} at depth {}", a, b, depth);
        depth -= 1;
        if a == b {
            trace!("Types {a:?} and {b:?} are trivially equal");
            return true;
        }
        if depth == 0 {
            trace!("Types {a:?} and {b:?} are too deep to compare");
            return false;
        }
        match (a, b) {
            (Int, Int) => true,
            (Float, Float) => true,
            (Bool, Bool) => true,
            (Char, Char) => true,
            (Unit, Unit) => true,

            (Named(a_name), Named(b_name)) => {
                a_name == b_name || self.type_equals_helper(self.types.get(a_name).unwrap_or(a), b, depth)
            }
            (Named(a_name), _) => {
                if let Some(ty) = self.types.get(a_name) {
                    self.type_equals_helper(ty, b, depth)
                } else {
                    false
                }
            }
            (_, Named(b_name)) => {
                if let Some(ty) = self.types.get(b_name) {
                    self.type_equals_helper(a, ty, depth)
                } else {
                    false
                }
            }

            (Struct(a_fields), Struct(b_fields)) => {
                a_fields.len() == b_fields.len() && a_fields.iter().all(|(name, ty)| {
                    b_fields.get(name).map_or(false, |b_ty| self.type_equals_helper(ty, b_ty, depth))
                })
            }

            (Enum(a_variants), Enum(b_variants)) => a_variants == b_variants,

            (Union(a_fields), Union(b_fields)) => {
                a_fields.len() == b_fields.len() && a_fields.iter().all(|(name, ty)| {
                    b_fields.get(name).map_or(false, |b_ty| self.type_equals_helper(ty, b_ty, depth))
                })
            }

            (Procedure(a_args, a_ret), Procedure(b_args, b_ret)) => {
                a_args.len() == b_args.len() && a_ret == b_ret && a_args.iter().zip(b_args).all(|(a, b)| self.type_equals_helper(a, &b, depth))
            }

            (Array(a_ty, a_size), Array(b_ty, b_size)) => a_size == b_size && self.type_equals_helper(a_ty, b_ty, depth),

            (Pointer(a_mut, a_ty), Pointer(b_mut, b_ty)) => {
                a_mut.can_use_as(*b_mut)
                && (self.type_equals_helper(a_ty, b_ty, depth)
                    || (match (a_ty.get_element_type(), b_ty.get_element_type()) {
                        (Some(a_elem_ty), Some(b_elem_ty)) => self.type_equals_helper(a_elem_ty, b_elem_ty, depth),
                        (Some(a_elem_ty), _) => self.type_equals_helper(a_elem_ty, b_ty, depth),
                        (_, Some(b_elem_ty)) => self.type_equals_helper(a_ty, b_elem_ty, depth),
                        _ => false,
                    }))

            },

            _ => false,
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
            Int | Float | Bool | Char | Cell | Unit => ty.clone(),
        }
    }

    pub fn get_expr_type(&self, expr: &Expr) -> Result<Type, CheckError> {
        use Expr::*;
        use CheckError::*;
        match expr {
            LengthOfExpr(expr) => {
                let ty = self.get_expr_type(expr)?;
                // Confirm the type is an array
                match ty {
                    Type::Array(_, _) => Ok(Type::Int),
                    _ => Err(LengthOfNonArray {
                        ty,
                        expr: expr.clone().into(),
                    }).with_metadata("Getting length of non-array type"),
                }
            }
            LengthOfType(ty) => {
                // Confirm the type is an array
                match ty {
                    Type::Array(_, _) => Ok(Type::Int),
                    _ => Err(LengthOfNonArray {
                        ty: ty.clone(),
                        expr: expr.clone().into(),
                    }).with_metadata("Getting length of non-array type"),
                }
            }
            SizeOfExpr(expr) => {
                let ty = self.get_expr_type(expr)?;
                let _size = self.get_type_size(&ty)?;
                Ok(Type::Int)
            }
            SizeOfType(ty) => {
                let _size = self.get_type_size(ty)?;
                Ok(Type::Int)
            },

            Var(name) => {
                match self.get_var_type(name.clone()) {
                    Ok(ty) => Ok(ty),
                    Err(e) => {
                        trace!("Could not find variable {name:?} in scope {self:#?}");
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
                }).with_metadata("Dereferencing a non-pointer type")
            }

            Ref(desired_mutability, val) => {

                match val.strip_annotations() {
                    Var(name) => {
                        let (found_mutability, ty) = self.get_var(name.clone())?;
                        if !found_mutability.can_use_as(*desired_mutability) {
                            return Err(MismatchMutability {
                                expected: *desired_mutability,
                                found: found_mutability,
                                expr: expr.clone().into(),
                            }).with_metadata("Taking reference of variable with mismatched mutability");
                        }

                        Ok(ty.refer(*desired_mutability))
                    },
                    Select(expr, _) => {
                        // Try to get the type a reference to inner value
                        let _ref_expr = Expr::Ref(*desired_mutability, expr.clone());
                        let ty = self.get_expr_type(&val)?;

                        Ok(ty.refer(*desired_mutability))
                    },
                    Index(_ptr, _) => {
                        let ty = self.get_expr_type(val)?;
                        Ok(ty.refer(*desired_mutability))
                    },
                    Deref(_ptr) => {
                        let ty = self.get_expr_type(val)?;
                        Ok(ty.refer(*desired_mutability))
                    },
                    other => {
                        error!("Taking reference of {other:?}");
                        Err(InvalidRef {
                            expr: other.clone(),
                            stmt: expr.clone().into(),
                        }).with_metadata("Taking reference of non-variable, non-deref, non-index, and non-select expression")
                    },
                }

            },
            // RefSelect(desired_mutability, container, name) => {
            //     // let container_ty = self.get_expr_type(&Expr::Var(container.clone()))?;
            //     let (found_container_mutability, container_ty) = self.get_var(container.clone())?;
            //     let container_ty = self.reduce_type(&container_ty);
            //     match &container_ty {
            //         Type::Struct(fields) | Type::Union(fields) => {
            //             if !found_container_mutability.can_use_as(*desired_mutability) {
            //                 return Err(MismatchMutability {
            //                     expected: *desired_mutability,
            //                     found: found_container_mutability,
            //                     expr: expr.clone().into(),
            //                 }).with_metadata("Taking reference of struct field with mismatched mutability");
            //             }
            //             let deref_ty = fields.get(name).cloned().ok_or(FieldNotFound {
            //                 container: container_ty,
            //                 name: name.clone(),
            //                 expr: expr.clone().into(),
            //             }).with_metadata("Field not found in struct")?;
            //             Ok(deref_ty.refer(*desired_mutability))
            //         },
            //         _ => Err(MismatchType {
            //             expected: Type::Struct(BTreeMap::new()),
            //             found: container_ty,
            //             expr: expr.clone().into(),
            //         }).with_metadata("Taking reference of non-struct type"),
            //     }
            // },

            Annotated(metadata, expr) => {
                self.get_expr_type(&expr.strip_annotations()).with_metadata(metadata.clone())
            }

            Select(struct_, field) => {
                // let struct_ = Expr::Var(struct_.clone());
                let struct_ty = self.get_expr_type(&struct_)?;
                let struct_ty = self.reduce_type(&struct_ty);
                match &struct_ty {
                    Type::Union(fields) | Type::Struct(fields) => {
                        fields.get(field).cloned().ok_or(FieldNotFound {
                            container: struct_ty,
                            name: field.clone(),
                            expr: expr.clone().into(),
                        }).with_metadata("Field not found in struct")
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
                    }).with_metadata("Tried to get field of non-struct type"),
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
                    }).with_metadata("Indexing array with non-integer type");
                }

                match array_ty {
                    Type::Array(ty, _) => Ok(*ty),
                    Type::Pointer(_, ty) => Ok(*ty),
                    _ => Err(IndexNonArray {
                        ty: array_ty,
                        expr: expr.clone().into(),
                    }).with_metadata("Indexing a non-array type"),
                }
            }

            If(cond, then, else_) => {
                let cond_ty = self.get_expr_type(cond)?;
                if !self.type_equals(&cond_ty, &Type::Bool) {
                    return Err(MismatchType {
                        expected: Type::Bool,
                        found: cond_ty,
                        expr: cond.clone().into(),
                    }).with_metadata("Condition of if statement is not a boolean");
                }
                let then_ty = self.get_expr_type(then)?;
                let else_ty = self.get_expr_type(else_)?;
                if !self.type_equals(&then_ty, &else_ty) {
                    return Err(MismatchType {
                        expected: then_ty,
                        found: else_ty,
                        expr: expr.clone().into(),
                    }).with_metadata("Branches of if statement have different types");
                }
                Ok(then_ty)
            }

            Unit => Ok(Type::Unit),
            Int(_) => Ok(Type::Int),
            Char(_) => Ok(Type::Char),
            Float(_) => Ok(Type::Float),
            Bool(_) => Ok(Type::Bool),
            Str(_) => Ok(Type::Char.refer(Mutability::Mutable)),
            CStr(_) => Ok(Type::Cell.refer(Mutability::Mutable)),

            Cast(cast_expr, ty) => {
                let expr_ty = self.get_expr_type(cast_expr)?;
                if !self.can_coerce_to(&expr_ty, ty) {
                    error!("Cannot cast {expr_ty:?} to {ty:?}");
                    return Err(MismatchType {
                        expected: ty.clone(),
                        found: expr_ty,
                        expr: expr.clone().into(),
                    }).with_metadata("Cannot cast expression to incompatible type");
                }
                Ok(ty.clone())
            }

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
                    }).with_metadata("Function application of non-function type"),
                };

                // Check if the number of arguments match
                if arg_tys.len() != param_tys.len() {
                    return Err(MismatchType {
                        expected: Type::Procedure(arg_tys.clone(), Box::new(Type::Unit)),
                        found: f_ty,
                        expr: expr.clone().into(),
                    }).with_metadata("Function application with wrong number of arguments");
                }

                // Check if the types of `args` match the params of `f`
                for (arg_ty, param_ty) in arg_tys.iter().zip(param_tys) {
                    if !self.type_equals(arg_ty, param_ty) {
                        return Err(MismatchType {
                            expected: param_ty.clone(),
                            found: arg_ty.clone(),
                            expr: expr.clone().into(),
                        }).with_metadata("Function application with wrong argument types");
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
                }).with_metadata("Empty array")?;

                if !elem_tys.iter().all(|ty| self.type_equals(ty, &first_elem_ty)) {
                    return Err(MismatchType {
                        expected: first_elem_ty.clone(),
                        found: Type::Unit,
                        expr: expr.clone().into(),
                    }).with_metadata("Array elements have different types");
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
                    }).with_metadata("Checking enum variant of non-enum type");
                }

                // Check if the variant is actually a variant of the enum
                if let Type::Enum(variants) = &ty {
                    if !variants.contains(variant) {
                        return Err(MismatchType {
                            expected: Type::Enum(variants.clone()),
                            found: Type::Unit,
                            expr: expr.clone().into(),
                        }).with_metadata("Enum does not contain variant");
                    }
                } else {
                    return Err(MismatchType {
                        expected: Type::Enum(BTreeSet::new()),
                        found: ty,
                        expr: expr.clone().into(),
                    }).with_metadata("Checking enum variant of non-enum type");
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
                    }).with_metadata("Checking union variant of non-union type");
                }

                // Check if the variant is actually a variant of the union
                let value_ty = self.get_expr_type(value)?;
                if let Type::Union(variants) = &ty {
                    if !variants.contains_key(variant) {
                        return Err(MismatchType {
                            expected: Type::Union(variants.clone()),
                            found: Type::Unit,
                            expr: expr.clone().into(),
                        }).with_metadata("Union does not contain variant");
                    }

                    let expected_ty = variants.get(variant).unwrap();
                    if !self.type_equals(&value_ty, expected_ty) {
                        return Err(MismatchType {
                            expected: expected_ty.clone(),
                            found: value_ty,
                            expr: expr.clone().into(),
                        }).with_metadata("Union variant does not match expected type");
                    }
                } else {
                    return Err(MismatchType {
                        expected: Type::Union(BTreeMap::new()),
                        found: ty,
                        expr: expr.clone().into(),
                    }).with_metadata("Checking union variant of non-union type");
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
                    idx += self.get_type_size(&field_ty)?;
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

    /// Returns the index of the variant in the enum
    pub fn get_variant_index(&self, ty: &Type, desired_variant_name: &Symbol) -> Result<usize, CheckError> {
        self.reduce_type(ty).get_variant_index(desired_variant_name)
    }

    /// Get the size of the type, in 64 bit integers
    pub fn get_type_size(&self, ty: &Type) -> Result<usize, CheckError> {
        use Type::*;
        Ok(match ty {
            Cell | Int | Float | Bool | Char => 1,
            Unit => 0,

            Named(name) => {
                let ty = self.get_type(name.clone())?;
                self.get_type_size(&ty)?
            },

            Pointer(_, _) => 1,

            Array(ty, len) => self.get_type_size(ty)? * len,

            Struct(fields) => fields.values().map(|ty| self.get_type_size(ty)).collect::<Result<Vec<_>, _>>()?.into_iter().sum(),
            Union(fields) => fields.values().map(|ty| self.get_type_size(ty)).collect::<Result<Vec<_>, _>>()?.into_iter().max().unwrap_or(0),

            Enum(_) => 1,

            Procedure(_, _) => 1,
        })
    }

    /// Get the size of an expression, in 64 bit integers
    pub fn get_expr_size(&self, expr: &Expr) -> Result<usize, CheckError> {
        let ty = self.get_expr_type(expr)?;
        self.get_type_size(&ty)
    }

    /// Get the size of a variable, in 64 bit integers
    pub fn get_var_size(&self, name: impl Into<Symbol>) -> Result<usize, CheckError> {
        let ty = self.get_var_type(name)?;
        self.get_type_size(&ty)
    }

    pub fn check(&mut self, stmt: &Stmt) -> Result<(), CheckError> {
        self.check_helper(stmt, &None)
    }

    fn check_helper(&mut self, stmt: &Stmt, expected_ret_ty: &Option<Type>) -> Result<(), CheckError> {
        match stmt {
            Stmt::Annotated(metadata, stmt) => {
                self.check_helper(stmt.strip_annotations(), expected_ret_ty).with_metadata(metadata.clone())
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
                        }).with_metadata("Return value does not match expected return type");
                    }
                }
                Ok(())
            }

            Stmt::Continue | Stmt::Break => Ok(()),

            Stmt::DeclareVar { mutability, name, is_static, ty, value } => {
                let value_ty = self.get_expr_type(value)?;
                let ty = &self.reduce_type(ty.as_ref().unwrap_or(&value_ty));
                if !self.type_equals(ty, &value_ty) {
                    return Err(CheckError::MismatchType {
                        expected: ty.clone(),
                        found: value_ty,
                        expr: stmt.clone(),
                    }).with_metadata("Declared variable type does not match initializer type");
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
                trace!("\n\nChecking {proc:?} in scope:\n{new_env:#?}\n\n");
                new_env.check_helper(&proc.body, &proc.ret_ty)
            }

            Stmt::DeclareType(name, ty) => {
                self.add_type(name.clone(), ty.clone())
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
                    }).with_metadata("Assignment type does not match variable type");
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
                }).with_metadata("Dereferencing a non-pointer type")?;

                if !self.type_equals(&dst_ty_deref, &src_ty) {
                    return Err(CheckError::MismatchType {
                        expected: dst_ty_deref.clone(),
                        found: src_ty,
                        expr: stmt.clone(),
                    }).with_metadata("Assignment type does not match variable type");
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
                    }).with_metadata("Condition of while loop is not a boolean");
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
                    }).with_metadata("Condition of if statement is not a boolean");
                }
                self.check_helper(then, expected_ret_ty)?;
                self.check_helper(else_, expected_ret_ty)
            }

            Stmt::Block(stmts) => {
                let mut env = self.new_local_scope();
                for stmt in stmts {
                    env.check_helper(stmt, expected_ret_ty)?;
                }
                trace!("\n\nChild scope:\n{env:#?}\n\n");
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
        }
    }
}

#[cfg(test)]
mod tests {
    use nom::error::VerboseError;
    
    use super::*;
    use codegen::ToMage;

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

        trace!("\n\n{:#?}\n\n", env);
        assert!(env.type_equals(&Type::Named("N".into()), &Type::Named("int".into())));
    }

    #[test]
    fn test_parse_struct() {
        let input = r#"struct { x: Int, y: Int }"#;

        let (rest, ty) = parse_struct_type::<VerboseError<&str>>(input).unwrap();
        trace!("{:#?}", ty);
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
        trace!("{:#?}", program);
        let mut ctx = Env::default();
        match ctx.check(&program) {
            Ok(_) => trace!("Check successful"),
            Err(e) => trace!("Check failed: {e:#?}"),
        }
        trace!("{:#?}\n\n", ctx);
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
        trace!("{:#?}", program);
        let mut ctx = Env::default();
        match ctx.check(&program) {
            Ok(_) => trace!("Check successful"),
            Err(e) => trace!("Check failed: {e:#?}"),
        }
        // trace!("{:#?}\n\n", ctx);

        // // Compile to mage
        let mut ctx = Env::default();
        let mage = program.compile_to_mage(&mut ctx).unwrap();
        trace!("MAGE:\n\n{}", mage);

        // Write to `test.mg`
        std::fs::write("test.mg", mage).unwrap();
    }



    #[test]
    fn test_to_mage2() {
        let input = r#"
        type OpType = enum { Add, Mul, Div, Sub, Num };
        type BinOp = struct {
            op: OpType,
            lhs: Op,
            rhs: Op
        };

        type Op = union {
            binop: BinOp,
            num: Int
        };


        let x: Op = Op of binop { op: OpType of Add, lhs: Op of num 1, rhs: Op of num 2 };
        "#;

        let program = match parse(input) {
            Ok(program) => program,
            Err(e) => panic!("Failed to parse: {e:#?}"),
        };
        trace!("{:#?}", program);
        let mut ctx = Env::default();
        match ctx.check(&program) {
            Ok(_) => trace!("Check successful"),
            Err(e) => trace!("Check failed: {e:#?}"),
        }
        // trace!("{:#?}\n\n", ctx);

        // // Compile to mage
        let mut ctx = Env::default();
        let mage = program.compile_to_mage(&mut ctx).unwrap();
        trace!("MAGE:\n\n{}", mage);

        // Write to `test.mg`
        std::fs::write("test.mg", mage).unwrap();
    }
}